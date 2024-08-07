use core::fmt;

use matchmaking_state::{adapters::MatchHandler, models::GameMode};
use pool::GameServerPool;

mod pool;

pub struct Match {
    pub address: String,
    pub game: String,
    pub players: Vec<String>,
    pub mode: GameMode,
}

pub struct MatchMaker {
    pub servers: GameServerPool
}

#[derive(Debug)]
pub enum MatchingError {
    ServerNotFound,
}

impl fmt::Display for MatchingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Server not found")
    }
}


impl MatchMaker {
    pub fn create(&self, match_info: Match) -> Result<(), MatchingError> {
        let server = self.servers.get_server_by_address(match_info.address);
        let server = match server {
            Some(server) => server,
            None => return Err(MatchingError::ServerNotFound) 
        };

        // TODO: Create a game on the server per TCP-Connection via the Game-Servers API using JSON 

        Ok(())
    }
}