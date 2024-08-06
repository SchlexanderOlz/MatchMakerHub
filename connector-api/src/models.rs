use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Default)]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}


#[derive(Deserialize, Default)]
pub struct Search {
    pub player_id: String, 
    pub game: String,
    pub mode: GameMode,
}


#[derive(Deserialize)]
pub struct Host {
    pub player_id: String, 
    pub invite_players: Vec<String>,
    pub game: String,
    pub config: Value
}


#[derive(Deserialize)]
pub struct DirectConnect {
    pub write_key: String,
}
