use ezauth::EZAUTHValidationResponse;
use gn_matchmaking_state::adapters::{
    redis::RedisAdapterDefault, Gettable, Insertable, Removable, Updateable,
};
use gn_matchmaking_state_types::{
    ActiveMatchDB, DBGameServer, DBSearcher, GameServer, HostRequest, HostRequestDB,
    HostRequestUpdate, Searcher,
};
use rand::{distributions::Alphanumeric, Rng};
use std::{
    f32::consts::E,
    fmt::{self, write},
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tracing::{debug, info, warn};
use uuid::Uuid;

use axum::body::Bytes;
use socketioxide::extract::SocketRef;

use crate::models::{Host, JoinPriv, JoinPub, Match, Search};

const DEFAULT_ELO: u32 = 1250;

#[derive(Debug)]
pub enum HandlerError {
    HostingNotStarted,
    NoServerOnline,
    NoServerFound,
    PlayerAlreadyHosting(HostRequestDB),
    PlayerUnauthorized,
    PlayerNotAllowedToStart,
    PlayerAlreadyJoined,
    NotEnoughPlayers,
    MatchAlreadyStarted,
    PlayerAlreadyPlaying(ActiveMatchDB),
    MatchIsFull,
    InvalidJoinToken,
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for HandlerError {}

/// The `Handler` struct manages matchmaking operations, including searching for matches,
/// hosting matches, joining matches, and starting matches. It interacts with a redis-database for state management and an external ranking client for player ELO ratings.
pub struct Handler {
    search: Mutex<Option<Search>>,
    state: Arc<RedisAdapterDefault>,
    search_id: Mutex<Option<String>>,
    ezauth_response: Mutex<Option<EZAUTHValidationResponse>>,
    ezauth_url: String,
    ranking_client: Arc<gn_ranking_client_rs::RankingClient>,
}

impl Handler {
    /// Creates a new `Handler` instance.
    ///
    /// # Arguments
    ///
    /// * `state` - An `Arc` containing the `RedisAdapterDefault` instance.
    /// * `ranking_client` - An `Arc` containing the `RankingClient` instance.
    ///
    /// # Returns
    ///
    /// A new `Handler` instance.
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

    /// Retrieves the searcher ID if available.
    ///
    /// # Returns
    ///
    /// An `Option` containing the searcher ID.
    #[inline]
    pub fn get_searcher_id(&self) -> Option<String> {
        self.search_id.lock().unwrap().clone()
    }

    /// Retrieves the user ID from the EZAUTH response if available.
    ///
    /// # Returns
    ///
    /// An `Option` containing the user ID.
    #[inline]
    pub fn get_user_id(&self) -> Option<String> {
        self.ezauth_response
            .lock()
            .unwrap()
            .as_ref()
            .map(|x| x._id.clone())
    }

    /// Retrieves the ELO rating for a player.
    ///
    /// # Arguments
    ///
    /// * `validation` - A reference to the `EZAUTHValidationResponse`.
    /// * `game` - The game name.
    /// * `mode` - The game mode.
    ///
    /// # Returns
    ///
    /// A `Result` containing the ELO rating or an error.
    #[inline]
    async fn get_elo(
        &self,
        validation: &ezauth::EZAUTHValidationResponse,
        game: &str,
        mode: &str,
    ) -> u32 {
        #[cfg(disable_elo)]
        return DEFAULT_ELO;
        #[cfg(not(disable_elo))]
        return match self
            .ranking_client
            .player_stars(&validation._id, game, mode)
            .await
        {
            Ok(stars) => stars as u32,
            Err(e) => {
                warn!("Failed to get player stars: {:?}", e);
                DEFAULT_ELO
            }
        };
    }

    /// Checks for active game servers in the redis-database that match the specified criteria.
    ///
    /// # Arguments
    ///
    /// * `game` - The game name.
    /// * `mode` - The game mode.
    /// * `region` - The game region.
    ///
    /// # Returns
    ///
    /// A vector of `DBGameServer` instances that match the criteria.
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

    /// Handles a search request for a match.
    ///
    /// # Arguments
    ///
    /// * `data` - The search data.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    pub async fn handle_search(&self, data: Search) -> Result<(), HandlerError> {
        debug!("Received Search event: {:?}", data);

        let validation = self.authorize(&data.session_token).await?;

        if data.allow_reconnect {
            if let Some(active_match) = self
                .state
                .all()
                .unwrap()
                .find(|x: &ActiveMatchDB| x.player_write.contains_key(&validation._id))
            {
                return Err(HandlerError::PlayerAlreadyPlaying(active_match));
            }
        }

        let servers = self.check_for_active_servers(&data.game, &data.mode, &data.region);

        debug!("Servers found for search ({:?}): {:?}", data, servers);

        if servers.is_empty() {
            return Err(HandlerError::NoServerOnline);
        }

        let elo = self.get_elo(&validation, &data.game, &data.mode).await;

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

    /// Handles a request to start a match. A host-request hast to be registered befor this can be called.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    pub async fn handle_start(&self) -> Result<(), HandlerError> {
        let validation = self.ezauth_response.lock().unwrap().clone();

        if validation.is_none() {
            return Err(HandlerError::PlayerUnauthorized);
        }

        let validation = validation.clone().unwrap();

        let search_id = self.search_id.lock().unwrap().clone();
        if search_id.is_none() {
            return Err(HandlerError::HostingNotStarted);
        }

        let search_id = search_id.unwrap();

        let host: HostRequestDB = self.state.get(&search_id).unwrap();

        if host.player_id != validation._id {
            return Err(HandlerError::PlayerNotAllowedToStart);
        }

        if host.joined_players.len() < host.min_players as usize {
            return Err(HandlerError::NotEnoughPlayers);
        }

        self.start(&search_id).await;

        Ok(())
    }

    async fn start(&self, search_id: &str) {
        let update = HostRequestUpdate {
            start_requested: Some(true),
            ..Default::default()
        };

        self.state.update(search_id, update).unwrap();
    }

    /// Handles a request to host a match.
    ///
    /// # Arguments
    ///
    /// * `data` - The host data.
    ///
    /// # Returns
    ///
    /// A `Result` containing the join token or an error.
    pub async fn handle_host(&self, data: Host) -> Result<String, HandlerError> {
        let validation = self.authorize(&data.session_token).await?;

        let servers = self.check_for_active_servers(&data.game, &data.mode, &data.region);

        debug!("Servers found for host-request ({:?}): {:?}", data, servers);

        if servers.is_empty() {
            return Err(HandlerError::NoServerFound);
        }

        let search = data;
        let join_token = if search.public {
            String::new()
        } else {
            Uuid::new_v4().to_string()
        };

        if let Some(host_request) = self
            .state
            .all()
            .unwrap()
            .find(|x: &HostRequestDB| x.player_id == validation._id)
        {
            return Err(HandlerError::PlayerAlreadyHosting(host_request));
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

    /// Get the user-information from ezauth using the session token.
    ///
    /// # Arguments
    ///
    /// * `session_token` - The session token.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `EZAUTHValidationResponse` or an error.
    async fn authorize(
        &self,
        session_token: &str,
    ) -> Result<EZAUTHValidationResponse, HandlerError> {
        if let Some(response) = self.ezauth_response.lock().unwrap().clone() {
            return Ok(response);
        }

        #[cfg(disable_auth)]
        {
            let random_id = Uuid::new_v4().to_string();
            debug!(
                "Authorization disabled, generated random user_id {:?} for session_token {:?}",
                random_id, session_token
            );
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

        let validation = ezauth::validate_user(session_token, &self.ezauth_url).await;

        if validation.is_err() {
            return Err(HandlerError::PlayerUnauthorized);
        }

        let validation = validation.unwrap();

        self.ezauth_response
            .lock()
            .unwrap()
            .replace(validation.clone());

        debug!("Player authorized: {:?}", validation);

        Ok(validation)
    }

    /// Handles a request to join a private match.
    ///
    /// # Arguments
    ///
    /// * `data` - The join data.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    pub async fn handle_join_priv(&self, data: JoinPriv) -> Result<(), HandlerError> {
        if data.join_token.is_empty() {
            return Err(HandlerError::InvalidJoinToken);
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

    /// Handles a request to join a public match.
    ///
    /// # Arguments
    ///
    /// * `data` - The join data.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    pub async fn handle_join_pub(&self, data: JoinPub) -> Result<(), HandlerError> {
        let validation = self.authorize(&data.session_token).await?;

        let host_request: HostRequestDB = self.state.get(&data.host_id).unwrap();
        self.handle_join("", host_request, &validation).await
    }

    /// Handles the common logic for joining a match.
    ///
    /// # Arguments
    ///
    /// * `join_token` - The join token.
    /// * `host_request` - The host request data.
    /// * `validation` - The EZAUTH validation response.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    async fn handle_join(
        &self,
        join_token: &str,
        host_request: HostRequestDB,
        validation: &ezauth::EZAUTHValidationResponse,
    ) -> Result<(), HandlerError> {
        if host_request.start_requested {
            return Err(HandlerError::MatchAlreadyStarted);
        }

        if host_request.joined_players.len() == host_request.max_players as usize {
            return Err(HandlerError::MatchIsFull);
        }

        if host_request.join_token != join_token {
            return Err(HandlerError::InvalidJoinToken);
        }

        if host_request.joined_players.contains(&validation._id) {
            return Err(HandlerError::PlayerAlreadyJoined);
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
        self.state.update(&host_request.uuid, update).unwrap();

        self.search_id
            .lock()
            .unwrap()
            .replace(host_request.uuid.clone());

        if host_request.joined_players.len() + 1 == host_request.max_players as usize {
            self.start(&host_request.uuid).await;
        }

        Ok(())
    }

    /// Removes the current searcher from the state.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    pub fn remove_searcher(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(search_id) = self.search_id.lock().unwrap().as_deref() {
            self.state.remove(search_id)?;
        }
        *self.search.lock().unwrap() = None;
        Ok(())
    }

    /// Removes the current joiner from the state.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
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

    /// Notifies the client that a match has been found.
    ///
    /// # Arguments
    ///
    /// * `socket` - A reference to the `SocketRef`.
    /// * `found_match` - The match data.
    pub fn notify_match_found(&self, socket: &SocketRef, found_match: Match) {
        socket.emit("match", &found_match).unwrap();
    }
}
