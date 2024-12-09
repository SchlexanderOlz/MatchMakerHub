use futures_lite::{Future, StreamExt};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

use gn_matchmaking_state_types::GameServer;
use lapin::{
    message::Delivery,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions,
        QueueDeclareOptions, QueuePurgeOptions,
    },
    types::FieldTable,
    BasicProperties, Channel, Connection,
};
use tracing::{debug, info};

use crate::{
    healthcheck::{self, HealthCheck},
    models::{CreateMatch, CreatedMatch, GameServerCreate, MatchAbrubtClose, MatchResult},
    MessageHandler,
};

const CREATED_MATCH_QUEUE: &str = "match-created";
const CREATE_MATCH_QUEUE: &str = "match-create-request";
const CREATE_GAME_QUEUE: &str = "game-created";
const MATCH_ABRUPT_CLOSE_QUEUE: &str = "match-abrupt-close";
const HEALTH_CHECK_QUEUE: &str = "health-check";
const RESULT_MATCH_QUEUE: &str = "match-result";
const AI_QUEUE: &str = "ai-task-generate-request";
const CREATE_MATCH_REQUEST_QUEUE: &str = "match-create-request";

async fn setup_queue_and_listen<F, Fut>(channel: Arc<Channel>, queue_name: &str, on_message: F)
where
    F: Fn(Arc<Channel>, Delivery) -> Fut,
    Fut: Future<Output = ()> + Send + Sync + 'static,
{
    channel
        .queue_declare(
            queue_name,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let mut consumer = channel
        .basic_consume(
            queue_name,
            &format!("communicator-{}", queue_name),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    info!("Listening on queue: {}", queue_name);
    while let Some(delivery) = consumer.next().await {
        debug!("Received event: {:?}", delivery);
        let channel = channel.clone();
        tokio::spawn(on_message(
            channel.clone(),
            delivery.expect("error in consumer"),
        ));
    }
}

pub struct RabbitMQCommunicator {
    channel: Arc<Channel>,
    uuid: String,
}

impl RabbitMQCommunicator {
    pub async fn connect(amqp_url: &str) -> Self {
        let amqp_connection =
            Connection::connect(&amqp_url, lapin::ConnectionProperties::default())
                .await
                .expect("Could not connect to AMQP server");

        let channel = Arc::new(
            amqp_connection
                .create_channel()
                .await
                .expect("Could not create channel"),
        );

        Self {
            channel,
            uuid: uuid::Uuid::new_v4().to_string(),
        }
    }
}

impl super::Communicator for RabbitMQCommunicator {
    async fn on_match_abrupt_close<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<MatchAbrubtClose, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static,
    {
        setup_queue_and_listen(
            self.channel.clone(),
            MATCH_ABRUPT_CLOSE_QUEUE,
            move |_, delivery| {
                let callback = callback.clone();
                async move {
                    delivery.ack(BasicAckOptions::default()).await.expect("ack");

                    let reason: MatchAbrubtClose = serde_json::from_slice(&delivery.data).unwrap();
                    callback(reason).await;
                }
            },
        ).await;
    }

    async fn on_game_created<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::GameServerCreate, Fut>,
        Fut: Future<Output = String> + Send + Sync + 'static,
    {
        setup_queue_and_listen(
            self.channel.clone(),
            CREATE_GAME_QUEUE,
            move |channel, delivery| {
                let callback = callback.clone();
                async move {
                    let reply_to = match delivery.properties.reply_to() {
                        Some(reply_to) => reply_to.clone(),
                        None => {
                            delivery
                                .nack(BasicNackOptions::default())
                                .await
                                .expect("nack");
                            return;
                        }
                    };
                    debug!("Received game created event: {:?}", delivery);
                    delivery.ack(BasicAckOptions::default()).await.expect("ack");

                    let channel = channel.clone();

                    let created_game: GameServerCreate =
                        serde_json::from_slice(&delivery.data).unwrap();

                    let game_id = callback(created_game).await;

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
                }
            },
        ).await;
    }

    async fn on_match_created<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::CreatedMatch, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static,
    {
        setup_queue_and_listen(
            self.channel.clone(),
            CREATED_MATCH_QUEUE,
            move |channel, delivery| {
                let callback = callback.clone();
                async move {
                    delivery.ack(BasicAckOptions::default()).await.expect("ack");

                    let created_match: CreatedMatch =
                        serde_json::from_slice(&delivery.data).unwrap();

                    callback(created_match).await;
                }
            },
        ).await;
    }

    async fn on_match_result<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::MatchResult, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static,
    {
        setup_queue_and_listen(self.channel.clone(), RESULT_MATCH_QUEUE, |_, delivery| {
            let callback = callback.clone();
            async move {
                delivery.ack(BasicAckOptions::default()).await.expect("ack");

                let result: MatchResult = serde_json::from_slice(&delivery.data).unwrap();
                callback(result).await;
            }
        }).await;
    }

    async fn on_match_create<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::CreateMatch, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static,
    {
        setup_queue_and_listen(
            self.channel.clone(),
            CREATE_MATCH_REQUEST_QUEUE,
            |_, delivery| {
                let callback = callback.clone();
                async move {
                    delivery.ack(BasicAckOptions::default()).await.expect("ack");

                    let result: CreateMatch = serde_json::from_slice(&delivery.data).unwrap();
                    callback(result).await;
                }
            },
        ).await;
    }

    async fn create_game(
        &self,
        game_server: GameServerCreate,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let reply_to = self
            .channel
            .queue_declare("", QueueDeclareOptions::default(), FieldTable::default())
            .await
            .unwrap();

        let mut consumer = self
            .channel
            .basic_consume(
                reply_to.name().as_str(),
                format!("{}@create_game", &self.uuid).as_str(),
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .unwrap();

        self.channel.basic_publish(
            "",
            CREATE_GAME_QUEUE,
            BasicPublishOptions::default(),
            &serde_json::to_vec(&game_server).unwrap(),
            BasicProperties::default()
                .with_reply_to(reply_to.name().clone())
                .with_correlation_id(uuid::Uuid::new_v4().to_string().into()),
        );

        while let Some(delivery) = consumer.next().await {
            let delivery = delivery.unwrap();
            delivery.ack(BasicAckOptions::default()).await.expect("ack");

            let server_id = std::string::String::from_utf8(delivery.data).unwrap();
            self.channel
                .queue_purge(reply_to.name().as_str(), QueuePurgeOptions::default())
                .await
                .unwrap();
            return Ok(server_id);
        }

        Err("Could not create game".into())
    }

    async fn create_match(&self, match_request: CreateMatch) {
        self.channel
            .basic_publish(
                "",
                CREATED_MATCH_QUEUE,
                BasicPublishOptions::default(),
                &serde_json::to_vec(&match_request).unwrap(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn report_match_abrupt_close(&self, match_close: MatchAbrubtClose) {
        self.channel
            .basic_publish(
                "",
                MATCH_ABRUPT_CLOSE_QUEUE,
                BasicPublishOptions::default(),
                &serde_json::to_vec(&match_close).unwrap(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn report_match_created(&self, created_match: CreatedMatch) {
        self.channel
            .basic_publish(
                "",
                CREATE_MATCH_QUEUE,
                BasicPublishOptions::default(),
                &serde_json::to_vec(&created_match).unwrap(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn create_ai_task(&self, task: crate::models::Task) {
        self.channel
            .basic_publish(
                "",
                AI_QUEUE,
                BasicPublishOptions::default(),
                serde_json::to_vec(&task).unwrap().as_slice(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn report_match_result(&self, match_result: MatchResult) {
        self.channel
            .basic_publish(
                "",
                RESULT_MATCH_QUEUE,
                BasicPublishOptions::default(),
                &serde_json::to_vec(&match_result).unwrap(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn send_health_check(&self, client_id: String) {
        self.channel
            .basic_publish(
                "",
                HEALTH_CHECK_QUEUE,
                BasicPublishOptions::default(),
                client_id.as_bytes(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn run_health_checks<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<String, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static,
    {
        setup_queue_and_listen(self.channel.clone(), HEALTH_CHECK_QUEUE, {
            |_, delivery| {
                let callback = callback.clone();
                async move {
                    debug!("Received healthcheck event: {:?}", delivery);
                    delivery.ack(BasicAckOptions::default()).await.expect("ack");

                    let client_id: String = std::string::String::from_utf8(delivery.data).unwrap();
                    callback(client_id).await;
                }
            }
        })
        .await;
    }
}
