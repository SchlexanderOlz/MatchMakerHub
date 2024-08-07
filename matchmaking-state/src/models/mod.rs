use std::time::SystemTime;

use redisadapter_derive::{RedisIdentifiable, RedisInsertWriter, RedisOutputReader, RedisUpdater};

#[cfg(feature = "redis")]
use crate::adapters::redis::RedisFilter;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "redis",
    derive(RedisInsertWriter, RedisIdentifiable),
    name("game_servers")
)]
pub struct GameServer {
    pub name: String,
    pub modes: Vec<GameMode>,
    pub server: String,
    pub token: String, // Token to authorize as the main-server at this game-server
}

#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "redis",
    derive(RedisOutputReader, RedisIdentifiable),
    name("game_servers")
)]
pub struct DBGameServer {
    #[cfg_attr(feature = "redis", uuid)]
    pub uuid: String,
    pub name: String,
    pub modes: Vec<GameMode>,
    pub server: String,
    pub token: String,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "redis", derive(RedisUpdater), name("game_servers"))]
pub struct GameServerUpdater {
    pub name: Option<String>,
    pub modes: Option<Vec<GameMode>>,
    pub server: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Default)]
pub struct GameServerFilter {
    pub game: Option<String>,
}

#[cfg(feature = "redis")]
impl RedisFilter<DBGameServer> for GameServerFilter {
    fn is_ok(&self, check: &DBGameServer) -> bool {
        if self.game.is_none() {
            return true;
        }
        return self.game.clone().unwrap() == check.name;
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "redis", derive(RedisOutputReader, RedisInsertWriter))]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}

#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "redis",
    derive(RedisInsertWriter, RedisIdentifiable),
    name("searchers")
)]
pub struct Searcher {
    pub player_id: String,
    pub elo: u32,
    pub mode: GameMode,
    pub game: String,
    pub servers: Vec<String>,
    pub wait_start: SystemTime,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "redis", derive(RedisUpdater), name("searchers"))]
pub struct SearcherUpdate {
    pub player_id: Option<String>,
    pub elo: Option<u32>,
    pub mode: Option<GameMode>,
    pub game: Option<String>,
    pub servers: Option<Vec<String>>,
    pub wait_start: Option<SystemTime>,
}

#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "redis",
    derive(RedisOutputReader, RedisIdentifiable),
    name("searchers")
)]
pub struct DBSearcher {
    #[cfg_attr(feature = "redis", uuid)]
    pub uuid: String,
    pub player_id: String,
    pub elo: u32,
    pub mode: GameMode,
    pub game: String,
    pub servers: Vec<String>,
    pub wait_start: SystemTime,
}

#[derive(Debug, Default)]
pub struct SearcherFilter {
    pub game: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Match {
    pub address: String,
    pub players: Vec<String>,
}

#[cfg(feature = "redis")]
#[derive(Debug, Clone, RedisInsertWriter, RedisOutputReader, RedisIdentifiable)]
#[name("config")]
#[single_instance(true)]
pub struct SearcherMatchConfig {
    pub max_elo_diff: u32,
    pub wait_time_to_elo_factor: f32,
    pub wait_time_to_server_factor: f32,
}
