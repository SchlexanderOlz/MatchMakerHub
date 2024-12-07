use std::{collections::HashMap, time::SystemTime};

use gn_redisadapter_derive::{RedisIdentifiable, RedisInsertWriter, RedisOutputReader, RedisUpdater};
use serde::Deserialize;

#[cfg(feature = "redis")]
use crate::adapters::redis::RedisFilter;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "redis",
    derive(RedisInsertWriter, RedisIdentifiable),
    name("game_servers")
)]
pub struct GameServer {
    pub region: String,
    pub game: String,
    pub mode: String,
    pub server_pub: String,
    pub server_priv: String,
    pub healthy: bool,
    pub min_players: u32,
    pub max_players: u32,
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
    pub region: String,
    pub game: String,
    pub mode: String,
    pub server_pub: String,
    pub server_priv: String,
    pub healthy: bool,
    pub min_players: u32,
    pub max_players: u32,
}

impl PartialEq for DBGameServer {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "redis", derive(RedisUpdater), name("game_servers"))]
pub struct GameServerUpdater {
    pub game: Option<String>,
    pub mode: Option<String>,
    pub region: Option<String>,
    pub server_pub: Option<String>,
    pub server_priv: Option<String>,
    pub healthy: Option<bool>,
    pub min_players: Option<u32>,
    pub max_players: Option<u32>,
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
        return self.game.clone().unwrap() == check.game;
    }
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
    pub mode: String,
    pub ai: bool,
    pub game: String,
    pub region: String,
    pub min_players: u32,
    pub max_players: u32,
    pub wait_start: SystemTime,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "redis", derive(RedisUpdater), name("searchers"))]
pub struct SearcherUpdate {
    pub player_id: Option<String>,
    pub elo: Option<u32>,
    pub mode: Option<String>,
    pub ai: Option<bool>,
    pub game: Option<String>,
    pub region: Option<String>,
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
    pub mode: String,
    pub ai: bool,
    pub game: String,
    pub region: String,
    pub min_players: u32,
    pub max_players: u32,
    pub wait_start: SystemTime,
}

#[derive(Debug, Default)]
pub struct SearcherFilter {
    pub game: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Match {
    pub region: String,
    pub game: String,
    pub players: Vec<String>,
    pub mode: String,
    pub ai: bool,
}

unsafe impl Send for Match {}
unsafe impl Sync for Match {}

#[cfg(feature = "redis")]
#[derive(Debug, Clone, RedisInsertWriter, RedisOutputReader, RedisIdentifiable)]
#[name("config")]
#[single_instance(true)]
pub struct SearcherMatchConfig {
    pub max_elo_diff: u32,
    pub wait_time_to_elo_factor: f32,
    pub wait_time_to_server_factor: f32,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "redis", derive(RedisInsertWriter, RedisIdentifiable), name("active_matches"))]
pub struct ActiveMatch {
    pub game: String,
    pub mode: String,
    pub ai: bool,
    pub server_pub: String,
    pub server_priv: String,
    pub region: String,
    pub read: String,
    pub player_write: HashMap<String, String>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "redis", derive(RedisOutputReader, RedisIdentifiable), name("active_matches"))]
pub struct ActiveMatchDB {
    #[uuid]
    pub uuid: String,
    pub game: String,
    pub mode: String,
    pub ai: bool,
    pub server_pub: String,
    pub server_priv: String,
    pub region: String,
    pub read: String,
    pub player_write: HashMap<String, String>,
}