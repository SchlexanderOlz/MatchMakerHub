use std::sync::{Arc, Mutex};

use gn_matchmaking_state::models::{DBSearcher, HostRequestDB};
use gn_matchmaking_state::prelude::*;
use handler::Handler;
use match_maker::MatchMaker;
use models::{DirectConnect, Host, HostInfo, Search};
use rand::rngs::adapter;
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use tower_http::{
    cors::{Any, CorsLayer},
    validate_request::ValidateRequestHeaderLayer,
};
use tracing::{debug, info, Level};
use tracing_subscriber::FmtSubscriber;

mod ezauth;
mod handler;
mod match_maker;
mod models;

static DEFAULT_HOST_ADDRESS: &str = "0.0.0.0:4000";

fn setup_listeners(
    io: &SocketIo,
    adapter: Arc<RedisAdapterDefault>,
    ranking_client: Arc<gn_ranking_client_rs::RankingClient>,
) {
    let match_maker = match_maker::MatchMaker::new(adapter.clone());
    let adapter_clone = adapter.clone();

    let on_match_search = {
        move |socket: SocketRef| {
            info!("Socket.IO connected: {:?} {:?}", socket.ns(), socket.id);
            let handler = Arc::new(Handler::new(adapter_clone.clone(), ranking_client));

            let notify_on_match = {
                let handler = handler.clone();
                let socket = socket.clone();
                move || {
                    match_maker.lock().unwrap().notify_on_match(
                        &handler.get_user_id().unwrap(),
                        move |r#match| {
                            debug!("Match found: {:?}", r#match);
                            handler.notify_match_found(&socket, r#match);
                            debug!("Match found event emitted");
                        },
                    );
                }
            };

            socket.on("search", {
                let handler = handler.clone();
                let init_notify_on_match = notify_on_match.clone();
                move |socket: SocketRef, Data::<Search>(data)| async move {
                    debug!("Search event received: {:?}", data);
                    if let Err(err) = handler.handle_search(data).await {
                        socket.emit("error", &err.to_string()).ok();
                        return;
                    }

                    init_notify_on_match();
                }
            });

            let disconnect_handler = handler.clone();
            socket.on_disconnect(move |socket: SocketRef| {
                info!("Socket.IO disconnected: {:?}", socket.id);
                disconnect_handler.handle_disconnect();
            });

            let host_handler = handler.clone();
            socket.on("host", {
                let init_notify_on_match = notify_on_match.clone();
                move |socket: SocketRef, Data::<Host>(data)| async move {
                    if let Err(err) = host_handler.handle_host(data).await {
                        socket.emit("error", &err.to_string()).ok();
                        return;
                    }

                    let host_info = HostInfo {
                        host_id: host_handler.get_searcher_id().unwrap(),
                    };
                    init_notify_on_match();
                    socket.emit("host_info", &host_info).ok();
                }
            });

            let host_handler = handler.clone();
            socket.on("start", move |socket: SocketRef| async move {
                if let Err(err) = host_handler.handle_start().await {
                    socket.emit("error", &err.to_string()).ok();
                };
            });

            let join_handler = handler.clone();
            socket.on("join", {
                let init_notify_on_match = notify_on_match.clone();
                move |socket: SocketRef, Data::<DirectConnect>(data)| async move {
                    if let Err(err) = join_handler.handle_join(data).await {
                        socket.emit("error", &err.to_string()).ok();
                        return;
                    }
                    init_notify_on_match();
                }
            });
        }
    };

    io.ns("/match", on_match_search);
    info!("Listeners setup for namespace /match");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let default_redis_url: String = std::env::var("REDIS_URL").unwrap();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting server");
    let adapter =
        RedisAdapter::connect(&default_redis_url).expect("Connection to redis database failed");
    let publisher = RedisInfoPublisher::new(adapter.client.get_connection().unwrap());
    let adapter = Arc::new(adapter.with_publisher(publisher));

    let ranking_client = Arc::new(gn_ranking_client_rs::RankingClient::new(
        std::env::var("RANKING_API_KEY").unwrap(),
    ));

    let (layer, io) = SocketIo::new_layer();
    setup_listeners(&io, adapter, ranking_client);

    let cors = CorsLayer::new().allow_origin(Any);

    let app = axum::Router::new().layer(cors).layer(layer);

    let listener = tokio::net::TcpListener::bind(DEFAULT_HOST_ADDRESS)
        .await
        .unwrap();

    info!("Server listening");
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::future::FutureExt;
    use models::GameMode;
    use rust_socketio::asynchronous::{Client, ClientBuilder};

    #[tokio::test]
    async fn test_connect() {
        info!("Starting server");
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let on_servers = move |_, _| {
            let tx = Arc::clone(&tx);
            async move {
                info!("Server received");
                assert!(true);
                let _ = tx.lock().unwrap().take().unwrap().send(()).unwrap();
            }
            .boxed()
        };

        let socket = ClientBuilder::new("http://127.0.0.1:4000/")
            .namespace("/match")
            .on("servers", on_servers)
            .on("connect_error", |err, _| {
                async move {
                    info!("Error: {:?}", err);
                }
                .boxed()
            })
            .transport_type(rust_socketio::TransportType::Polling) // WTF
            .connect()
            .await
            .unwrap();

        info!("Connected to server");

        let search = Search {
            session_token: "saus".to_string(),
            region: "eu".to_string(),
            game: "SchnapsenTest".to_string(),
            mode: "duo".to_string(),
            ai: false,
        };

        socket
            .emit("search", serde_json::to_value(search).unwrap())
            .await
            .unwrap();
        info!("Search event emitted");
        rx.await.unwrap();
        socket.disconnect().await.unwrap();
    }

    #[tokio::test]
    async fn test_search() {
        tracing::subscriber::set_global_default(FmtSubscriber::default()).unwrap();
        info!("Starting server");
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let on_servers = {
            move |payload, client: Client| {
                async move {
                    info!("Server received: {:?}", payload);
                    client.emit("servers", payload).await.unwrap();
                }
                .boxed()
            }
        };

        let on_match = {
            move |_, _| {
                let tx = Arc::clone(&tx);
                async move {
                    info!("Match received");
                    assert!(true);
                    let _ = tx.lock().unwrap().take().unwrap().send(()).unwrap();
                }
                .boxed()
            }
        };

        let make_socket = || {
            ClientBuilder::new("http://127.0.0.1:4000/")
                .namespace("/match")
                .on("servers", on_servers)
                .on("connect_error", |err, _| {
                    async move {
                        info!("Error: {:?}", err);
                    }
                    .boxed()
                })
                .on("match", on_match.clone())
                .transport_type(rust_socketio::TransportType::Polling) // WTF
                .connect()
        };

        let socket = make_socket().await.unwrap();
        let other_socket = make_socket().await.unwrap();

        info!("Connected to server");

        let make_search = || Search {
            session_token: "saus".to_string(),
            region: "eu".to_string(),
            game: "Schnapsen".to_string(),
            mode: "duo".to_string(),
            ai: false,
        };

        let search = make_search();
        let other = make_search();

        socket
            .emit("search", serde_json::to_value(search).unwrap())
            .await
            .unwrap();

        other_socket
            .emit("search", serde_json::to_value(other).unwrap())
            .await
            .unwrap();
        info!("Search event emitted");
        rx.await.unwrap();
        socket.disconnect().await.unwrap();
    }
}
