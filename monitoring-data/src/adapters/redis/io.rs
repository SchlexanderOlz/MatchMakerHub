// NOTE: The content of this file is, in best case, temporary and will be rewritten to automated proc-macros
use redis::{Connection};

use crate::models::{self, GameMode};

use super::{RedisNameable, RedisOutputReader};


impl RedisNameable for models::DBGameServer {
    fn name() -> String {
        "game_server".to_owned()
    }
}

impl RedisOutputReader for models::DBGameServer {
    fn read(connection: &mut Connection, base_key: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut pipe = redis::pipe();
        pipe.atomic();
        pipe.get(format!("{base_key}:name"))
            .get(format!("{base_key}:modes:name"))
            .get(format!("{base_key}:modes:player_count"))
            .get(format!("{base_key}:server"))
            .get(format!("{base_key}:token"));

        let res: (String, Vec<String>, Vec<u32>, String, String) = pipe.query(connection)?;
        Ok(Self {
            uuid: base_key.to_owned(),
            name: res.0,
            modes: res
                .1
                .into_iter()
                .zip(res.2)
                .map(|x| GameMode {
                    name: x.0,
                    player_count: x.1,
                })
                .collect(),
            server: res.3,
            token: res.4,
        })
    }
}

