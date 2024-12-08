use std::sync::Arc;

use actix_web::{delete, get, http::StatusCode, post, web, Error, HttpRequest, HttpResponse};
use gn_matchmaking_state::adapters::{redis::Commands, Gettable};
use gn_matchmaking_state_types::{ActiveMatchDB, DBGameServer};

use crate::models::{ActiveMatch, ActiveMatchFilter, GameServer, GameServerFilter};

#[utoipa::path(
    context_path = "/game-servers",
    responses(
        (status = 200, description = "List of currently active matches according to the filter", body = Vec<GameServer>),
        (status = 409, description = "Invalid Request Format")
    ),
    params(
        GameServerFilter
    )
)]
#[get("/")]
async fn get_game_servers(
    req: HttpRequest,
    client: web::Data<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    filter: web::Query<GameServerFilter>,
) -> Result<HttpResponse, Error> {
    let servers: Vec<GameServer> =
        super::filter::<_, DBGameServer, _>((*client).clone(), &*filter).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(servers))
}


#[utoipa::path(
    context_path = "/game-servers",
    responses(
        (status = 200, description = "Game Server with the requested uuid", body = GameServer),
        (status = 409, description = "Invalid Request Format")
    ),
    params(
        ("uuid" = String, Path, description = "UUID of the match")
    )
)]
#[get("/{uuid}")]
async fn get_game_server_by_uuid(
    req: HttpRequest,
    client: web::Data<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    uuid: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let m: DBGameServer = client.get(&uuid).map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to fetch match: {}", e))
    })?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(GameServer::from(m)))
}