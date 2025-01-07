use std::sync::Arc;

use actix_web::{delete, get, http::StatusCode, post, web, Error, HttpRequest, HttpResponse};
use gn_matchmaking_state::adapters::{redis::Commands, Gettable};
use gn_matchmaking_state_types::{AIPlayerDB, ActiveMatchDB, DBGameServer};

use crate::models::{AIPlayer, AIPlayerFilter};

#[utoipa::path(
    context_path = "/ai-players",
    responses(
        (status = 200, description = "List of currently active matches according to the filter", body = Vec<AIPlayer>),
        (status = 409, description = "Invalid Request Format")
    ),
    params(
        AIPlayerFilter 
    )
)]
#[get("/")]
async fn get_ai_players(
    req: HttpRequest,
    client: web::Data<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    filter: web::Query<AIPlayerFilter>,
) -> Result<HttpResponse, Error> {
    let servers: Vec<AIPlayer> =
        super::filter::<_, AIPlayerDB, _>((*client).clone(), &*filter).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(servers))
}


#[utoipa::path(
    context_path = "/ai-players",
    responses(
        (status = 200, description = "Game Server with the requested uuid", body = AIPlayer),
        (status = 409, description = "Invalid Request Format")
    ),
    params(
        ("uuid" = String, Path, description = "UUID of the match")
    )
)]
#[get("/{uuid}")]
async fn get_ai_player_by_uuid(
    req: HttpRequest,
    client: web::Data<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    uuid: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let m: AIPlayerDB = client.get(&uuid).map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to fetch match: {}", e))
    })?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(AIPlayer::from(m)))
}