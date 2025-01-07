mod handler;
mod models;
mod swagger_docs;

use actix_web::{web, App, HttpServer};
use tracing::info;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::FmtSubscriber;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::swagger_docs::ApiDoc;

fn config(conf: &mut web::ServiceConfig) {
    let scope = web::scope("/active-matches")
        .service(handler::active_matches::get_active_matches)
        .service(handler::active_matches::get_active_match_by_uuid)
        .service(handler::active_matches::get_write_token);
    conf.service(scope);

    let scope = web::scope("/game-servers")
        .service(handler::game_servers::get_game_server_by_uuid)
        .service(handler::game_servers::get_game_servers);
    conf.service(scope);

    let scope = web::scope("/host-requests")
        .service(handler::host_requests::get_host_request_by_uuid)
        .service(handler::host_requests::get_host_requests);
    conf.service(scope);

    let scope = web::scope("/ai-players")
        .service(handler::ai_players::get_ai_player_by_uuid)
        .service(handler::ai_players::get_ai_players);
    conf.service(scope);
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    let host_url = std::env::var("HOST_URL").expect("HOST_URL must be set");

    info!("Starting HTTP server at: {}", host_url);

    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let client = web::Data::new(
        gn_matchmaking_state::adapters::redis::RedisAdapterDefault::connect(&redis_url)
            .expect("Failed to connect to Redis"),
    );

    HttpServer::new(move || {
        App::new()
            .app_data(client.clone())
            .wrap(TracingLogger::default())
            .configure(config)
            .service(
                SwaggerUi::new("/docs/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
    })
    .bind(host_url)?
    .run()
    .await
}
