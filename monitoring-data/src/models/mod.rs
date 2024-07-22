use redis::{FromRedisValue, ToRedisArgs, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct GameServer {
    pub name: String,
    pub modes: Vec<GameMode>,
    pub server: String,
    pub token: String, // Token to authorize as the main-server at this game-server
}

pub struct RedisInsert<T: ToRedisArgs>(pub String, pub T);

#[derive(Debug, Clone)]
pub struct DBGameServer {
    pub uuid: String,
    pub name: String,
    pub modes: Vec<GameMode>,
    pub server: String,
    pub token: String,
}

pub struct GameServerFilter {
    pub game: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Searcher {
    pub player_id: String,
    pub elo: u32,
    pub game: String,
    pub mode: String,
    pub servers: Vec<String>,
}

pub struct ModelSearcher {
    pub player_id: String,
    pub elo: u32,
    pub game: String,
    pub mode: String,
    pub servers: Vec<String>,
}

pub struct DBSearcher {
    pub uuid: String,
    pub player_id: String,
    pub elo: u32,
    pub game: String,
    pub mode: String,
    pub servers: Vec<String>,
}

pub struct SearcherFilter {
    pub game: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Match {
    // TODO: Come back to this and review
    pub players: Vec<String>,
    pub game: GameServer,
    pub mode: String,
}
