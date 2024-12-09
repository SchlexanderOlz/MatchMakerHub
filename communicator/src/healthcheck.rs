use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use gn_matchmaking_state::{adapters::{Removable, Updateable}, prelude::RedisAdapterDefault};
use gn_matchmaking_state_types::GameServerUpdater;
use tracing::debug;

pub struct HealthCheck {
    pub connection: Arc<RedisAdapterDefault>,
    pub active_clients: HashMap<String, DateTime<Utc>>
}

const CLIENT_TIMEOUT: i64 = 30;

impl HealthCheck {
    pub fn new(connection: Arc<RedisAdapterDefault>) -> Self {
        Self {
            connection,
            active_clients: HashMap::new()
        }
    }

    #[inline]
    pub fn refresh(&mut self, client_id: String) {
        debug!("Client {} has refreshed", client_id);
        if self.active_clients.insert(client_id.clone(), Utc::now()).is_none() {
            let mut update = GameServerUpdater::default();
            update.healthy = Some(true);

            let _ = self.connection.update(&client_id, update);
        }
    }

    #[inline]
    pub fn check(&mut self) -> bool {
        let now = Utc::now();

        self.active_clients.clone().into_iter().filter(|(_, v)| {
            now.signed_duration_since(*v).num_seconds() >= CLIENT_TIMEOUT 
        }).for_each(|(k, _)| {
            debug!("Client {} has timed out", k);

            let mut update = GameServerUpdater::default();
            update.healthy = Some(false);

            self.connection.update(&k, update).unwrap();
            self.active_clients.remove(&k);
        });

        self.active_clients.len() > 0
    }
}