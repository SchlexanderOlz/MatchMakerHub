use gn_communicator::Communicator;
use gn_matchmaking_state_types::DBSearcher;
use std::{collections::HashMap, sync::Arc};
use tokio::runtime::Runtime;
use tracing::{debug, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use gn_matchmaking_state::models::Match;

use gn_matchmaking_state::prelude::*;
use serde::{Deserialize, Serialize};

mod model;

fn handle_match(
    new_match: Match,
    conn: Arc<RedisAdapterDefault>,
) -> gn_communicator::models::CreateMatch {
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
            }
            Err(err) => {
                warn!("Player not found: {}", err);
                ai_players.push(player_id.clone());
                player_id
            }
        })
        .collect();

    gn_communicator::models::CreateMatch {
        game: new_match.game,
        players,
        ai_players,
        mode: new_match.mode.clone().into(),
        ai: new_match.ai,
    }
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let connector = RedisAdapter::connect(&redis_url).expect("Could not connect to Redis database");

    let redis_connection = connector.client.get_connection().unwrap();
    let connector = Arc::new(connector.with_publisher(RedisInfoPublisher::new(redis_connection)));

    let amqp_url = std::env::var("AMQP_URL").expect("AMQP_URL must be set");
    let communicator =
        Arc::new(gn_communicator::rabbitmq::RabbitMQCommunicator::connect(&amqp_url).await);

    info!("Started pool auto-update");
    info!("Started match check");

    let match_checker = connector.clone();
    connector.clone().on_match(move |new_match| {
        info!("New match: {:?}", new_match);

        let connector = connector.clone();

        let created_match = handle_match(new_match, connector.clone());
        let communicator = communicator.clone();
        tokio::spawn(async move {
            communicator.create_match(&created_match).await;
        });
    });
    info!("On match handler registered");
    match_checker.start_match_check().await.unwrap();
}
