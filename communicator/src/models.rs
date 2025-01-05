use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum MatchError {
    AllPlayersDisconnected,
    PlayerDidNotJoin(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MatchAbrubtClose {
    pub match_id: String,
    pub reason: MatchError,
}


#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct MatchResult {
    pub match_id: String,
    pub winners: HashMap<String, u8>,
    pub losers: HashMap<String, u8>,
    pub ranking: Ranking,
    pub event_log: Vec<Value>
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Ranking {
    pub performances: HashMap<String, Vec<String>>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct CreatedMatch {
    pub region: String,
    pub player_write: HashMap<String, String>,
    pub game: String,
    pub mode: String,
    pub ai_players: Vec<String>,
    pub read: String,
    pub url_pub: String,
    pub url_priv: String,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct GameServerCreate {
    pub region: String,
    pub game: String,
    pub mode: String,
    pub min_players: u32,
    pub max_players: u32,
    pub server_pub: String,
    pub server_priv: String,
    pub ranking_conf: RankingConf,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RankingConf {
    pub max_stars: i32,
    pub description: String,
    pub performances: Vec<Performance>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Performance {
    pub name: String,
    pub weight: i32,
}


#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct CreateMatch {
    pub game: String,
    pub players: Vec<String>,
    pub ai_players: Vec<String>,
    pub mode: String,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct AIPlayerRegister {
    pub game: String,
    pub mode: String,
    pub elo: u32,
    pub display_name: String
}

#[derive(Serialize)]
pub struct Task {
    pub ai_id: String,
    pub game: String,
    pub mode: String,
    pub address: String,
    pub read: String,
    pub write: String,
    pub players: Vec<String>
}
