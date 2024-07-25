use serde::{Deserialize, Serialize};
use redisadapter_derive::{RedisInsertWriter, RedisOutputReader};

use crate::adapters::redis::RedisFilter;


#[derive(Debug, Clone, RedisInsertWriter)]
pub struct GameServer {
    pub name: String,
    pub modes: Vec<GameMode>,
    pub server: String,
    pub token: String, // Token to authorize as the main-server at this game-server
}

#[derive(Debug, Clone, RedisOutputReader)]
pub struct DBGameServer {
    #[uuid]
    pub uuid: String,
    pub name: String,
    pub modes: Vec<GameMode>,
    pub server: String,
    pub token: String,
}

#[derive(Debug, Default)]
pub struct GameServerFilter {
    pub game: Option<String>,
}

impl RedisFilter<DBGameServer> for GameServerFilter {
    fn is_ok(&self, check: &DBGameServer) -> bool {
        if self.game.is_none() {
            return true;
        }
        return self.game.clone().unwrap() == check.name;
    }
}

#[derive(Debug, Clone, RedisInsertWriter, RedisOutputReader)]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Searcher {
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

#[derive(Debug, Default)]
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
