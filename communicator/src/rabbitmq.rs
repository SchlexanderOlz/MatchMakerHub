use async_recursion::async_recursion;
use futures_lite::{Future, StreamExt};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use lapin::{
    message::Delivery,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions,
        QueueDeclareOptions, QueuePurgeOptions,
    },
    types::FieldTable,
    BasicProperties, Channel, Connection,
};
use tracing::{debug, error, info};

use crate::{
    models::{CreateMatch, CreatedMatch, GameServerCreate, MatchAbrubtClose, MatchResult},
    MessageHandler,
};

async fn setup_queue_and_listen<F, Fut>(channel: Arc<Channel>, queue_name: &str, on_message: F)
where
    F: Fn(Arc<Channel>, Delivery) -> Fut + Send + Sync + 'static,
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
    tokio::spawn(async move {
        while let Some(delivery) = consumer.next().await {
            debug!("Received event: {:?}", delivery);
            let channel = channel.clone();
            tokio::spawn(on_message(
                channel.clone(),
                delivery.expect("error in consumer"),
            ));
        }
    });
}

/// Communicator implementation for RabbitMQ
/// Uses lapin to communicate over AMQP-Messages and Channels
pub struct RabbitMQCommunicator {
    channel: Arc<Channel>,
    uuid: String,
    queues: HashMap<String, HashMap<String, String>>,
}

#[async_recursion]
async fn try_connect(amqp_url: &str) -> Connection {
    match Connection::connect(&amqp_url, lapin::ConnectionProperties::default()).await {
        Ok(res) => res,
        Err(err) => {
            error!("Could not connect to rabbitmq server: {:?}", err);
            tokio::time::sleep(Duration::from_secs(5)).await;
            try_connect(amqp_url).await
        }
    }
}

impl RabbitMQCommunicator {
    /// Connects to the AMQP server and creates a new instance of the struct.
    ///
    /// # Arguments
    ///
    /// * `amqp_url` - A string slice that holds the URL of the AMQP server.
    ///
    /// # Returns
    ///
    /// A new instance of the struct with an established AMQP connection and a created channel.
    ///
    /// # Panics
    ///
    /// This function will panic if it fails to create a channel.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let connection = RabbitMQCommunicator::connect("amqp://localhost:5672").await;
    /// ```
    pub async fn connect(amqp_url: &str) -> Self {
        let amqp_connection = try_connect(amqp_url).await;

        let channel = Arc::new(
            amqp_connection
                .create_channel()
                .await
                .expect("Could not create channel"),
        );

        Self {
            channel,
            uuid: uuid::Uuid::new_v4().to_string(),
            queues: Self::load_default_queues(),
        }
    }

    fn load_default_queues() -> HashMap<String, HashMap<String, String>> {
        let content = include_str!("../queues.yml");
        serde_yaml::from_str(&content).expect("Failed to parse routes file")
    }

    /// Get queue name
    /// # Arguments
    ///
    /// * `name` - A string slice that holds the type of which the queue is. (e.g. "match" or "game")
    ///
    /// * `action` - A string slice that holds the action that is performed on the queue. (e.g. "created" or "result")
    fn get_queue_name(&self, name: &str, action: &str) -> &str {
        self.queues
            .get(name)
            .expect(format!("Queue {} not found", name).as_str())
            .get(action)
            .expect(format!("Action {} not found", action).as_str())
    }

    /// Use a differing queue-configuration file from the default config 'queues.yml'
    ///
    /// # Arguments
    ///
    /// * `path` - A string slice that holds the path to the file.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let communicator = RabbitMQCommunicator::connect("amqp://localhost:5672").await;
    /// communicator.load_queues("custom_queues.yml");
    /// ```
    ///
    /// # Default Queues
    /// ```yaml
    /// match:
    ///  created: "match-created"
    ///  result: "match-result"
    ///  close: "match-abrupt-close"
    ///  create: "match-create-request"
    ///
    /// game:
    ///   register: "game-created"
    ///
    /// health_check:
    ///   check: "health-check"
    ///
    /// ai:
    ///   create_task: "ai-task-generate-request"
    ///   register: "ai-register"
    /// ```
    pub fn load_queues(&mut self, path: &str) {
        let content = std::fs::read_to_string(path).expect("Failed to read file");
        self.queues = serde_yaml::from_str(&content).expect("Failed to parse routes file");
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
            self.get_queue_name("match", "abrupt_close"),
            move |_, delivery| {
                let callback = callback.clone();
                async move {
                    delivery.ack(BasicAckOptions::default()).await.expect("ack");

                    let reason: MatchAbrubtClose = serde_json::from_slice(&delivery.data).unwrap();
                    callback(reason).await;
                }
            },
        )
        .await;
    }

    async fn on_game_create<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::GameServerCreate, Fut>,
        Fut: Future<Output = String> + Send + Sync + 'static,
    {
        setup_queue_and_listen(
            self.channel.clone(),
            self.get_queue_name("game", "create"),
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
        )
        .await;
    }

    async fn on_match_created<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::CreatedMatch, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static,
    {
        setup_queue_and_listen(
            self.channel.clone(),
            self.get_queue_name("match", "created"),
            move |channel, delivery| {
                let callback = callback.clone();
                async move {
                    delivery.ack(BasicAckOptions::default()).await.expect("ack");

                    let created_match: CreatedMatch =
                        serde_json::from_slice(&delivery.data).unwrap();

                    callback(created_match).await;
                }
            },
        )
        .await;
    }

    async fn on_match_result<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::MatchResult, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static,
    {
        setup_queue_and_listen(
            self.channel.clone(),
            self.get_queue_name("match", "result"),
            move |_, delivery| {
                let callback = callback.clone();
                async move {
                    delivery.ack(BasicAckOptions::default()).await.expect("ack");

                    let result: MatchResult = serde_json::from_slice(&delivery.data).unwrap();
                    callback(result).await;
                }
            },
        )
        .await;
    }

    async fn on_match_create<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::CreateMatch, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static,
    {
        setup_queue_and_listen(
            self.channel.clone(),
            self.get_queue_name("match", "create"),
            move |_, delivery| {
                let callback = callback.clone();
                async move {
                    delivery.ack(BasicAckOptions::default()).await.expect("ack");

                    let result: CreateMatch = serde_json::from_slice(&delivery.data).unwrap();
                    callback(result).await;
                }
            },
        )
        .await;
    }

    async fn create_game(
        &self,
        game_server: &GameServerCreate,
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

        self.channel
            .basic_publish(
                "",
                self.get_queue_name("game", "create"),
                BasicPublishOptions::default(),
                &serde_json::to_vec(&game_server).unwrap(),
                BasicProperties::default()
                    .with_reply_to(reply_to.name().clone())
                    .with_correlation_id(uuid::Uuid::new_v4().to_string().into()),
            )
            .await
            .unwrap();

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

    async fn create_match(&self, match_request: &CreateMatch) {
        self.channel
            .basic_publish(
                "",
                self.get_queue_name("match", "create"),
                BasicPublishOptions::default(),
                &serde_json::to_vec(&match_request).unwrap(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn report_match_abrupt_close(&self, match_close: &MatchAbrubtClose) {
        self.channel
            .basic_publish(
                "",
                self.get_queue_name("match", "abrupt_close"),
                BasicPublishOptions::default(),
                &serde_json::to_vec(&match_close).unwrap(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn report_match_created(&self, created_match: &CreatedMatch) {
        self.channel
            .basic_publish(
                "",
                self.get_queue_name("match", "created"),
                BasicPublishOptions::default(),
                &serde_json::to_vec(&created_match).unwrap(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn create_ai_task(&self, task: &crate::models::Task) {
        self.channel
            .basic_publish(
                "",
                self.get_queue_name("ai", "task"),
                BasicPublishOptions::default(),
                serde_json::to_vec(&task).unwrap().as_slice(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn report_match_result(&self, match_result: &MatchResult) {
        self.channel
            .basic_publish(
                "",
                self.get_queue_name("match", "result"),
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
                self.get_queue_name("health_check", "check"),
                BasicPublishOptions::default(),
                client_id.as_bytes(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }

    async fn on_health_check<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<String, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static,
    {
        setup_queue_and_listen(
            self.channel.clone(),
            self.get_queue_name("health_check", "check"),
            {
                move |_, delivery| {
                    let callback = callback.clone();
                    async move {
                        debug!("Received healthcheck event: {:?}", delivery);
                        delivery.ack(BasicAckOptions::default()).await.expect("ack");

                        let client_id: String =
                            std::string::String::from_utf8(delivery.data).unwrap();
                        callback(client_id).await;
                    }
                }
            },
        )
        .await;
    }

    async fn on_ai_register<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::AIPlayerRegister, Fut>,
        Fut: Future<Output = ()> + Send + Sync + 'static,
    {
        setup_queue_and_listen(
            self.channel.clone(),
            self.get_queue_name("ai", "register"),
            {
                move |_, delivery| {
                    let callback = callback.clone();
                    async move {
                        debug!("Received AI register event: {:?}", delivery);
                        delivery.ack(BasicAckOptions::default()).await.expect("ack");

                        let ai_register: crate::models::AIPlayerRegister =
                            serde_json::from_slice(&delivery.data).unwrap();
                        callback(ai_register).await;
                    }
                }
            },
        )
        .await;
    }

    async fn register_ai_player(&self, ai_player: &crate::models::AIPlayerRegister) {
        self.channel
            .basic_publish(
                "",
                self.get_queue_name("ai", "register"),
                BasicPublishOptions::default(),
                &serde_json::to_vec(&ai_player).unwrap(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
    }
}
