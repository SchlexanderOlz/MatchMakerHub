use std::{future::Future, sync::Arc};

use models::{
    AIPlayerRegister, CreateMatch, CreatedMatch, GameServerCreate, MatchAbrubtClose, MatchResult,
    Task,
};

pub mod models;
pub mod rabbitmq;

pub trait MessageHandler<T, Fut>: Fn(T) -> Fut + Send + Sync + 'static + Clone {}

impl<T, Fut, F> MessageHandler<T, Fut> for F where F: Fn(T) -> Fut + Send + Sync + 'static + Clone {}

/// Handles communication between the game server and the matchmaker.
/// Structs which implement the Communicator trait enable multi-device communication.
/// This means that if a callback is registered with an "on"-function, it may be called when any device calls a "report" or "send" function.
pub trait Communicator
where
    Self: Sized,
{
    /// Registers a callback for when a match is abruptly closed.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function that handles `MatchAbrubtClose` events.
    async fn on_match_abrupt_close<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<MatchAbrubtClose, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;

    /// Registers a callback for when a match result is reported.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function that handles `MatchResult` events.
    async fn on_match_result<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<MatchResult, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;

    /// Registers a callback for when a match is created.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function that handles `CreatedMatch` events.
    async fn on_match_created<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<CreatedMatch, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;

    /// Registers a callback for when a game server is created.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function that handles `GameServerCreate` events.
    async fn on_game_create<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<GameServerCreate, Fut>,
        Fut: Future<Output = String> + Send + Sync + 'static;

    /// Registers a callback for health check events.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function that handles health check events.
    async fn on_health_check<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<String, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;

    /// Registers a callback for when a match creation request is received.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function that handles `CreateMatch` events.
    async fn on_match_create<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<CreateMatch, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;

    /// Registers a callback for when a new ai-player is registered.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function that handles `AIPlayerRegister` events.
    async fn on_ai_register<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<AIPlayerRegister, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;

    /// Creates a game on the game server.
    ///
    /// # Arguments
    ///
    /// * `game_server` - The game server creation request.
    ///
    /// # Returns
    ///
    /// A result containing the game server ID or an error.
    async fn create_game(
        &self,
        game_server: &GameServerCreate,
    ) -> Result<String, Box<dyn std::error::Error>>;

    /// Sends a health check to the specified client.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The ID of the client to send the health check to.
    async fn send_health_check(&self, client_id: String);

    /// Creates a match based on the provided match request.
    ///
    /// # Arguments
    ///
    /// * `match_request` - The match creation request.
    async fn create_match(&self, match_request: &CreateMatch);

    /// Reports that a match has been created.
    ///
    /// # Arguments
    ///
    /// * `created_match` - The created match information.
    async fn report_match_created(&self, created_match: &CreatedMatch);

    /// Reports the result of a match.
    ///
    /// # Arguments
    ///
    /// * `match_result` - The match result information.
    async fn report_match_result(&self, match_result: &MatchResult);

    /// Reports that a match was abruptly closed.
    ///
    /// # Arguments
    ///
    /// * `match_close` - The abrupt match close information.
    async fn report_match_abrupt_close(&self, match_close: &MatchAbrubtClose);

    /// Sends a message to create a new AI-Task.
    ///
    /// # Arguments
    ///
    /// * `task` - The AI task information.
    async fn create_ai_task(&self, task: &Task);
    
    /// Sends a message to create a new AI-Player.
    ///
    /// # Arguments
    ///
    /// * `ai_player` - AI-Player Information.
    async fn register_ai_player(&self, ai_player: &AIPlayerRegister);
}
