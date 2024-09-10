use reqwest;
use std::{collections::HashMap, sync::Arc};
use tracing::{debug, info, Level};
use tracing_subscriber::FmtSubscriber;

use gn_matchmaking_state::{
    adapters::{redis::RedisAdapter, Gettable, Insertable, Matcher},
    models::{ActiveMatch, DBSearcher, Match},
};
use serde::{Deserialize, Serialize};
mod pool;

#[derive(Serialize)]
struct GameMode {
    pub name: String,
    pub player_count: u32,
    pub computer_lobby: bool,
}

impl From<gn_matchmaking_state::models::GameMode> for GameMode {
    fn from(value: gn_matchmaking_state::models::GameMode) -> Self {
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

#[derive(Deserialize, Debug)]
struct CreatedMatch {
    pub player_write: HashMap<String, String>,
    pub read: String,
    pub url: String,
}

async fn handle_match(new_match: Match, pool: pool::GameServerPool) {
    debug!("Matched players: {:?}", new_match);

    // TODO: Write the logic for the pool to look up the existence of the game (If needed. Else remove the pool)

    let create_match = NewMatch {
        game: new_match.game,
        players: new_match
            .players
            .into_iter()
            .map(|player_id| pool.get_connection().get(&player_id).unwrap())
            .map(|player| {
                let player: DBSearcher = player;
                player.player_id
            })
            .collect(),
        mode: new_match.mode.clone().into(),
    };

    let client = reqwest::Client::new();

    info!(
        "Requesting creation of match on server: {}",
        new_match.address
    );
    let res = client
        .post(format!("http://{}/{}/", new_match.address, new_match.mode.name)) // TODO: This is a temporary sollution. Resolve the hostname here
        .json(&create_match)
        .send()
        .await
        .expect("Could not send request");

    debug!("Response: {:?}", res);
    let created: CreatedMatch = res.json().await.expect("Could not parse response");
    debug!("Match created: {:?}", created);

    let insert = ActiveMatch {
        game: create_match.game,
        mode: new_match.mode,
        server: created.url,
        read: created.read.clone(),
        player_write: created.player_write,
    };

    debug!("Inserting match {:?} into State", created.read);
    pool.get_connection().insert(insert).unwrap();
    debug!("Match {:?} inserted", created.read);
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let connector =
        Arc::new(RedisAdapter::connect(&redis_url).expect("Could not connect to Redis database"));
    info!("Creating game-server pool");
    let mut pool = pool::GameServerPool::new(connector.clone());
    pool.populate();
    pool.start_auto_update();

    info!("Started pool auto-update");
    info!("Started match check");
    connector.on_match(move |new_match| {
        info!("New match: {:?}", new_match);
        let pool = pool.clone();
        let _ = tokio::spawn(async move {
            handle_match(new_match, pool).await;
        });
    });
    info!("On match handler registered");
    connector.start_match_check().await.unwrap();
}
