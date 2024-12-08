use std::sync::Arc;

use actix_web::{delete, get, http::StatusCode, post, web, Error, HttpRequest, HttpResponse};
use gn_matchmaking_state::adapters::{redis::Commands, Gettable};
use gn_matchmaking_state_types::{ActiveMatchDB, DBGameServer, HostRequestDB};

use crate::models::{ActiveMatch, ActiveMatchFilter, GameServer, GameServerFilter, HostRequest, HostRequestFilter};

#[utoipa::path(
    context_path = "/host-requests",
    responses(
        (status = 200, description = "List of currently active matches according to the filter", body = Vec<HostRequest>),
        (status = 409, description = "Invalid Request Format")
    ),
    params(
        HostRequestFilter
    )
)]
#[get("/")]
async fn get_host_requests(
    req: HttpRequest,
    client: web::Data<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    filter: web::Query<HostRequestFilter>,
) -> Result<HttpResponse, Error> {
    let servers: Vec<HostRequest> =
        super::filter::<_, HostRequestDB, _>((*client).clone(), &*filter).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(servers))
}


#[utoipa::path(
    context_path = "/host-requests",
    responses(
        (status = 200, description = "Host Request with the requested uuid", body = HostRequest),
        (status = 409, description = "Invalid Request Format")
    ),
    params(
        ("uuid" = String, Path, description = "UUID of the match")
    )
)]
#[get("/{uuid}")]
async fn get_host_request_by_uuid(
    req: HttpRequest,
    client: web::Data<gn_matchmaking_state::adapters::redis::RedisAdapterDefault>,
    uuid: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let m: HostRequestDB = client.get(&uuid).map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to fetch match: {}", e))
    })?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(HostRequest::from(m)))
}