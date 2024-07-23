use redis::{from_redis_value, FromRedisValue, RedisResult, RedisWrite, ToRedisArgs, Value};
use serde::{Deserialize, Serialize};
use redisadapter_derive::{RedisInsertWriter, RedisOutputReader};

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

#[derive(Debug, Clone)]
pub struct GameMode {
    pub name: String,
    pub player_count: u32,
}

impl ToRedisArgs for GameMode {
    fn write_redis_args<W>(&self, out: &mut W) 
    where
        W: ?Sized + RedisWrite,
    {
        self.name.write_redis_args(out);
        self.player_count.write_redis_args(out);
    }
}

impl FromRedisValue for GameMode {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        let vec: Vec<Value> = from_redis_value(v)?;
        if vec.len() != 2 {
            return Err(redis::RedisError::from((redis::ErrorKind::TypeError, "Expected 2 elements")));
        }

        let name: String = from_redis_value(&vec[0])?;
        let player_count: u32 = from_redis_value(&vec[1])?;

        Ok(GameMode { name, player_count })
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
