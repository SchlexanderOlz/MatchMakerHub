use std::collections::HashMap;
use itertools::Itertools;

use gn_matchmaking_state::models::{ActiveMatchDB, GameServer};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct CreatedMatch {
    pub region: String,
    pub player_write: HashMap<String, String>,
    pub game: String,
    pub mode: String,
    pub ai: bool,
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
    pub mode: String,
    pub ai: bool,
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
            healthy: true,
        }
    }
}


#[derive(Deserialize, Debug, Clone)]
pub struct Ranking {
    pub performances: HashMap<String, Vec<String>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MatchResult {
    pub match_id: String,
    pub winners: HashMap<String, u8>,
    pub losers: HashMap<String, u8>,
    pub ranking: Ranking,
}

pub struct MatchResultMaker(MatchResult, ActiveMatchDB);

impl From<(MatchResult, ActiveMatchDB)> for MatchResultMaker {
    fn from(x: (MatchResult, ActiveMatchDB)) -> Self {
        MatchResultMaker(x.0, x.1)
    }
}

impl Into<gn_ranking_client_rs::models::create::Match> for MatchResultMaker {
    fn into(self) -> gn_ranking_client_rs::models::create::Match {
        let active_match = self.1;
        let result = self.0;

        gn_ranking_client_rs::models::create::Match {
        game_name: active_match.game.clone(),
        game_mode: active_match.mode.clone(),
        player_match_list: result
            .ranking
            .performances
            .into_iter()
            .map(
                |(player_id, performances)| gn_ranking_client_rs::models::create::PlayerMatch {
                    player_id,
                    player_performances: performances
                        .into_iter()
                        .counts()
                        .into_iter()
                        .map(
                            |x| gn_ranking_client_rs::models::create::PlayerPerformance {
                                name: x.0,
                                count: x.1 as i32,
                            },
                        )
                        .collect(),
                },
            )
            .collect(),
    }
    }
}