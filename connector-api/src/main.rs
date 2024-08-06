use std::{sync::{Arc, Mutex}, time::SystemTime};

use axum::{body::Bytes, routing::get};
use models::{DirectConnect, Host, Search};
use serde_json::{ser, Value};
use socketioxide::{
    adapter, extract::{AckSender, Bin, Data, SocketRef}, SocketIo
};
use tracing::info;
use tracing_subscriber::FmtSubscriber;
use serde::{Deserialize, Serialize};
use matchmaking_state::{adapters::{redis::RedisAdapter, Gettable, Insertable}, models::{DBGameServer, GameMode, GameServer, Searcher}};
use matchmaking_state::adapters::DataAdapter;

mod models;


pub struct Handler {
    search: Mutex<Option<Search>>,
    state: Arc<Mutex<RedisAdapter>>,
    search_id: Mutex<Option<String>>
}

impl Handler {
    pub fn new(state: Arc<Mutex<RedisAdapter>>) -> Self {
        Self {
            search: Mutex::new(None),
            state ,
            search_id: Mutex::new(None)
        }
    }

    pub fn handle_search(&self, socket: SocketRef, data: Search, _: Vec<Bytes>) {
        // TODO: First verify if the user with this id, actually exists and has the correct token etc.
        let servers: Vec<String> = self.state.lock().unwrap().all().unwrap().filter(|server: &DBGameServer| {
            let game_mode = GameMode {
                name: data.mode.name.clone(),
                player_count: data.mode.player_count,
                computer_lobby: data.mode.computer_lobby 
            };
            server.name == data.game && server.modes.contains(&game_mode)
        }).map(|server| server.server).collect();
        // TODO: Throw some error and return it to the client if the selected game_mode is not valid. Ask the game-servers for validity. Rethink the saving the GameModes in the DB approach

        *self.search.lock().unwrap() = Some(data);
        socket.emit("servers", servers).ok();
    }

    pub fn handle_host(&self, socket: SocketRef, data: Host, bin: Vec<Bytes>) {
    }

    pub fn handle_join(&self, socket: SocketRef, data: DirectConnect, bin: Vec<Bytes>) {
    }

    pub fn handle_servers(&self, socket: SocketRef, data: Vec<String>, bin: Vec<Bytes>) {
        let elo = 42; // TODO: Get real elo from leitner

        let search = self.search.lock().unwrap();

        if search.is_none() {
            socket.emit("reject", "Search has not been started").ok();
            return;
        }
        let search = search.as_ref().unwrap();

        let searcher = Searcher {
            player_id: search.player_id.clone(),
            elo,
            mode: GameMode {
                name: search.mode.name.clone(),
                player_count: search.mode.player_count,
                computer_lobby: search.mode.computer_lobby
            },
            servers: data,
            wait_start: SystemTime::now()
        };
        let uuid = self.state.lock().unwrap().insert(searcher).unwrap();
        self.search_id.lock().unwrap().replace(uuid);
    }

}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing::subscriber::set_global_default(FmtSubscriber::default())?;
    let adapter = Arc::new(Mutex::new(RedisAdapter::connect("redis://0.0.0.0:6379").expect("Connection to redis database failed")));

    let (layer, io) = SocketIo::new_layer();

    let on_match_search = |socket: SocketRef, Data(data): Data<Value>| {
        info!("Socket.IO connected: {:?} {:?}", socket.ns(), socket.id);
        socket.emit("auth", data).ok();
        let handler = Arc::new(Handler::new(adapter));


        // TODO: The client sends information about the search. For example the target game and configurations for the game (like f.e. multiplayer or 2-player)
        let search_handler = handler.clone();
        socket.on("search", move |socket: SocketRef, Data::<Search>(data), Bin(bin)| search_handler.handle_search(socket, data, bin));

        let host_handler = handler.clone();
        socket.on("host", move |socket: SocketRef, Data::<Host>(data), Bin(bin)| host_handler.handle_host(socket, data, bin));
        let join_handler = handler.clone();
        socket.on("join", move |socket: SocketRef, Data::<DirectConnect>(data), Bin(bin)| join_handler.handle_join(socket, data, bin));
        

        // TODO: The client returns the available servers ranked by ping
        let servers_handler = handler.clone();
        socket.on("servers", move |socket: SocketRef, Data::<Vec<String>>(data), Bin(bin)| servers_handler.handle_servers(socket, data, bin));
    };

    io.ns("/match", on_match_search);

    let app = axum::Router::new().layer(layer);

    info!("Starting server");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
