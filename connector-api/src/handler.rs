use gn_matchmaking_state::{
    adapters::{redis::RedisAdapterDefault, Gettable, Insertable, Removable},
    models::{DBGameServer, DBSearcher, GameMode, Searcher},
};
use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tracing::{debug, info};

use axum::body::Bytes;
use socketioxide::extract::SocketRef;

use crate::{
    ezauth,
    models::{DirectConnect, Host, Match, Search},
};

pub struct Handler {
    search: Mutex<Option<Search>>,
    state: Arc<RedisAdapterDefault>,
    search_id: Mutex<Option<String>>,
    ezauth_url: String,
}

impl Handler {
    pub fn new(state: Arc<RedisAdapterDefault>) -> Self {
        let ezauth_url = std::env::var("EZAUTH_URL").unwrap();
        let ezauth_url = ezauth_url.to_string();

        Self {
            search: Mutex::new(None),
            state,
            search_id: Mutex::new(None),
            ezauth_url,
        }
    }

    #[inline]
    pub fn get_searcher_id(&self) -> Option<String> {
        self.search_id.lock().unwrap().clone()
    }

    pub async fn handle_search(
        &self,
        socket: &SocketRef,
        data: Search,
    ) -> Result<(), Box<dyn std::error::Error + 'static>> {
        debug!("Received Search event: {:?}", data);
        let validation = ezauth::validate_user(&data.session_token, &self.ezauth_url).await?;

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
                server.healthy
                    && server.game == data.game
                    && server.mode == game_mode
                    && server.region == data.region
            })
            .map(|server| server.server_pub)
            .collect();

        debug!("Servers found for search ({:?}): {:?}", data, servers);

        if servers.is_empty() {
            return Err("No such server online".into());
        }
        // TODO: Throw some error and return it to the client if the selected game_mode is not valid. Ask the game-servers for validity. Rethink the saving the GameModes in the DB approach

        let elo = 42; // TODO: Get real elo from leitner

        let search = data;

        // TODO: Replace all function with find function
        if let Some(searcher) = self
            .state
            .all()
            .unwrap()
            .find(|x: &DBSearcher| x.player_id == validation._id)
        {
            self.search_id.lock().unwrap().replace(searcher.uuid);
            return Ok(());
        }

        let searcher = Searcher {
            player_id: validation._id.clone(),
            elo,
            game: search.game.clone(),
            mode: GameMode {
                name: search.mode.name.clone(),
                player_count: search.mode.player_count,
                computer_lobby: search.mode.computer_lobby,
            },
            region: search.region.clone(),
            wait_start: SystemTime::now(),
        };
        let uuid = self.state.insert(searcher).unwrap();
        debug!("Searcher inserted with uuid: {}", uuid);
        self.search_id.lock().unwrap().replace(uuid);
        Ok(())
    }

    pub fn handle_host(&self, socket: &SocketRef, data: Host) {}

    pub fn handle_join(&self, socket: &SocketRef, data: DirectConnect) {}

    pub fn handle_disconnect(&self, socket: &SocketRef) {
        info!("Socket.IO disconnected: {:?}", socket.id);
        if let Some(search_id) = self.search_id.lock().unwrap().as_deref() {
            self.state.remove(search_id).unwrap();
        }
        *self.search.lock().unwrap() = None;
    }

    pub fn notify_match_found(&self, socket: &SocketRef, found_match: Match) {
        socket.emit("match", &found_match).unwrap();
    }
}
