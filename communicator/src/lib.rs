use std::{future::Future, sync::Arc};

use gn_matchmaking_state::prelude::RedisAdapterDefault;
use models::{CreateMatch, CreatedMatch, GameServerCreate, MatchAbrubtClose, MatchResult, Task};

pub mod rabbitmq;
pub mod models;
pub mod healthcheck;

pub trait MessageHandler<T, Fut>:  Fn(T) -> Fut + Send + Sync + 'static + Clone 
{}


pub trait Communicator 
where Self: Sized
{
    async fn on_match_abrupt_close<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<MatchAbrubtClose, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;

    async fn on_match_result<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<MatchResult, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;
    
    async fn on_match_created<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<CreatedMatch, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;
    
    async fn on_game_created<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<GameServerCreate, Fut>,
        Fut: Future<Output = String> + Send + Sync + 'static;
    
    async fn run_health_checks<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<String, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;

    async fn on_match_create<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<CreateMatch, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static;
    
    async fn create_game(&self, game_server: GameServerCreate) -> Result<String, Box<dyn std::error::Error>>;
    async fn send_health_check(&self, client_id: String);
    async fn create_match(&self, match_request: CreateMatch);
    async fn report_match_created(&self, created_match: CreatedMatch);
    async fn report_match_result(&self, match_result: MatchResult);
    async fn report_match_abrupt_close(&self, match_close: MatchAbrubtClose);
    async fn create_ai_task(&self, task: Task);
}

