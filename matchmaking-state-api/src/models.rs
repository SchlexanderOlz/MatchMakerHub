use gn_matchmaking_state_types::ActiveMatchDB;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};


#[derive(ToSchema, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ActiveMatch {
    pub uuid: String,
    pub game: String,
    pub mode: String,
    pub ai: bool,
    pub address: String,
    pub region: String,
    pub read: String,
    pub players: Vec<String>
}

impl From<ActiveMatchDB> for ActiveMatch {
    fn from(am: ActiveMatchDB) -> Self {
        ActiveMatch {
            uuid: am.uuid,
            game: am.game,
            mode: am.mode,
            ai: am.ai,
            address: am.server_pub,
            region: am.region,
            read: am.read,
            players: am.player_write.keys().cloned().collect()
        }
    }
}

impl Filter<ActiveMatchFilter> for gn_matchmaking_state_types::ActiveMatchDB {
    fn matches(&self, filter: &ActiveMatchFilter) -> bool {
        if let Some(game) = &filter.game {
            if self.game != *game {
                return false;
            }
        }

        if let Some(mode) = &filter.mode {
            if self.mode != *mode {
                return false;
            }
        }

        if let Some(ai) = filter.ai {
            if self.ai != ai {
                return false;
            }
        }

        if let Some(region) = &filter.region {
            if self.region != *region {
                return false;
            }
        }

        if let Some(read) = &filter.read {
            if self.read != *read {
                return false;
            }
        }

        if let Some(player) = &filter.player {
            if !self.player_write.contains_key(player) {
                return false;
            }
        }

        true
    }
}

#[derive(ToSchema, Serialize, Deserialize, Debug, PartialEq, Eq, IntoParams, Default)]
pub struct ActiveMatchFilter {
    pub game: Option<String>,
    pub mode: Option<String>,
    pub ai: Option<bool>,
    pub region: Option<String>,
    pub read: Option<String>,
    pub player: Option<String>
}

#[derive(ToSchema, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct GameServer {
    pub uuid: String,
    pub region: String,
    pub game: String,
    pub mode: String,
    pub address: String,
    pub healthy: bool,
    pub min_players: u32,
    pub max_players: u32,
}


#[derive(ToSchema, Serialize, Deserialize, Debug, PartialEq, Eq, IntoParams, Default)]
pub struct GameServerFilter {
    pub region: Option<String>,
    pub game: Option<String>,
    pub mode: Option<String>,
    pub healthy: Option<bool>,
    pub min_players: Option<u32>,
    pub max_players: Option<u32>,
}


impl From<gn_matchmaking_state_types::DBGameServer> for GameServer {
    fn from(gs: gn_matchmaking_state_types::DBGameServer) -> Self {
        GameServer {
            uuid: gs.uuid,
            region: gs.region,
            game: gs.game,
            mode: gs.mode,
            address: gs.server_pub,
            healthy: gs.healthy,
            min_players: gs.min_players,
            max_players: gs.max_players,
        }
    }
}


impl Filter<GameServerFilter> for gn_matchmaking_state_types::DBGameServer {
    fn matches(&self, filter: &GameServerFilter) -> bool {
        if let Some(region) = &filter.region {
            if self.region != *region {
                return false;
            }
        }

        if let Some(game) = &filter.game {
            if self.game != *game {
                return false;
            }
        }

        if let Some(mode) = &filter.mode {
            if self.mode != *mode {
                return false;
            }
        }

        if let Some(healthy) = filter.healthy {
            if self.healthy != healthy {
                return false;
            }
        }

        if let Some(min_players) = filter.min_players {
            if self.min_players < min_players {
                return false;
            }
        }

        if let Some(max_players) = filter.max_players {
            if self.max_players > max_players {
                return false;
            }
        }

        true
    }
}

pub trait Filter<F> {
    fn matches(&self, filter: &F) -> bool;
}

#[derive(ToSchema, Serialize, Deserialize, Debug, PartialEq, Eq, IntoParams, Default)]
pub struct HostRequest {
    pub uuid: String,
    pub host_player_id: String,
    pub mode: String,
    pub game: String,
    pub region: String,
    pub joined_players: Vec<String>,
    pub start_requested: bool,
    pub min_players: u32,
    pub max_players: u32,
}


#[derive(ToSchema, Serialize, Deserialize, Debug, PartialEq, Eq, IntoParams, Default)]
pub struct HostRequestFilter {
    pub host_player_id: Option<String>,
    pub mode: Option<String>,
    pub game: Option<String>,
    pub region: Option<String>,
    pub is_public: Option<bool>,
    pub joined_players: Option<Vec<String>>,
    pub start_requested: Option<bool>,
    pub min_players: Option<u32>,
    pub max_players: Option<u32>,
}

impl From<gn_matchmaking_state_types::HostRequestDB> for HostRequest {
    fn from(hr: gn_matchmaking_state_types::HostRequestDB) -> Self {
        HostRequest {
            uuid: hr.uuid,
            host_player_id: hr.player_id,
            mode: hr.mode,
            game: hr.game,
            region: hr.region,
            joined_players: hr.joined_players,
            start_requested: hr.start_requested,
            min_players: hr.min_players,
            max_players: hr.max_players,
        }
    }
}

impl Filter<HostRequestFilter> for gn_matchmaking_state_types::HostRequestDB {
    fn matches(&self, filter: &HostRequestFilter) -> bool {
        if let Some(host_player_id) = &filter.host_player_id {
            if self.player_id != *host_player_id {
                return false;
            }
        }

        if let Some(is_public) = filter.is_public {
            if self.join_token.is_empty() != is_public {
                return false;
            }
        }

        if let Some(mode) = &filter.mode {
            if self.mode != *mode {
                return false;
            }
        }

        if let Some(game) = &filter.game {
            if self.game != *game {
                return false;
            }
        }

        if let Some(region) = &filter.region {
            if self.region != *region {
                return false;
            }
        }


        if let Some(joined_players) = &filter.joined_players {
            if self.joined_players != *joined_players {
                return false;
            }
        }

        if let Some(start_requested) = filter.start_requested {
            if self.start_requested != start_requested {
                return false;
            }
        }

        if let Some(min_players) = filter.min_players {
            if self.min_players < min_players {
                return false;
            }
        }

        if let Some(max_players) = filter.max_players {
            if self.max_players > max_players {
                return false;
            }
        }

        true
    }
}

#[derive(ToSchema, Serialize, Deserialize, Debug, PartialEq, Eq, IntoParams, Default)]
pub struct AIPlayer {
    pub game: String,
    pub mode: String,
    pub elo: u32,
    pub display_name: String,
}

#[derive(ToSchema, Serialize, Deserialize, Debug, PartialEq, Eq, IntoParams, Default)]
pub struct AIPlayerFilter {
    pub game: Option<String>,
    pub mode: Option<String>,
    pub elo_ge: Option<u32>,
    pub elo_le: Option<u32>,
    pub display_name: Option<String>,
}

impl From<gn_matchmaking_state_types::AIPlayerDB> for AIPlayer {
    fn from(ai: gn_matchmaking_state_types::AIPlayerDB) -> Self {
        AIPlayer {
            game: ai.game,
            mode: ai.mode,
            elo: ai.elo,
            display_name: ai.display_name,
        }
    }
}

impl Filter<AIPlayerFilter> for gn_matchmaking_state_types::AIPlayerDB {
    fn matches(&self, filter: &AIPlayerFilter) -> bool {
        if let Some(game) = &filter.game {
            if self.game != *game {
                return false;
            }
        }

        if let Some(mode) = &filter.mode {
            if self.mode != *mode {
                return false;
            }
        }

        if let Some(elo_ge) = filter.elo_ge {
            if self.elo <= elo_ge {
                return false;
            }
        }

        if let Some(elo_le) = filter.elo_le {
            if self.elo >= elo_le {
                return false;
            }
        }

        if let Some(display_name) = &filter.display_name {
            if self.display_name != *display_name {
                return false;
            }
        }

        true
    }
}
