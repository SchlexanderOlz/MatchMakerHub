use std::{collections::HashMap, time::SystemTime};

use gn_redisadapter_derive::{
    RedisIdentifiable, RedisInsertWriter, RedisOutputReader, RedisUpdater,
};
use serde::Deserialize;

#[cfg(feature = "redis")]
use gn_matchmaking_state::adapters::redis::RedisFilter;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[derive(RedisInsertWriter, RedisIdentifiable)]
#[name("game_servers")]
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
#[derive(RedisOutputReader, RedisIdentifiable)]
#[name("game_servers")]
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
#[derive(RedisUpdater)]
#[name("game_servers")]
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
#[derive(RedisInsertWriter, RedisIdentifiable)]
#[name("host_requests")]

pub struct HostRequest {
    pub player_id: String,
    pub mode: String,
    pub game: String,
    pub region: String,
    pub reserved_players: Vec<String>,
    pub joined_players: Vec<String>,
    pub start_requested: bool,
    pub min_players: u32,
    pub max_players: u32,
    pub wait_start: SystemTime,
}

#[derive(Debug, Clone)]
#[derive(RedisOutputReader, RedisIdentifiable)]
#[name("host_requests")]
pub struct HostRequestDB {
    #[cfg_attr(feature = "redis", uuid)]
    pub uuid: String,
    pub player_id: String,
    pub mode: String,
    pub game: String,
    pub region: String,
    pub reserved_players: Vec<String>,
    pub joined_players: Vec<String>,
    pub start_requested: bool,
    pub min_players: u32,
    pub max_players: u32,
    pub wait_start: SystemTime,
}

#[derive(Debug, Clone, Default)]
#[derive(RedisUpdater)]
#[name("host_requests")]
pub struct HostRequestUpdate {
    pub player_id: Option<String>,
    pub mode: Option<String>,
    pub game: Option<String>,
    pub region: Option<String>,
    pub reserved_players: Option<Vec<String>>,
    pub joined_players: Option<Vec<String>>,
    pub start_requested: Option<bool>,
    pub min_players: Option<u32>,
    pub max_players: Option<u32>,
    pub wait_start: Option<SystemTime>,
}
#[derive(Debug, Clone)]
#[derive(RedisInsertWriter, RedisIdentifiable)]
#[name("searchers")]
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
#[derive(RedisUpdater)]
#[name("searchers")]
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
#[derive(RedisOutputReader, RedisIdentifiable)]
#[name("searchers")]
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
#[derive(RedisInsertWriter, RedisIdentifiable)]
#[name("active_matches")]
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
#[derive(RedisOutputReader, RedisIdentifiable)]
#[name("active_matches")]
pub struct ActiveMatchDB {
    #[cfg_attr(feature = "redis", uuid)]
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

#[cfg(test)]
mod tests {
    use std::error::Error;

    use gn_matchmaking_state::adapters::{Gettable, Insertable, Removable, Updateable};

    #[test]

    fn test_redis_adapter_insert_game_server() {
        use gn_matchmaking_state::adapters::redis::publisher::native::RedisInfoPublisher;
        use gn_matchmaking_state::adapters::redis::RedisAdapter;
        use gn_matchmaking_state::adapters::Insertable;
        use super::*;

        let adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();
        let publisher = RedisInfoPublisher::new(adapter.client.get_connection().unwrap());
        let adapter = adapter.with_publisher(publisher);


        let game_server = GameServer {
            region: "eu".to_owned(),
            game: "Test Server".to_owned(),
            mode: "Test Mode".to_owned(),
            server_pub: "127.0.0.1:3456".to_owned(),
            server_priv: "127.0.0.1:3456".to_owned(),
            healthy: true,
            min_players: 2,
            max_players: 2,
        };
        adapter.insert(game_server).unwrap();
    }

    #[test]
    fn test_redis_adapter_all_game_server() {
        use gn_matchmaking_state::adapters::redis::publisher::native::RedisInfoPublisher;
        use gn_matchmaking_state::adapters::redis::{RedisAdapter};
        use super::*;

        let adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();
        let publisher = RedisInfoPublisher::new(adapter.client.get_connection().unwrap());
        let adapter = adapter.with_publisher(publisher);



        let game_server = GameServer {
            region: "eu".to_owned(),
            game: "Test Server".to_owned(),
            mode: "Test Mode".to_owned(),
            server_pub: "127.0.0.1:3456".to_owned(),
            server_priv: "127.0.0.1:3456".to_owned(),
            healthy: true,
            min_players: 2,
            max_players: 2,
        };
        adapter.insert(game_server.clone()).unwrap();

        let found_server = adapter.all().unwrap().collect::<Vec<DBGameServer>>();

        for game in &found_server {
            println!("{:?}", game);
        }

        assert!(found_server.len() > 0);
        assert!(found_server.iter().any(|x| x.game == game_server.game));
    }

    #[test]
    fn test_redis_adapter_remove_game_server() {
        use gn_matchmaking_state::adapters::redis::publisher::native::RedisInfoPublisher;
        use gn_matchmaking_state::adapters::redis::{RedisAdapter};
        use super::*;

        let adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();
        let publisher = RedisInfoPublisher::new(adapter.client.get_connection().unwrap());
        let adapter = adapter.with_publisher(publisher);

        let game_server = GameServer {
            region: "eu".to_owned(),
            game: "Test Server".to_owned(),
            mode: "Test Mode".to_owned(),
            server_pub: "127.0.0.1:3456".to_owned(),
            server_priv: "127.0.0.1:3456".to_owned(),
            healthy: true,
            min_players: 2,
            max_players: 2,
        };
        let uuid = adapter.insert(game_server.clone()).unwrap();

        adapter.remove(&uuid).unwrap();

        let game: Result<DBGameServer, Box<dyn Error>> = adapter.get(&uuid);
        assert!(game.is_err());
    }

    #[test]
    fn test_redis_adapter_update_game_server() {
        use gn_matchmaking_state::adapters::redis::publisher::native::RedisInfoPublisher;
        use gn_matchmaking_state::adapters::redis::{RedisAdapter};
        use super::*;



        let adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();
        let publisher = RedisInfoPublisher::new(adapter.client.get_connection().unwrap());
        let adapter = adapter.with_publisher(publisher);


        let game_server = GameServer {
            region: "eu".to_owned(),
            game: "Test Server".to_owned(),
            mode: "Test Mode".to_owned(),
            server_pub: "127.0.0.1:3456".to_owned(),
            server_priv: "127.0.0.1:3456".to_owned(),
            healthy: true,
            min_players: 2,
            max_players: 2,
        };
        let uuid = adapter.insert(game_server.clone()).unwrap();

        let mut update = GameServerUpdater::default();
        update.game = Some("CSS Battle (Cum Sum Sus Battle)".to_owned());
        adapter.update(&uuid, update).unwrap();

        let result: DBGameServer = adapter.get(&uuid).unwrap();

        assert!(result.game == "CSS Battle (Cum Sum Sus Battle)");
    }
}
