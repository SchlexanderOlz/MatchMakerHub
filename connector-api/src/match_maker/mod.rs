use core::fmt;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::models::Match;
use gn_matchmaking_state::{
    adapters::{
        redis::{NotifyOnRedisEvent, RedisAdapter},
        Gettable,
    },
    models::{ActiveMatch, ActiveMatchDB},
};
use tracing::{debug, info};
use tracing_subscriber::field::debug;

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
    T: FnOnce(Match) -> () + Send + Sync + 'static, // TODO: Mark this as async
{
    handlers: HashMap<String, T>,
}

impl<T> MatchMaker<T>
where
    T: FnOnce(Match) -> () + Send + Sync + 'static,
{
    pub fn new(connection: Arc<RedisAdapter>) -> Arc<Mutex<Self>>
where {
        let connection = connection;
        let instance = Arc::new(Mutex::new(Self {
            handlers: HashMap::new(),
        }));

        let matchmaker_copy = instance.clone();

        let connection_clone = connection.clone();
        ActiveMatch::on_insert(&connection, move |uuid: String| {
            debug!("New match created with uuid: {}", uuid);
            let new: ActiveMatchDB = connection_clone.get(&uuid).unwrap();
            matchmaker_copy.lock().unwrap().create(new).unwrap();
            debug!("All player handlers for match {:?} notified", uuid);
        })
        .unwrap();

        instance
    }

    pub fn notify_on_match(&mut self, searcher_uuid: &str, handler: T) {
        self.handlers.insert(searcher_uuid.to_string(), handler);
    }

    pub fn create(
        &mut self,
        match_info: gn_matchmaking_state::models::ActiveMatchDB,
    ) -> Result<(), MatchingError> {
        debug!("Handlers: {:?}", self.handlers.keys());
        debug!("Match info: {:?}", match_info);
        for (key, val) in match_info.player_write.into_iter() {
            if let Some(handler) = self.handlers.remove(&key) {
                let server_match = Match {
                    address: match_info.server_pub.clone(),
                    read: match_info.read.clone(),
                    write: val,
                };

                tokio::task::spawn_blocking(|| handler(server_match));
            } else {
                debug!("No handler found for player: {}", key);
            }
        }
        Ok(())
    }
}
