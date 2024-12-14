use ezauth::EZAUTHValidationResponse;
use gn_matchmaking_state::adapters::{
    redis::RedisAdapterDefault, Gettable, Insertable, Removable, Updateable,
};
use gn_matchmaking_state_types::{
    DBGameServer, DBSearcher, GameServer, HostRequest, HostRequestDB, HostRequestUpdate, Searcher,
};
use rand::{distributions::Alphanumeric, Rng};
use std::{
    f32::consts::E,
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tracing::{debug, info, warn};
use uuid::Uuid;

use axum::body::Bytes;
use socketioxide::extract::SocketRef;

use crate::models::{Host, JoinPriv, JoinPub, Match, Search};

const DEFAULT_ELO: u32 = 1250;

pub struct Handler {
    search: Mutex<Option<Search>>,
    state: Arc<RedisAdapterDefault>,
    search_id: Mutex<Option<String>>,
    ezauth_response: Mutex<Option<EZAUTHValidationResponse>>,
    ezauth_url: String,
    ranking_client: Arc<gn_ranking_client_rs::RankingClient>,
}

impl Handler {
    pub fn new(
        state: Arc<RedisAdapterDefault>,
        ranking_client: Arc<gn_ranking_client_rs::RankingClient>,
    ) -> Self {
        let ezauth_url = std::env::var("EZAUTH_URL").unwrap();
        let ezauth_url = ezauth_url.to_string();

        Self {
            search: Mutex::new(None),
            state,
            search_id: Mutex::new(None),
            ezauth_url,
            ranking_client,
            ezauth_response: Mutex::new(None),
        }
    }

    #[inline]
    pub fn get_searcher_id(&self) -> Option<String> {
        self.search_id.lock().unwrap().clone()
    }

    #[inline]
    pub fn get_user_id(&self) -> Option<String> {
        self.ezauth_response
            .lock()
            .unwrap()
            .as_ref()
            .map(|x| x._id.clone())
    }

    #[inline]
    async fn get_elo(
        &self,
        validation: &ezauth::EZAUTHValidationResponse,
        game: &str,
        mode: &str,
    ) -> Result<u32, Box<dyn std::error::Error + 'static>> {
        #[cfg(disable_elo)]
        return Ok(DEFAULT_ELO);
        #[cfg(not(disable_elo))]
        return Ok(
            match self
                .ranking_client
                .player_stars(&validation._id, game, mode)
                .await
            {
                Ok(stars) => stars as u32,
                Err(e) => {
                    warn!("Failed to get player stars: {:?}", e);
                    DEFAULT_ELO
                }
            },
        );
    }

    #[inline]
    fn check_for_active_servers(&self, game: &str, mode: &str, region: &str) -> Vec<DBGameServer> {
        self.state
            .all()
            .unwrap()
            .filter(|server: &DBGameServer| {
                server.healthy
                    && server.game == game
                    && server.mode == mode
                    && server.region == region
            })
            .collect()
    }

    pub async fn handle_search(
        &self,
        data: Search,
    ) -> Result<(), Box<dyn std::error::Error + 'static>> {
        debug!("Received Search event: {:?}", data);

        let validation = self.authorize(&data.session_token).await?;
        let servers = self.check_for_active_servers(&data.game, &data.mode, &data.region);

        debug!("Servers found for search ({:?}): {:?}", data, servers);

        if servers.is_empty() {
            return Err("No such server online".into());
        }

        let elo = self.get_elo(&validation, &data.game, &data.mode).await?;

        let search = data;

        if let Some(searcher) = self
            .state
            .all()
            .unwrap()
            .find(|x: &DBSearcher| x.player_id == validation._id)
        {
            self.search_id.lock().unwrap().replace(searcher.uuid);
            return Ok(());
        }

        let sample_server = servers.first().unwrap();
        let searcher = Searcher {
            player_id: validation._id.clone(),
            elo,
            game: search.game.clone(),
            mode: search.mode.clone(),
            ai: search.ai,
            region: search.region.clone(),
            min_players: sample_server.min_players,
            max_players: sample_server.max_players,
            wait_start: SystemTime::now(),
        };
        let uuid = self.state.insert(searcher).unwrap();
        debug!("Searcher inserted with uuid: {}", uuid);
        self.search_id.lock().unwrap().replace(uuid);
        Ok(())
    }

    pub async fn handle_start(&self) -> Result<(), Box<dyn std::error::Error + 'static>> {
        let validation = self.ezauth_response.lock().unwrap().clone();

        if validation.is_none() {
            return Err("Player has not been authorized".into());
        }

        let validation = validation.clone().unwrap();

        let search_id = self.search_id.lock().unwrap().clone();
        if search_id.is_none() {
            return Err("No hosting has been started".into());
        }

        let search_id = search_id.unwrap();

        let host: HostRequestDB = self.state.get(&search_id)?;

        if host.player_id != validation._id {
            return Err("Player is not allowed to start the game".into());
        }

        if host.joined_players.len() < host.min_players as usize {
            return Err("Not enough players to start the game".into());
        }

        self.start(&search_id).await?;

        Ok(())
    }

    async fn start(&self, search_id: &str) -> Result<(), Box<dyn std::error::Error + 'static>> {
        let update = HostRequestUpdate {
            start_requested: Some(true),
            ..Default::default()
        };

        self.state.update(search_id, update)?;

        Ok(())
    }

    pub async fn handle_host(
        &self,
        data: Host,
    ) -> Result<String, Box<dyn std::error::Error + 'static>> {
        let validation = self.authorize(&data.session_token).await?;

        let servers = self.check_for_active_servers(&data.game, &data.mode, &data.region);

        debug!("Servers found for host-request ({:?}): {:?}", data, servers);

        if servers.is_empty() {
            return Err("No such server online".into());
        }

        let search = data;
        let join_token = if search.public {
            String::new()
        } else {
            Uuid::new_v4().to_string()
        };

        if self
            .state
            .all()
            .unwrap()
            .find(|x: &HostRequestDB| x.player_id == validation._id)
            .is_some()
        {
            return Err("Player is already hosting".into());
        }

        let host_request = HostRequest {
            player_id: validation._id.clone(),
            mode: search.mode.clone(),
            game: search.game.clone(),
            region: search.region.clone(),
            join_token: join_token.clone(),
            joined_players: vec![validation._id.clone()],
            start_requested: false,
            min_players: servers.first().unwrap().min_players,
            max_players: servers.first().unwrap().max_players,
            wait_start: SystemTime::now(),
        };

        let uuid = self.state.insert(host_request).unwrap();
        debug!("Host request inserted with uuid: {}", uuid);
        self.search_id.lock().unwrap().replace(uuid);
        Ok(join_token)
    }

    async fn authorize(
        &self,
        session_token: &str,
    ) -> Result<EZAUTHValidationResponse, Box<dyn std::error::Error + 'static>> {
        if let Some(response) = self.ezauth_response.lock().unwrap().clone() {
            return Ok(response);
        }

        #[cfg(disable_auth)]
        {
            let random_id = Uuid::new_v4().to_string();
            let random_username: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(10)
                .map(char::from)
                .collect();
            let random_email = format!("{}@example.com", random_username);

            let validation = EZAUTHValidationResponse {
                _id: random_id,
                username: random_username,
                email: random_email,
                created_at: "2021-01-01T00:00:00.000Z".to_string(),
            };
            self.ezauth_response
                .lock()
                .unwrap()
                .replace(validation.clone());
            return Ok(validation);
        }

        let validation = ezauth::validate_user(session_token, &self.ezauth_url).await?;
        self.ezauth_response
            .lock()
            .unwrap()
            .replace(validation.clone());

        debug!("Player authorized: {:?}", validation);

        Ok(validation)
    }

    pub async fn handle_join_priv(
        &self,
        data: JoinPriv,
    ) -> Result<(), Box<dyn std::error::Error + 'static>> {
        if data.join_token.is_empty() {
            return Err("Invalid join token".into());
        }

        let validation = self.authorize(&data.session_token).await?;

        let host_request: HostRequestDB = self
            .state
            .all()
            .unwrap()
            .find(|x: &HostRequestDB| x.join_token == data.join_token)
            .unwrap();

        self.handle_join(&data.join_token, host_request, &validation)
            .await
    }

    pub async fn handle_join_pub(
        &self,
        data: JoinPub,
    ) -> Result<(), Box<dyn std::error::Error + 'static>> {
        let validation = self.authorize(&data.session_token).await?;

        let host_request: HostRequestDB = self.state.get(&data.host_id)?;
        self.handle_join("", host_request, &validation).await
    }

    async fn handle_join(
        &self,
        join_token: &str,
        host_request: HostRequestDB,
        validation: &ezauth::EZAUTHValidationResponse,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if host_request.start_requested {
            return Err("Match has started".into());
        }

        if host_request.joined_players.len() == host_request.max_players as usize {
            return Err("Match is full".into());
        }

        if host_request.join_token != join_token {
            return Err("Invalid join token".into());
        }

        if host_request.joined_players.contains(&validation._id) {
            return Err("Player is already in the match".into());
        }

        let mut update = HostRequestUpdate {
            joined_players: Some(host_request.joined_players.clone()),
            ..Default::default()
        };
        update
            .joined_players
            .as_mut()
            .unwrap()
            .push(validation._id.clone());
        self.state.update(&host_request.uuid, update)?;

        self.search_id
            .lock()
            .unwrap()
            .replace(host_request.uuid.clone());

        if host_request.joined_players.len() + 1 == host_request.max_players as usize {
            self.start(&host_request.uuid).await?;
        }

        Ok(())
    }

    pub fn remove_searcher(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(search_id) = self.search_id.lock().unwrap().as_deref() {
            self.state.remove(search_id)?;
        }
        *self.search.lock().unwrap() = None;
        Ok(())
    }

    pub fn remove_joiner(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(host_id) = self.search_id.lock().unwrap().as_deref() {
            let host_request: HostRequestDB = self.state.get(&host_id)?;
            let mut update = HostRequestUpdate {
                joined_players: Some(host_request.joined_players.clone()),
                ..Default::default()
            };
            update
                .joined_players
                .as_mut()
                .unwrap()
                .retain(|x| *x != self.get_user_id().unwrap());
            self.state.update(&host_id, update).unwrap();
        }
        *self.search.lock().unwrap() = None;
        Ok(())
    }

    pub fn notify_match_found(&self, socket: &SocketRef, found_match: Match) {
        socket.emit("match", &found_match).unwrap();
    }
}
