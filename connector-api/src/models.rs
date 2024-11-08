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
    pub player_id: String,
    pub game: String,
    pub mode: GameMode,
}

#[derive(Deserialize)]
pub struct Host {
    pub player_id: String,
    pub invite_players: Vec<String>,
    pub game: String,
    pub config: Value,
}

#[derive(Deserialize)]
pub struct DirectConnect {
    pub write_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Match {
    pub address: String,
    pub read: String,
    pub write: String,
}
