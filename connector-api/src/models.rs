use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Default, Debug, Clone)]
#[cfg_attr(test, derive(Serialize))]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[cfg_attr(test, derive(Serialize))]
pub struct Search {
    pub region: String,
    pub session_token: String,
    pub game: String,
    pub mode: String,
    pub ai: bool,
}

#[derive(Deserialize, Debug)]
pub struct Host {
    pub session_token: String,
    pub region: String,
    pub game: String,
    pub mode: String,
    pub public: bool,
}

#[derive(Deserialize)]
pub struct JoinPub {
    pub session_token: String,
    pub host_id: String,
}

#[derive(Deserialize)]
pub struct JoinPriv {
    pub session_token: String,
    pub join_token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Match {
    pub address: String,
    pub read: String,
    pub write: String,
    pub players: Vec<String>,
}

#[derive(Serialize)]
pub struct HostInfo {
    pub host_id: String,
    pub join_token: String,
}
