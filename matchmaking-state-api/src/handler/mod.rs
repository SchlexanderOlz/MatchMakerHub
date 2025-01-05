pub mod active_matches;
pub mod game_servers;
pub mod ai_players;
pub mod host_requests;


use std::sync::Arc;

use actix_web::{delete, get, http::StatusCode, post, web, Error, HttpRequest, HttpResponse};
use gn_matchmaking_state::adapters::{redis::{Commands, RedisIdentifiable, RedisOutputReader}, Gettable};
use gn_matchmaking_state_types::ActiveMatchDB;
use lazy_static::lazy_static;

use crate::models::{ActiveMatch, ActiveMatchFilter, Filter};


lazy_static! {
    pub static ref EZAUTH_URL: String = std::env::var("EZAUTH_URL").unwrap();
}

async fn filter<T, D, F>(
    state: Arc<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    filter: &F,
) -> Result<Vec<T>, Error> 
where D: RedisOutputReader + RedisIdentifiable + Filter<F>,
      T: From<D>
{
    Ok(state.all().map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to fetch matches: {}", e))
    })?
    .filter_map(|m: D| {
        if m.matches(&filter) {
            Some(m.into())
        } else {
            None
        }
    })
    .collect())
}