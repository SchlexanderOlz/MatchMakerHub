// TODO: This entire crate is a temporary implementation. Switch this to a socket based-connecton without http requests as soon as there is time.
use std::sync::Arc;


use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json};
use matchmaking_state::{adapters::{redis::RedisAdapter, Gettable, Insertable}, models::{DBGameServer, GameServer}};
use tracing::{info, warn};
use tracing_subscriber::FmtSubscriber;

const DEFAULT_URL: &str = "0.0.0.0:7000";



async fn handle_create(
    State(state): State<Arc<RedisAdapter>>,
    Json(body): Json<GameServer>,
) -> impl IntoResponse {
    info!("Trying to create server: {:?}", body);
    let servers: Vec<DBGameServer> = state.all().unwrap().collect();

    if servers.iter().find(|x| x.server.clone() == body.server.clone() && x.name.clone() == body.server.clone()).is_some() {
        warn!("Tried to create a server that already exists. Creation skipped");
        return (StatusCode::BAD_REQUEST, "Server already exists");
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
        .route("/register", post(handle_create))
        .with_state(Arc::new(state));

    info!("Server listening on {}", DEFAULT_URL);

    axum::serve(listener, app).await.unwrap();
}
