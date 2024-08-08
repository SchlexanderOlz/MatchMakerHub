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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing::subscriber::set_global_default(FmtSubscriber::default())?;
    let adapter = Arc::new(Mutex::new(
        RedisAdapter::connect("redis://0.0.0.0:6379").expect("Connection to redis database failed"),
    ));

    let (layer, io) = SocketIo::new_layer();
    setup_listeners(&io, adapter);
    let app = axum::Router::new().layer(layer);

    info!("Starting server");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
