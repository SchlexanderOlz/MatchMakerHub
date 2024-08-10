use std::sync::{Arc, Mutex};

use handler::Handler;
use matchmaking_state::adapters::{redis::RedisAdapter, Removable};
use models::{DirectConnect, Host, Search};
use serde_json::Value;
use socketioxide::{
    extract::{Bin, Data, SocketRef},
    SocketIo,
};
use tracing::info;
use tracing_subscriber::FmtSubscriber;

mod handler;
mod match_maker;
mod models;

static DEFAULT_HOST_ADDRESS: &str = "0.0.0.0:4000";

fn setup_listeners(io: &SocketIo, adapter: Arc<Mutex<RedisAdapter>>) {
    let match_maker = match_maker::MatchMaker::new(adapter.clone());
    let on_match_search = |socket: SocketRef, Data(data): Data<Value>| {
        info!("Socket.IO connected: {:?} {:?}", socket.ns(), socket.id);
        socket.emit("auth", data).ok();
        let handler = Arc::new(Handler::new(adapter.clone()));

        // TODO: The client sends information about the search. For example the target game and configurations for the game (like f.e. multiplayer or 2-player)
        let search_handler = handler.clone();
        socket.on(
            "search",
            move |socket: SocketRef, Data::<Search>(data), Bin(bin)| {
                info!("Search event received: {:?}", data);
                search_handler.handle_search(&socket, data, bin)
            },
        );

        let host_handler = handler.clone();
        socket.on(
            "host",
            move |socket: SocketRef, Data::<Host>(data), Bin(bin)| {
                host_handler.handle_host(&socket, data, bin)
            },
        );
        let join_handler = handler.clone();
        socket.on(
            "join",
            move |socket: SocketRef, Data::<DirectConnect>(data), Bin(bin)| {
                join_handler.handle_join(&socket, data, bin)
            },
        );

        let servers_handler = handler.clone();
        socket.on(
            "servers",
            move |socket: SocketRef, Data::<Vec<String>>(data), Bin(bin)| {
                info!("Servers event received: {:?}", data);
                servers_handler.handle_servers(&socket, data, bin);
                let uuid = servers_handler.get_searcher_id().unwrap();

                match_maker.lock().unwrap().notify_on_match(
                    uuid.clone().as_str(),
                    move |r#match| {
                        servers_handler.notify_match_found(&socket, r#match);
                        adapter
                            .clone()
                            .lock()
                            .unwrap()
                            .remove(uuid.as_str())
                            .unwrap();
                    },
                );
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

    tracing::subscriber::set_global_default(FmtSubscriber::default())?;

    info!("Starting server");
    let adapter = Arc::new(Mutex::new(
        RedisAdapter::connect(&default_redis_url).expect("Connection to redis database failed"),
    ));

    let (layer, io) = SocketIo::new_layer();
    setup_listeners(&io, adapter);
    let app = axum::Router::new().layer(layer);

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
    use rust_socketio::asynchronous::ClientBuilder;

    #[tokio::test]
    async fn test_connect() {
        tracing::subscriber::set_global_default(FmtSubscriber::default()).unwrap();
        info!("Starting server");
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let on_servers = {
            move |_, _| {
                let tx = Arc::clone(&tx);
                async move {
                    info!("Server");
                    assert!(true);
                    loop {
                        info!("asddasdasdjlkjasadasdfasd");
                        let _ = tx.lock().unwrap().take().unwrap().send(()).unwrap();
                        info!("asasadasdfasd");
                    }
                }
                .boxed()
            }
        };

        let socket = ClientBuilder::new("http://127.0.0.1:4000/")
            .namespace("/match")
            .on("servers", on_servers)
            .on_any(|event, _, _| {
                async move {
                    info!("Unhandled event: {:?}", event);
                }
                .boxed()
            })
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
            game: "Fortnite".to_string(),
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
}
