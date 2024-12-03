use std::collections::HashMap;

use gn_matchmaking_state::models::{GameMode, GameServer};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct CreatedMatch {
    pub region: String,
    pub player_write: HashMap<String, String>,
    pub game: String,
    pub mode: GameMode,
    pub read: String,
    pub url_pub: String,
    pub url_priv: String,
}


#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Performance {
    pub name: String,
    pub weight: i32,
}

impl Into<gn_ranking_client_rs::models::create::Performance> for Performance {
    fn into(self) -> gn_ranking_client_rs::models::create::Performance {
        gn_ranking_client_rs::models::create::Performance {
            name: self.name,
            weight: self.weight,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RankingConf {
    pub max_stars: i32,
    pub description: String,
    pub performances: Vec<Performance>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GameServerCreateRequest {
    pub region: String,
    pub game: String,
    pub mode: GameMode,
    pub server_pub: String,
    pub server_priv: String,
    pub token: String, // Token to authorize as the main-server at this game-server
    pub ranking_conf: RankingConf,
}

impl Into<GameServer> for GameServerCreateRequest {
    fn into(self) -> GameServer {
        GameServer {
            region: self.region,
            game: self.game,
            mode: self.mode,
            server_pub: self.server_pub,
            server_priv: self.server_priv,
            token: self.token,
            healthy: true,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct MatchResult {
    pub match_id: String,
    pub winner: String,
    pub points: u8,
    pub ranked: HashMap<String, u8>,
}