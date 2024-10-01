// TODO: This entire crate is a temporary implementation. Switch this to a socket based-connecton without http requests as soon as there is time.
use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{connect_info::IntoMakeServiceWithConnectInfo, ConnectInfo, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json,
};
use gn_matchmaking_state::{
    adapters::{redis::RedisAdapter, Gettable, Insertable},
    models::{DBGameServer, GameMode, GameServer},
};
use serde::Deserialize;
use tower::ServiceBuilder;
use tower_http::{cors::{Any, CorsLayer}, trace::TraceLayer};
use tracing::{info, warn};
use tracing_subscriber::FmtSubscriber;

const DEFAULT_URL: &str = "0.0.0.0:7000";



async fn handle_create(
    State(state): State<Arc<RedisAdapter>>,
    Json(body): Json<GameServer>,
) -> impl IntoResponse {
    info!("Trying to create server: {:?}", body);
    let mut servers = state.all().unwrap();

    if servers.find(|x: &DBGameServer| x.server_pub.clone() == body.server_pub.clone() && x.name.clone() == body.name.clone()).is_some() {
        warn!("Tried to create a server that already exists. Creation skipped");
        return (StatusCode::OK, "Server already exists");
    }
    state.insert(body.clone()).unwrap();
    info!("Successfully Created server: {:?}", body);
    (StatusCode::CREATED, "")
}

#[tokio::main]
async fn main() {
    tracing::subscriber::set_global_default(FmtSubscriber::default()).unwrap();
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");

    let state = RedisAdapter::connect(&redis_url).unwrap();
    let listener = tokio::net::TcpListener::bind(DEFAULT_URL).await.unwrap();
    let app = axum::Router::new()
        .layer(CorsLayer::new().allow_origin(Any))
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()))
        .route("/register", post(handle_create))
        .with_state(Arc::new(state));

    info!("Server listening on {}", DEFAULT_URL);

    axum::serve(listener, app).await.unwrap();
}
