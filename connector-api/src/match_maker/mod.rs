use core::fmt;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use matchmaking_state::{
    adapters::{redis::RedisAdapter, Matcher},
    models::GameMode,
};
use pool::GameServerPool;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::models::Match;

mod pool;

#[derive(Debug)]
pub enum MatchingError {
    ServerNotFound,
}

impl fmt::Display for MatchingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Server not found")
    }
}

pub struct MatchMaker<T>
where
    Self: 'static,
    T: FnOnce(Match) -> () + Send + Sync + 'static,
{
    servers: GameServerPool,
    handlers: HashMap<String, T>,
}

impl<T> MatchMaker<T>
where
    T: FnOnce(Match) -> () + Send + Sync + 'static,
{
    pub fn new(connection: Arc<Mutex<RedisAdapter>>) -> Arc<Mutex<Self>>
where {
        connection.lock().unwrap().start_match_check();

        let connection = connection;
        let mut servers = GameServerPool::new(connection.clone());
        servers.populate();

        let instance = Arc::new(Mutex::new(Self {
            servers,
            handlers: HashMap::new(),
        }));

        let copy = instance.clone();
        connection
            .lock()
            .unwrap()
            .on_match(move |new: matchmaking_state::models::Match| {
                copy.lock().unwrap().create(new).unwrap();
            });

        instance
    }

    pub fn notify_on_match(&mut self, searcher_uuid: &str, handler: T) {
        self.handlers.insert(searcher_uuid.to_string(), handler);
    }

    pub fn create(
        &mut self,
        match_info: matchmaking_state::models::Match,
    ) -> Result<(), MatchingError> {
        info!("Sasuausu");
        let server = self.servers.get_server_by_address(match_info.address);
        let server = match server {
            Some(server) => server,
            None => return Err(MatchingError::ServerNotFound),
        };
        info!("Creating new match on server: {:?}", server);

        // TODO: Create a game on the server per TCP-Connection via the Game-Servers API using JSON

        let server_match = Match {
            address: server.server,
            read: "sososo".to_string(),
            write: "sosoosese".to_string(),
        };

        for player_uuid in match_info.players {
            if let Some(handler) = self.handlers.remove(&player_uuid) {
                let server_match_clone = server_match.clone();
                tokio::task::spawn(async move {
                    handler(server_match_clone);
                });
            }
        }
        Ok(())
    }
}
