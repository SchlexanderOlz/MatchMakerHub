use futures_lite::StreamExt;
use gn_ranking_client_rs::RankingClient;
use lazy_static::lazy_static;
use models::{CreatedMatch, GameServerCreateRequest, MatchResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use gn_matchmaking_state::models::{
    ActiveMatch, ActiveMatchDB, DBGameServer, GameMode, GameServer,
};
use gn_matchmaking_state::prelude::*;
use healthcheck::HealthCheck;
use lapin::options::{
    BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions,
    QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Channel, Connection};
use serde::Deserialize;
use tracing::{debug, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

const CREATE_MATCH_QUEUE: &str = "match-created";
const CREATE_GAME_QUEUE: &str = "game-created";
const HEALTH_CHECK_QUEUE: &str = "health-check";
const RESULT_MATCH_QUEUE: &str = "match-result";

mod healthcheck;
mod models;

lazy_static! {
    static ref ranking_client: RankingClient =
        RankingClient::new(std::env::var("RANKING_API_KEY").unwrap().to_owned());
}

async fn on_match_created(created_match: CreatedMatch, conn: Arc<RedisAdapterDefault>) {
    debug!("Match created: {:?}", created_match);

    let insert = ActiveMatch {
        region: created_match.region,
        game: created_match.game,
        mode: created_match.mode,
        server_pub: created_match.url_pub,
        server_priv: created_match.url_priv,
        read: created_match.read.clone(),
        player_write: created_match.player_write,
    };

    debug!("Inserting match {:?} into State", created_match.read);
    conn.insert(insert).unwrap();
    debug!("Match {:?} inserted", created_match.read);
}

async fn on_match_result(result: MatchResult, conn: Arc<RedisAdapterDefault>) {
    debug!("Match result: {:?}", result);
    let mut matches = conn.all().unwrap();
    if let Some(match_) = matches.find(|x: &ActiveMatchDB| x.read.clone() == result.match_id) {
        conn.remove(&match_.uuid).unwrap();
        debug!("Match {:?} removed", match_.uuid);
    }
}

async fn init_game_ranking(
    created_game: GameServerCreateRequest,
) -> Result<gn_ranking_client_rs::models::read::Game, Box<dyn std::error::Error>> {
    debug!("Initializing game at ranking server: {:?}", created_game.game);
    let game = gn_ranking_client_rs::models::create::Game {
        game_name: created_game.game.clone(),
        game_mode: created_game.mode.name.clone(),
        max_stars: created_game.ranking_conf.max_stars,
        description: created_game.ranking_conf.description.clone(),
        performances: created_game
            .ranking_conf
            .performances
            .into_iter()
            .map(|x| x.into())
            .collect(),
    };
    Ok(ranking_client.game_init(game).await?)
}

async fn save_game(
    created_game: GameServer,
    conn: Arc<RedisAdapterDefault>,
) -> Result<String, Box<dyn std::error::Error>> {
    debug!("Trying to create server: {:?}", created_game);

    if let Some(server) = conn.all().unwrap().find(|x: &DBGameServer| {
        x.server_pub.clone() == created_game.server_pub.clone()
            && x.game.clone() == created_game.game.clone()
    }) {
        warn!("Tried to create a server that already exists. Creation skipped");
        return Ok(server.uuid);
    }

    let server = conn.insert(created_game.clone()).unwrap();
    debug!("Successfully Created server: {:?}", created_game);
    Ok(server)
}

async fn listen_for_match_result(channel: Arc<Channel>, conn: Arc<RedisAdapterDefault>) {
    channel
        .queue_declare(
            RESULT_MATCH_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let mut consumer = channel
        .basic_consume(
            RESULT_MATCH_QUEUE,
            "games-agent-match-result",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    info!("Listening for match result events");
    while let Some(delivery) = consumer.next().await {
        debug!("Received match result event: {:?}", delivery);
        let conn = conn.clone();
        tokio::spawn(async move {
            let delivery = delivery.expect("error in consumer");
            delivery.ack(BasicAckOptions::default()).await.expect("ack");

            let result: MatchResult = serde_json::from_slice(&delivery.data).unwrap();
            on_match_result(result, conn.clone()).await;
        });
    }
}

async fn listen_for_match_created(channel: Arc<Channel>, conn: Arc<RedisAdapterDefault>) {
    channel
        .queue_declare(
            CREATE_MATCH_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let mut consumer = channel
        .basic_consume(
            CREATE_MATCH_QUEUE,
            "games-agent-create-match",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    info!("Listening for match created events");
    while let Some(delivery) = consumer.next().await {
        debug!("Received match created event: {:?}", delivery);
        let conn = conn.clone();
        tokio::spawn(async move {
            let delivery = delivery.expect("error in consumer");
            delivery.ack(BasicAckOptions::default()).await.expect("ack");

            let created_match: CreatedMatch = serde_json::from_slice(&delivery.data).unwrap();
            on_match_created(created_match, conn.clone()).await;
        });
    }
}

async fn listen_for_game_created(channel: Arc<Channel>, conn: Arc<RedisAdapterDefault>) {
    channel
        .queue_declare(
            CREATE_GAME_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let mut consumer = channel
        .basic_consume(
            CREATE_GAME_QUEUE,
            "games-agent-create-game",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    info!("Listening for game created events");
    while let Some(delivery) = consumer.next().await {
        let delivery = delivery.expect("error in consumer");

        let reply_to = match delivery.properties.reply_to() {
            Some(reply_to) => reply_to.clone(),
            None => {
                delivery
                    .nack(BasicNackOptions::default())
                    .await
                    .expect("nack");
                continue;
            }
        };
        debug!("Received game created event: {:?}", delivery);
        delivery.ack(BasicAckOptions::default()).await.expect("ack");

        let conn = conn.clone();
        let channel = channel.clone();

        tokio::spawn(async move {
            let created_game: GameServerCreateRequest =
                serde_json::from_slice(&delivery.data).unwrap();
            let game_id = save_game(created_game.clone().into(), conn.clone())
                .await
                .unwrap();

            init_game_ranking(created_game).await.unwrap();

            channel
                .basic_publish(
                    "",
                    reply_to.as_str(),
                    BasicPublishOptions::default(),
                    game_id.as_bytes(),
                    BasicProperties::default(),
                )
                .await
                .unwrap();
        });
    }
}

async fn listen_for_healthcheck(channel: Arc<Channel>, conn: Arc<RedisAdapterDefault>) {
    channel
        .queue_declare(
            HEALTH_CHECK_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let mut consumer = channel
        .basic_consume(
            HEALTH_CHECK_QUEUE,
            "games-agent-healthcheck",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let healthcheck = Arc::new(Mutex::new(HealthCheck::new(conn)));

    {
        let healthcheck = healthcheck.clone();
        tokio::task::spawn_blocking(move || loop {
            thread::sleep(Duration::from_secs(1));
            healthcheck.lock().unwrap().check();
        });
    }

    info!("Listening for healthcheck events");
    while let Some(delivery) = consumer.next().await {
        let healthcheck = healthcheck.clone();
        tokio::spawn(async move {
            debug!("Received healthcheck event: {:?}", delivery);
            let delivery = delivery.expect("error in consumer");
            delivery.ack(BasicAckOptions::default()).await.expect("ack");

            let client_id: String = std::string::String::from_utf8(delivery.data).unwrap();
            healthcheck.lock().unwrap().refresh(client_id);
        });
    }
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let amqp_url = std::env::var("AMQP_URL").expect("AMQP_URL must be set");
    let amqp_connection = Connection::connect(&amqp_url, lapin::ConnectionProperties::default())
        .await
        .expect("Could not connect to AMQP server");

    let state = RedisAdapter::connect(&redis_url).unwrap();
    let connection = state.client.get_connection().unwrap();
    let state = Arc::new(state.with_publisher(RedisInfoPublisher::new(connection)));

    let amqp_channel = Arc::new(amqp_connection.create_channel().await.unwrap());

    let listen_for_match_created = {
        let state = state.clone();
        let channel = amqp_channel.clone();
        tokio::spawn(listen_for_match_created(channel, state.clone()))
    };

    let listen_for_game_created = {
        let state = state.clone();
        let channel = amqp_channel.clone();
        tokio::spawn(listen_for_game_created(channel, state.clone()))
    };

    let listen_for_healthcheck = {
        let state = state.clone();
        let channel = amqp_channel.clone();
        tokio::spawn(listen_for_healthcheck(channel, state.clone()))
    };

    let listen_for_match_result = {
        let state = state.clone();
        let channel = amqp_channel.clone();
        tokio::spawn(listen_for_match_result(channel, state.clone()))
    };

    listen_for_healthcheck.await.unwrap();
    listen_for_match_created.await.unwrap();
    listen_for_match_result.await.unwrap();
    listen_for_game_created.await.unwrap();
}
