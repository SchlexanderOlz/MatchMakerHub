use axum::routing::get;
use serde_json::Value;
use socketioxide::{
    extract::{AckSender, Bin, Data, SocketRef},
    SocketIo,
};
use tracing::info;
use tracing_subscriber::FmtSubscriber;


fn on_match_search(socket: SocketRef, Data(data): Data<Value>) {
    info!("Socket.IO connected: {:?} {:?}", socket.ns(), socket.id);
    socket.emit("auth", data).ok();


    // TODO: The client sends information about the search. For example the target game and configurations for the game (like f.e. multiplayer or 2-player)
    socket.on("search", |socket: SocketRef, Data::<Value>(data), Bin(bin)| {});

    // TODO: The client returns the available servers ranked by ping
    socket.on("servers", |socket: SocketRef, Data::<Value>(data), Bin(bin)| {});
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing::subscriber::set_global_default(FmtSubscriber::default())?;

    let (layer, io) = SocketIo::new_layer();

    io.ns("/match", on_match_search);

    let app = axum::Router::new().layer(layer);

    info!("Starting server");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
