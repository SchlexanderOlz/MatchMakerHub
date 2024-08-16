use matchmaking_state::{
    adapters::{redis::RedisAdapter, Gettable, Insertable},
    models::{DBGameServer, GameMode, Searcher},
};
use tracing_subscriber::field::debug;
use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tracing::{debug, info};

use axum::body::Bytes;
use socketioxide::extract::SocketRef;

use crate::models::{DirectConnect, Host, Match, Search};

pub struct Handler {
    search: Mutex<Option<Search>>,
    state: Arc<RedisAdapter>,
    search_id: Mutex<Option<String>>,
}

impl Handler {
    pub fn new(state: Arc<RedisAdapter>) -> Self {
        Self {
            search: Mutex::new(None),
            state,
            search_id: Mutex::new(None),
        }
    }

    #[inline]
    pub fn get_searcher_id(&self) -> Option<String> {
        self.search_id.lock().unwrap().clone()
    }

    pub fn handle_search(&self, socket: &SocketRef, data: Search, bin: Vec<Bytes>) {
        info!("Received Search event: {:?}", data);
        // TODO: First verify if the user with this id, actually exists and has the correct token etc.
        let servers: Vec<String> = self
            .state
            .all()
            .unwrap()
            .filter(|server: &DBGameServer| {
                let game_mode = GameMode {
                    name: data.mode.name.clone(),
                    player_count: data.mode.player_count,
                    computer_lobby: data.mode.computer_lobby,
                };
                server.name == data.game && server.modes.contains(&game_mode)
            })
            .map(|server| server.server)
            .collect();
        // TODO: Throw some error and return it to the client if the selected game_mode is not valid. Ask the game-servers for validity. Rethink the saving the GameModes in the DB approach

        *self.search.lock().unwrap() = Some(data);
        socket
            .emit("servers", [serde_json::to_value(servers).unwrap()])
            .ok();
    }

    pub fn handle_host(&self, socket: &SocketRef, data: Host, bin: Vec<Bytes>) {}

    pub fn handle_join(&self, socket: &SocketRef, data: DirectConnect, bin: Vec<Bytes>) {}

    pub fn handle_servers(&self, socket: &SocketRef, data: Vec<String>, bin: Vec<Bytes>) {
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
            game: search.game.clone(),
            mode: GameMode {
                name: search.mode.name.clone(),
                player_count: search.mode.player_count,
                computer_lobby: search.mode.computer_lobby,
            },
            servers: data,
            wait_start: SystemTime::now(),
        };
        let uuid = self.state.insert(searcher).unwrap();
        info!("Searcher inserted with uuid: {}", uuid);
        self.search_id.lock().unwrap().replace(uuid);
    }

    pub fn notify_match_found(&self, socket: &SocketRef, found_match: Match) {
        socket.emit("match", found_match).unwrap();
    }
}
