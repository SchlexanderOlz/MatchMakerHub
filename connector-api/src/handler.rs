use gn_matchmaking_state::{
    adapters::{redis::RedisAdapterDefault, Gettable, Insertable, Removable, Updateable},
    models::{
        DBGameServer, DBSearcher, GameServer, HostRequest, HostRequestDB, HostRequestUpdate,
        Searcher,
    },
};
use std::{
    f32::consts::E,
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tracing::{debug, info};

use axum::body::Bytes;
use socketioxide::extract::SocketRef;

use crate::{
    ezauth::{self, EZAUTHValidationResponse},
    models::{DirectConnect, Host, Match, Search},
};

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
    fn get_elo(
        &self,
        validation: &ezauth::EZAUTHValidationResponse,
        game: &str,
        mode: &str,
    ) -> Result<u32, Box<dyn std::error::Error + 'static>> {
        #[cfg(disable_elo)]
        return Ok(42);
        #[cfg(not(disable_elo))]
        return Ok(self
            .ranking_client
            .player_stars(&validation._id, game, mode)
            .await?);
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

        let elo = self.get_elo(&validation, &data.game, &data.mode)?;

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
    ) -> Result<(), Box<dyn std::error::Error + 'static>> {
        let validation = self.authorize(&data.session_token).await?;

        let servers = self.check_for_active_servers(&data.game, &data.mode, &data.region);

        debug!("Servers found for host-request ({:?}): {:?}", data, servers);

        if servers.is_empty() {
            return Err("No such server online".into());
        }

        let search = data;

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
            reserved_players: search.reserved_players.clone(),
            joined_players: vec![validation._id.clone()],
            start_requested: false,
            min_players: servers.first().unwrap().min_players,
            max_players: servers.first().unwrap().max_players,
            wait_start: SystemTime::now(),
        };

        let uuid = self.state.insert(host_request).unwrap();
        debug!("Host request inserted with uuid: {}", uuid);
        self.search_id.lock().unwrap().replace(uuid);
        Ok(())
    }

    async fn authorize(
        &self,
        session_token: &str,
    ) -> Result<EZAUTHValidationResponse, Box<dyn std::error::Error + 'static>> {
        if let Some(response) = self.ezauth_response.lock().unwrap().clone() {
            return Ok(response);
        }

        let validation = ezauth::validate_user(session_token, &self.ezauth_url).await?;
        self.ezauth_response
            .lock()
            .unwrap()
            .replace(validation.clone());

        debug!("Player authorized: {:?}", validation);

        Ok(validation)
    }

    pub async fn handle_join(
        &self,
        data: DirectConnect,
    ) -> Result<(), Box<dyn std::error::Error + 'static>> {
        let validation = self.authorize(&data.session_token).await?;

        let host_request: HostRequestDB = self.state.get(&data.host_id)?;

        if host_request.start_requested {
            return Err("Match has already started".into());
        }

        if host_request.joined_players.len() == host_request.max_players as usize {
            return Err("Match is full".into());
        }

        if !host_request.reserved_players.is_empty()
            && !host_request.reserved_players.contains(&validation._id)
        {
            return Err("Player is not allowed to join".into());
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
        self.state.update(&data.host_id, update)?;

        self.search_id.lock().unwrap().replace(data.host_id.clone());

        if host_request.joined_players.len() + 1 == host_request.max_players as usize {
            self.start(&data.host_id).await?;
        }

        Ok(())
    }

    pub fn handle_disconnect(&self) {
        if let Some(search_id) = self.search_id.lock().unwrap().as_deref() {
            self.state.remove(search_id).unwrap();
        }
        *self.search.lock().unwrap() = None;
    }

    pub fn notify_match_found(&self, socket: &SocketRef, found_match: Match) {
        socket.emit("match", &found_match).unwrap();
    }
}
