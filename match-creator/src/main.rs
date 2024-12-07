use lapin::{
    options::{BasicPublishOptions, QueueDeclareOptions},
    types::FieldTable,
    BasicProperties, Channel, Connection,
};
use reqwest;
use std::{collections::HashMap, sync::Arc};
use tracing::{debug, info, Level};
use tracing_subscriber::FmtSubscriber;

use gn_matchmaking_state::models::{ActiveMatch, DBSearcher, Match};
use gn_matchmaking_state::prelude::*;
use serde::{Deserialize, Serialize};

mod model;

#[derive(Serialize)]
struct NewMatch {
    pub game: String,
    pub players: Vec<String>,
    pub ai_players: Vec<String>,
    pub mode: String,
    pub ai: bool,
}

const CREATE_QUEUE: &str = "match-create-request";

async fn handle_match(new_match: Match, channel: &Channel, conn: Arc<RedisAdapterDefault>) {
    debug!("Matched players: {:?}", new_match);

    // TODO: Write the logic for the pool to look up the existence of the game (If needed. Else remove the pool)

    let mut ai_players = Vec::new();

    let players: Vec<_> = new_match
        .players
        .into_iter()
        .map(|player_id| match conn.get(&player_id) {
            Ok(player) => {
                let player: DBSearcher = player;
                player.player_id
            },
            Err(_) => {
                ai_players.push(player_id.clone());
                player_id
            }
        })
        .collect();

    let create_match = NewMatch {
        game: new_match.game,
        players,
        ai_players,
        mode: new_match.mode.clone().into(),
        ai: new_match.ai,
    };

    info!(
        "Requesting creation of match on server: {}",
        new_match.region
    );

    channel
        .basic_publish(
            "",
            CREATE_QUEUE,
            BasicPublishOptions::default(),
            &serde_json::to_vec(&create_match).unwrap(),
            BasicProperties::default(),
        )
        .await
        .unwrap();
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let connector = RedisAdapter::connect(&redis_url).expect("Could not connect to Redis database");

    let amqp_url = std::env::var("AMQP_URL").expect("AMQP_URL must be set");
    let amqp_connection = Connection::connect(&amqp_url, lapin::ConnectionProperties::default())
        .await
        .expect("Could not connect to AMQP server");
    let create_channel = Arc::new(amqp_connection.create_channel().await.unwrap());
    create_channel
        .queue_declare(
            CREATE_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let redis_connection = connector.client.get_connection().unwrap();
    let connector = Arc::new(connector.with_publisher(RedisInfoPublisher::new(redis_connection)));

    info!("Started pool auto-update");
    info!("Started match check");

    let match_checker = connector.clone();
    connector.clone().on_match(move |new_match| {
        info!("New match: {:?}", new_match);
        let create_channel = create_channel.clone();

        let connector = connector.clone();
        let _ = tokio::spawn(async move {
            handle_match(new_match, &create_channel, connector.clone()).await;
        });
    });
    info!("On match handler registered");
    match_checker.start_match_check().await.unwrap();
}
