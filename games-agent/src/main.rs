use gn_communicator::Communicator;
use gn_ranking_client_rs::RankingClient;
use lazy_static::lazy_static;
use models::MatchResultMaker;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use gn_communicator::rabbitmq::RabbitMQCommunicator;
use gn_matchmaking_state::prelude::*;
use gn_matchmaking_state_types::{ActiveMatch, ActiveMatchDB, DBGameServer, GameServer};
use healthcheck::HealthCheck;
use tracing::{debug, error, warn, Level};
use tracing_subscriber::FmtSubscriber;

mod healthcheck;
mod models;

lazy_static! {
    static ref ranking_client: RankingClient =
        RankingClient::new(std::env::var("RANKING_API_KEY").unwrap().to_owned());
    static ref communicator: RabbitMQCommunicator = {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(RabbitMQCommunicator::connect(
                std::env::var("AMQP_URL").unwrap().as_str(),
            ))
    };
}



async fn on_match_created(
    created_match: gn_communicator::models::CreatedMatch,
    conn: Arc<RedisAdapterDefault>,
) {
    debug!("Match created: {:?}", created_match);

    let insert = ActiveMatch {
        region: created_match.region,
        game: created_match.game.clone(),
        mode: created_match.mode.clone(),
        ai: created_match.ai,
        server_pub: created_match.url_pub.clone(),
        server_priv: created_match.url_priv.clone(),
        read: created_match.read.clone(),
        player_write: created_match.player_write.clone(),
    };

    debug!("Inserting match {:?} into State", created_match.read);
    conn.insert(insert).unwrap();
    debug!("Match {:?} inserted", created_match.read.clone());

    if created_match.ai {
        for player in created_match.ai_players {
            let task = gn_communicator::models::Task {
                ai_level: 1,
                game: created_match.game.clone(),
                mode: created_match.mode.clone(),
                address: created_match.url_priv.clone(),
                read: created_match.read.clone(),
                write: created_match.player_write.get(&player).unwrap().clone(),
                players: created_match
                    .player_write
                    .keys()
                    .map(|x| x.clone())
                    .collect(),
            };

            communicator.create_ai_task(&task).await;
        }
        debug!("AI tasks created for match {:?}", created_match.read);
    }
}

async fn on_match_abrupt_close(
    reason: gn_communicator::models::MatchAbrubtClose,
    conn: Arc<RedisAdapterDefault>,
) {
    debug!("Match closed abruptly: {:?}", reason);

    let match_ = conn
        .all()
        .unwrap()
        .find(|x: &ActiveMatchDB| x.read.clone() == reason.match_id);

    if let Some(match_) = match_ {
        conn.remove(&match_.uuid).unwrap();
        debug!("Match {:?} removed", match_.uuid);
    }
}

async fn on_match_result(result: gn_communicator::models::MatchResult, conn: Arc<RedisAdapterDefault>) {
    debug!("Match result: {:?}", result);

    let match_ = conn
        .all()
        .unwrap()
        .find(|x: &ActiveMatchDB| x.read.clone() == result.match_id);

    if let Some(match_) = match_ {
        conn.remove(&match_.uuid).unwrap();
        debug!("Match {:?} removed", match_.uuid);
        if let Err(err) = report_match_result(result.clone(), match_.clone()).await {
            error!("Error reporting match result: {:?}", err);
            return;
        }

        debug!(
            "Match {:?} successfully reported to ranking system",
            match_.uuid
        );
    }
}

async fn report_match_result(
    result: gn_communicator::models::MatchResult,
    active_match: ActiveMatchDB,
) -> Result<gn_ranking_client_rs::models::read::Match, Box<dyn std::error::Error>> {
    let request: gn_ranking_client_rs::models::create::Match =
        MatchResultMaker::from((result, active_match)).into();

    ranking_client.match_init(request).await
}

async fn init_game_ranking(
    created_game: gn_communicator::models::GameServerCreate,
) -> Result<gn_ranking_client_rs::models::read::Game, Box<dyn std::error::Error>> {
    debug!(
        "Initializing game at ranking server: {:?}",
        created_game.game
    );
    let game = gn_ranking_client_rs::models::create::Game {
        game_name: created_game.game.clone(),
        game_mode: created_game.mode.clone(),
        max_stars: created_game.ranking_conf.max_stars,
        description: created_game.ranking_conf.description.clone(),
        performances: created_game
            .ranking_conf
            .performances
            .into_iter()
            .map(|x| 
                gn_ranking_client_rs::models::create::Performance {
                    name: x.name,
                    weight: x.weight,
                }
            )
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

async fn listen_for_match_abrupt_close(conn: Arc<RedisAdapterDefault>) {
    communicator.on_match_abrupt_close(
        move |close: gn_communicator::models::MatchAbrubtClose| 
        {
            let conn = conn.clone();
            on_match_abrupt_close(close, conn.clone())
        }
    ).await;
}

async fn listen_for_match_result(conn: Arc<RedisAdapterDefault>) {
    communicator.on_match_result(
        move |result: gn_communicator::models::MatchResult| 
        {
            let conn = conn.clone();
            on_match_result(result, conn.clone())
        }
    ).await;
}

async fn listen_for_match_created(conn: Arc<RedisAdapterDefault>) {
    communicator.on_match_created(
        move |created_match: gn_communicator::models::CreatedMatch| 
        {
            let conn = conn.clone();
            async move {
                on_match_created(created_match, conn.clone()).await;
            }
        }
    ).await;
}

async fn listen_for_game_created(conn: Arc<RedisAdapterDefault>) {
    communicator.on_game_create(
        move |created_game: gn_communicator::models::GameServerCreate| 
        {
            let conn = conn.clone();
            async move {
                let game_id = save_game(created_game.clone().into(), conn.clone())
                    .await
                    .unwrap();

                if let Err(err) = init_game_ranking(created_game).await {
                    error!("Error initializing game at ranking server: {:?}", err);
                }

                game_id
            }
        }
    ).await;
}

async fn listen_for_healthcheck(conn: Arc<RedisAdapterDefault>) {
    let healthcheck = Arc::new(Mutex::new(HealthCheck::new(conn.clone())));

    {
        let healthcheck = healthcheck.clone();
        tokio::task::spawn_blocking(move || loop {
            thread::sleep(Duration::from_secs(1));
            healthcheck.lock().unwrap().check();
        });
    }

    communicator.on_health_check(
        move |client_id: String| 
        {
            let healthcheck = healthcheck.clone();
            async move {
                healthcheck.lock().unwrap().refresh(client_id);
            }
        }
    ).await;
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");

    let state = RedisAdapter::connect(&redis_url).unwrap();
    let connection = state.client.get_connection().unwrap();
    let state = Arc::new(state.with_publisher(RedisInfoPublisher::new(connection)));

    let listen_for_match_created = {
        let state = state.clone();
        tokio::spawn(listen_for_match_created(state.clone()))
    };

    let listen_for_game_created = {
        let state = state.clone();
        tokio::spawn(listen_for_game_created(state.clone()))
    };

    let listen_for_healthcheck = {
        let state = state.clone();
        tokio::spawn(listen_for_healthcheck(state.clone()))
    };

    let listen_for_match_result = {
        let state = state.clone();
        tokio::spawn(listen_for_match_result(state.clone()))
    };

    let listen_for_match_abrupt_close = {
        let state = state.clone();
        tokio::spawn(listen_for_match_abrupt_close(state.clone()))
    };

    listen_for_healthcheck.await.unwrap();
    listen_for_match_created.await.unwrap();
    listen_for_match_result.await.unwrap();
    listen_for_game_created.await.unwrap();
    listen_for_match_abrupt_close.await.unwrap();
}
