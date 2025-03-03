use utoipa::{
    openapi::{
        self,
        security::{Http, HttpAuthScheme, SecurityScheme},
    },
    Modify, OpenApi,
};

use crate::models;


#[derive(OpenApi)]
#[openapi(
    paths(
        super::handler::active_matches::get_active_matches,
        super::handler::active_matches::get_active_match_by_uuid,
        super::handler::active_matches::leave,
        super::handler::active_matches::get_write_token,

        super::handler::game_servers::get_game_servers,
        super::handler::game_servers::get_game_server_by_uuid,

        super::handler::host_requests::get_host_requests,
        super::handler::host_requests::get_host_request_by_uuid,

        super::handler::ai_players::get_ai_players,
        super::handler::ai_players::get_ai_player_by_uuid,
    ),
    components(
        schemas(
            utoipa::TupleUnit,
            models::ActiveMatch,
            models::ActiveMatchFilter,
            models::GameServer,
            models::GameServerFilter,
            models::HostRequest,
            models::HostRequestFilter,
        )
    ),
    tags((name = "Matchmaking State API", description = "Accessing matchmaking-state runtime-values")),
)]
pub struct ApiDoc;
