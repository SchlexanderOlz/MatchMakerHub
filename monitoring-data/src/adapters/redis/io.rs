// NOTE: The content of this file is, in best case, temporary and will be rewritten to automated proc-macros
use redis::{Connection};

use crate::models::{self, GameMode};

use super::{RedisNameable, RedisOutputReader};


impl RedisNameable for models::DBGameServer {
    fn name() -> String {
        "game_server".to_owned()
    }
}

