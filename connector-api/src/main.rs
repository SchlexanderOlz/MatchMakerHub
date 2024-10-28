use std::sync::{Arc, Mutex};

use tower::ServiceBuilder;
use handler::Handler;
use gn_matchmaking_state::{
    adapters::{redis::RedisAdapter, Gettable},
    models::DBSearcher,
};
use models::{DirectConnect, Host, Search};
use rand::rngs::adapter;
use serde_json::Value;
use socketioxide::{
    extract::{Data, SocketRef},
    handler::disconnect,
    SocketIo,
};
use tower_http::{cors::{Any, CorsLayer}};
use tracing::{debug, info, Level};
use tracing_subscriber::FmtSubscriber;

mod handler;
mod match_maker;
mod models;

static DEFAULT_HOST_ADDRESS: &str = "0.0.0.0:4000";

fn setup_listeners(io: &SocketIo, adapter: Arc<RedisAdapter>) {
    let match_maker = match_maker::MatchMaker::new(adapter.clone());
    let adapter_clone = adapter.clone();
    let on_match_search = move |socket: SocketRef| {
        info!("Socket.IO connected: {:?} {:?}", socket.ns(), socket.id);
        let handler = Arc::new(Handler::new(adapter_clone.clone()));

        let search_handler = handler.clone();
        socket.on(
            "search",
            move |socket: SocketRef, Data::<Search>(data)| {
                debug!("Search event received: {:?}", data);
                search_handler.handle_search(&socket, data)
            },
        );

        let disconnect_handler = handler.clone();
        socket.on_disconnect(move |socket: SocketRef| {
            info!("Socket.IO disconnected: {:?}", socket.id);
            disconnect_handler.handle_disconnect(&socket);
        });

        let host_handler = handler.clone();
        socket.on(
            "host",
            move |socket: SocketRef, Data::<Host>(data)| {
                host_handler.handle_host(&socket, data)
            },
        );
        let join_handler = handler.clone();
        socket.on(
            "join",
            move |socket: SocketRef, Data::<DirectConnect>(data)| {
                join_handler.handle_join(&socket, data)
            },
        );

        let servers_handler = handler.clone();
        let adapter = adapter_clone.clone();
        socket.on(
            "servers",
            move |socket: SocketRef, Data::<Vec<String>>(data)| {
                debug!("Servers event received: {:?}", data);
                servers_handler.handle_servers(&socket, data);
                let uuid = servers_handler.get_searcher_id().unwrap();
                let instance: DBSearcher = adapter.get(&uuid).unwrap();

                match_maker
                    .lock()
                    .unwrap()
                    .notify_on_match(&instance.player_id, move |r#match| {
                        debug!("Match found: {:?}", r#match);
                        servers_handler.notify_match_found(&socket, r#match);
                        debug!("Match found event emitted");
                    });
            },
        );
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
    let adapter = Arc::new(
        RedisAdapter::connect(&default_redis_url).expect("Connection to redis database failed"),
    );

    let (layer, io) = SocketIo::new_layer();
    setup_listeners(&io, adapter);

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
    use rand::Rng;
    use rust_socketio::asynchronous::{Client, ClientBuilder};

    #[tokio::test]
    async fn test_connect() {
        info!("Starting server");
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let on_servers = {
            move |_, _| {
                let tx = Arc::clone(&tx);
                async move {
                    info!("Server received");
                    assert!(true);
                    let _ = tx.lock().unwrap().take().unwrap().send(()).unwrap();
                }
                .boxed()
            }
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
            player_id: "test".to_string(),
            game: "SchnapsenTest".to_string(),
            mode: GameMode {
                player_count: 2,
                name: "duo".to_string(),
                computer_lobby: false,
            },
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
            player_id: format!("test{}", rand::thread_rng().gen_range(0..100000)).to_string(),
            game: "Schnapsen".to_string(),
            mode: GameMode {
                player_count: 2,
                name: "duo".to_string(),
                computer_lobby: false,
            },
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
