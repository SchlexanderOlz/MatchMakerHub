use redis::{from_redis_value, FromRedisValue, RedisResult, RedisWrite, ToRedisArgs, Value};
use serde::{Deserialize, Serialize};
use redisadapter_derive::{RedisInsertWriter, RedisOutputReader};

use crate::adapters::Filter;

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

impl Filter<DBGameServer> for GameServerFilter {
    fn is_ok(&self, check: &DBGameServer) -> bool {
        if self.game.is_none() {
            return true;
        }
        return self.game.clone().unwrap() == check.name;
    }
}

#[derive(Debug, Clone)]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}


// TODO: Switch to more convient method if possible
impl ToRedisArgs for GameMode {
    fn write_redis_args<W>(&self, out: &mut W) 
    where
        W: ?Sized + RedisWrite,
    {

        out.write_arg(vec![
            self.name.as_bytes(),
            &[0x0],
            self.player_count.to_be_bytes().as_ref(),
            &[self.computer_lobby as u8],
        ].concat().as_slice());
    }
}

// TODO: Switch to more convient method if possible
impl FromRedisValue for GameMode {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        let vec: Vec<u8> = from_redis_value(v)?;

        println!("{:?}", vec);

        let idx = vec.iter().position(|x| *x == 0x0).unwrap().clone();
        let name: String = unsafe { String::from_utf8_unchecked(vec[..idx].to_vec()) };
        let player_count: u32 = u32::from_be_bytes([vec[idx], vec[idx + 1], vec[idx + 2], vec[idx + 3]]);
        let computer_lobby: bool = vec[idx + 4] == 0x1;

        Ok(GameMode { name, player_count, computer_lobby })
    }
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
