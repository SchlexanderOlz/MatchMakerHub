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
    pub reserved_players: Vec<String>,
}

#[derive(Deserialize)]
pub struct DirectConnect {
    pub session_token: String,
    pub host_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Match {
    pub address: String,
    pub read: String,
    pub write: String,
    pub players: Vec<String>
}




#[derive(Deserialize, Clone, Debug)]
pub struct EZAUTHValidationResponse {
    pub _id: String,
    pub username: String,
    pub email: String,

    #[serde(rename = "createdAt")]
    pub created_at: String
}

#[derive(Serialize)]
pub struct HostInfo {
    pub host_id: String,
}