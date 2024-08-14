use reqwest;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use matchmaking_state::{
    adapters::{redis::RedisAdapter, Insertable, Matcher},
    models::{ActiveMatch, Match},
};
use serde::{Deserialize, Serialize};
mod pool;

#[derive(Serialize)]
struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}

impl From<matchmaking_state::models::GameMode> for GameMode {
    fn from(value: matchmaking_state::models::GameMode) -> Self {
        Self {
            name: value.name,
            player_count: value.player_count,
            computer_lobby: value.computer_lobby,
        }
    }
}

#[derive(Serialize)]
struct NewMatch {
    pub game: String,
    pub players: Vec<String>,
    pub mode: GameMode,
}

#[derive(Deserialize)]
struct Keys {
    read: String,
    write: String,
}

#[derive(Deserialize)]
struct CreatedMatch {
    pub match_id: String,
    pub player_keys: HashMap<String, Keys>,
}

async fn handle_match(new_match: Match, pool: pool::GameServerPool) {
    println!("Matched players: {:?}", new_match);
    let server = pool
        .get_server_by_address(&new_match.address)
        .expect("Server not found");
    let create_match = NewMatch {
        game: new_match.game,
        players: new_match.players,
        mode: new_match.mode.clone().into(),
    };

    let client = reqwest::Client::new();
    let res = client
        .post(server.server.as_str())
        .json(&create_match)
        .send()
        .await
        .expect("Could not send request");

    let created: CreatedMatch = res.json().await.expect("Could not parse response");

    let insert = ActiveMatch {
        match_id: created.match_id,
        game: create_match.game,
        mode: new_match.mode,
        server: new_match.address,
        player_read: created
            .player_keys
            .iter()
            .map(|(key, val)| (key.clone(), val.read.clone()))
            .collect(),
        player_write: created
            .player_keys
            .iter()
            .map(|(key, val)| (key.clone(), val.write.clone()))
            .collect(),
    };

    pool.get_connection()
        .lock()
        .unwrap()
        .insert(insert)
        .unwrap();
}

fn main() {
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let connector = Arc::new(Mutex::new(
        RedisAdapter::connect(&redis_url).expect("Could not connect to Redis database"),
    ));
    let mut pool = pool::GameServerPool::new(connector.clone());
    pool.populate();
    pool.start_auto_update();

    connector.lock().unwrap().on_match(move |new_match| {
        let pool = pool.clone();
        let _ = tokio::spawn(async move {
            handle_match(new_match, pool).await;
        });
    });
}
