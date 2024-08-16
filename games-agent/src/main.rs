// TODO: This entire crate is a temporary implementation. Switch this to a socket based-connecton without http requests as soon as there is time.
use std::sync::Arc;


use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json};
use matchmaking_state::{adapters::{redis::RedisAdapter, Gettable, Insertable}, models::{DBGameServer, GameServer}};

const DEFAULT_URL: &str = "0.0.0.0:7000";



async fn handle_create(
    State(state): State<Arc<RedisAdapter>>,
    Json(body): Json<GameServer>,
) -> impl IntoResponse {
    let servers: Vec<DBGameServer> = state.all().unwrap().collect();

    if servers.iter().find(|x| x.server == body.server && x.name == body.server).is_some() {
        return (StatusCode::BAD_REQUEST, "Server already exists");
    }
    state.insert(body).unwrap();
    (StatusCode::CREATED, "")
}

#[tokio::main]
async fn main() {
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");

    let state = RedisAdapter::connect(&redis_url).unwrap();
    let listener = tokio::net::TcpListener::bind(DEFAULT_URL).await.unwrap();
    let app = axum::Router::new()
        .route("/register", post(handle_create))
        .with_state(Arc::new(state));

    axum::serve(listener, app).await.unwrap();
}
