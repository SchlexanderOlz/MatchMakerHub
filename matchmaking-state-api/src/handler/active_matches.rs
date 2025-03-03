use std::sync::Arc;

use actix_web::{delete, get, http::StatusCode, post, web, Error, HttpRequest, HttpResponse};
use gn_matchmaking_state::adapters::{redis::Commands, Gettable, Removable};
use gn_matchmaking_state_types::ActiveMatchDB;
use tracing::debug;

use crate::models::{ActiveMatch, ActiveMatchFilter, Filter, GameServer, GameServerFilter};

#[utoipa::path(
    context_path = "/active-matches",
    responses(
        (status = 200, description = "List of currently active matches according to the filter", body = Vec<ActiveMatch>),
        (status = 409, description = "Invalid Request Format")
    ),
    params(
        ActiveMatchFilter
    )
)]
#[get("/")]
async fn get_active_matches(
    req: HttpRequest,
    client: web::Data<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    filter: web::Query<ActiveMatchFilter>,
) -> Result<HttpResponse, Error> {
    let matches: Vec<ActiveMatch> =
        super::filter::<_, ActiveMatchDB, _>((*client).clone(), &*filter).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(matches))
}

#[utoipa::path(
    context_path = "/active-matches",
    responses(
        (status = 204, description = "Left game"),
    ),
    params(
        ("session_token" = String, Path, description = "Session token of user"),
        ("read" = String, Path, description = "Write token of the match"),
    )
)]
#[delete("/{read}/{session_token}")]
async fn leave(
    req: HttpRequest,
    state: web::Data<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    session_token: web::Path<String>,
    read: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let validation = ezauth::validate_user(&session_token, &super::EZAUTH_URL).await?;

    let mut filter = ActiveMatchFilter::default();
    filter.player = Some(validation._id.clone());
    filter.read = Some(read.clone());

    let matches: Vec<ActiveMatchDB> =
        super::filter::<_, ActiveMatchDB, _>((*state).clone(), &filter).await?;
    let write = matches
        .first()
        .ok_or_else(|| actix_web::error::ErrorNotFound("No active matches found".to_string()))?
        .player_write
        .get(&validation._id);

    if let Some(write) = write {
        state.remove(matches.first().unwrap().uuid.as_str()).map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Failed to remove match: {}", e))
        })?;
    }

    Ok(HttpResponse::Ok().finish())
}


#[utoipa::path(
    context_path = "/active-matches",
    responses(
        (status = 200, description = "Active Match with the requested uuid", body = ActiveMatch),
        (status = 409, description = "Invalid Request Format")
    ),
    params(
        ("uuid" = String, Path, description = "UUID of the match")
    )
)]
#[get("/{uuid}")]
async fn get_active_match_by_uuid(
    req: HttpRequest,
    client: web::Data<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    uuid: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let m: ActiveMatchDB = client.get(&uuid).map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to fetch match: {}", e))
    })?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(ActiveMatch::from(m)))
}

#[utoipa::path(
    context_path = "/active-matches",
    responses(
        (status = 200, description = "Write token for a user in an active match", body = String),
        (status = 409, description = "Invalid Request Format")
    ),
    params(
        ("session_token" = String, Path, description = "Session token of user"),
        ("read" = String, Path, description = "Write token of the match"),
    )
)]
#[get("/{read}/{session_token}")]
async fn get_write_token(
    req: HttpRequest,
    state: web::Data<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    session_token: web::Path<String>,
    read: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let validation = ezauth::validate_user(&session_token, &super::EZAUTH_URL).await?;

    let mut filter = ActiveMatchFilter::default();
    filter.player = Some(validation._id.clone());
    filter.read = Some(read.clone());

    let matches: Vec<ActiveMatchDB> =
        super::filter::<_, ActiveMatchDB, _>((*state).clone(), &filter).await?;
    let write = matches
        .first()
        .ok_or_else(|| actix_web::error::ErrorNotFound("No active matches found".to_string()))?
        .player_write
        .get(&validation._id)
        .unwrap();

    Ok(HttpResponse::Ok().body(write.clone()))
}

