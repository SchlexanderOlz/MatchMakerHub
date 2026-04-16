use futures_lite::{Future, StreamExt};
use std::{collections::HashMap, sync::Arc, time::Duration};

use lapin::{
    message::Delivery,
    options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions},
    types::FieldTable,
    BasicProperties, Channel, Connection,
};
use tracing::{debug, error, info, warn};

use crate::{
    models::{CreateMatch, CreatedMatch, GameServerCreate, MatchAbrubtClose, MatchResult},
    MessageHandler,
};

async fn try_connect(amqp_url: &str) -> Connection {
    loop {
        match Connection::connect(amqp_url, lapin::ConnectionProperties::default()).await {
            Ok(conn) => return conn,
            Err(err) => {
                error!("Could not connect to RabbitMQ: {:?}", err);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// Spawn a consumer loop that automatically reconnects when the connection drops.
/// Each invocation creates its own AMQP connection so consumers are isolated from
/// each other and from the publish channel.
async fn setup_queue_and_listen<F, Fut>(amqp_url: String, queue_name: String, on_message: F)
where
    F: Fn(Delivery) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            let conn = try_connect(&amqp_url).await;

            let channel = match conn.create_channel().await {
                Ok(ch) => Arc::new(ch),
                Err(e) => {
                    error!("Failed to create channel: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            if let Err(e) = channel
                .queue_declare(&queue_name, QueueDeclareOptions::default(), FieldTable::default())
                .await
            {
                error!("Failed to declare queue {}: {:?}", queue_name, e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            let mut consumer = match channel
                .basic_consume(
                    &queue_name,
                    // Use a unique tag so re-registration never conflicts with a stale consumer
                    &uuid::Uuid::new_v4().to_string(),
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to start consumer for {}: {:?}", queue_name, e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            info!("Listening on queue: {}", queue_name);

            while let Some(delivery) = consumer.next().await {
                match delivery {
                    Ok(delivery) => {
                        let on_message = on_message.clone();
                        tokio::spawn(async move { on_message(delivery).await });
                    }
                    Err(err) => {
                        error!("Consumer error on {}, reconnecting: {:?}", queue_name, err);
                        break;
                    }
                }
            }

            warn!("Consumer loop for {} ended, reconnecting in 5s...", queue_name);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

pub struct RabbitMQCommunicator {
    amqp_url: String,
    /// Shared publish channel — replaced atomically when a publish fails.
    channel: Arc<tokio::sync::RwLock<Arc<Channel>>>,
    queues: HashMap<String, HashMap<String, String>>,
}

impl RabbitMQCommunicator {
    pub async fn connect(amqp_url: &str) -> Self {
        let conn = try_connect(amqp_url).await;
        let channel = Arc::new(
            conn.create_channel()
                .await
                .expect("Could not create publish channel"),
        );

        Self {
            amqp_url: amqp_url.to_string(),
            channel: Arc::new(tokio::sync::RwLock::new(channel)),
            queues: Self::load_default_queues(),
        }
    }

    fn load_default_queues() -> HashMap<String, HashMap<String, String>> {
        let content = include_str!("../queues.yml");
        serde_yaml::from_str(content).expect("Failed to parse queues file")
    }

    fn get_queue_name(&self, name: &str, action: &str) -> &str {
        self.queues
            .get(name)
            .expect(&format!("Queue {} not found", name))
            .get(action)
            .expect(&format!("Action {} not found for queue {}", action, name))
    }

    pub fn load_queues(&mut self, path: &str) {
        let content = std::fs::read_to_string(path).expect("Failed to read file");
        self.queues = serde_yaml::from_str(&content).expect("Failed to parse routes file");
    }

    /// Publish `data` to `queue`, transparently reconnecting if the channel is dead.
    async fn publish_with_retry(&self, queue: &str, data: Vec<u8>) {
        loop {
            let channel = self.channel.read().await.clone();
            match channel
                .basic_publish(
                    "",
                    queue,
                    BasicPublishOptions::default(),
                    &data,
                    BasicProperties::default(),
                )
                .await
            {
                Ok(_) => return,
                Err(err) => {
                    error!("Publish to {} failed: {:?}, reconnecting...", queue, err);
                    let conn = try_connect(&self.amqp_url).await;
                    match conn.create_channel().await {
                        Ok(new_ch) => *self.channel.write().await = Arc::new(new_ch),
                        Err(e) => error!("Failed to create channel after reconnect: {:?}", e),
                    }
                }
            }
        }
    }
}

impl super::Communicator for RabbitMQCommunicator {
    async fn on_match_abrupt_close<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<MatchAbrubtClose, Fut>,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let queue = self.get_queue_name("match", "abrupt_close").to_string();
        setup_queue_and_listen(self.amqp_url.clone(), queue, move |delivery| {
            let callback = callback.clone();
            async move {
                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                    error!("Failed to ack delivery: {:?}", e);
                    return;
                }
                let reason: MatchAbrubtClose = serde_json::from_slice(&delivery.data).unwrap();
                callback(reason).await;
            }
        })
        .await;
    }

    async fn on_game_create<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::GameServerCreate, Fut>,
        Fut: Future<Output = String> + Send + 'static,
    {
        let queue = self.get_queue_name("game", "create").to_string();
        setup_queue_and_listen(self.amqp_url.clone(), queue, move |delivery| {
            let callback = callback.clone();
            async move {
                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                    error!("Failed to ack delivery: {:?}", e);
                    return;
                }
                let created_game: GameServerCreate =
                    serde_json::from_slice(&delivery.data).unwrap();
                callback(created_game).await;
            }
        })
        .await;
    }

    async fn on_match_created<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::CreatedMatch, Fut>,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let queue = self.get_queue_name("match", "created").to_string();
        setup_queue_and_listen(self.amqp_url.clone(), queue, move |delivery| {
            let callback = callback.clone();
            async move {
                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                    error!("Failed to ack delivery: {:?}", e);
                    return;
                }
                let created_match: CreatedMatch = serde_json::from_slice(&delivery.data).unwrap();
                callback(created_match).await;
            }
        })
        .await;
    }

    async fn on_match_result<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::MatchResult, Fut>,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let queue = self.get_queue_name("match", "result").to_string();
        setup_queue_and_listen(self.amqp_url.clone(), queue, move |delivery| {
            let callback = callback.clone();
            async move {
                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                    error!("Failed to ack delivery: {:?}", e);
                    return;
                }
                let result: MatchResult = serde_json::from_slice(&delivery.data).unwrap();
                callback(result).await;
            }
        })
        .await;
    }

    async fn on_match_create<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::CreateMatch, Fut>,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let queue = self.get_queue_name("match", "create").to_string();
        setup_queue_and_listen(self.amqp_url.clone(), queue, move |delivery| {
            let callback = callback.clone();
            async move {
                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                    error!("Failed to ack delivery: {:?}", e);
                    return;
                }
                let result: CreateMatch = serde_json::from_slice(&delivery.data).unwrap();
                callback(result).await;
            }
        })
        .await;
    }

    async fn create_game(
        &self,
        game_server: &GameServerCreate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.publish_with_retry(
            self.get_queue_name("game", "create"),
            serde_json::to_vec(&game_server).unwrap(),
        )
        .await;
        Ok(())
    }

    async fn create_match(&self, match_request: &CreateMatch) {
        self.publish_with_retry(
            self.get_queue_name("match", "create"),
            serde_json::to_vec(&match_request).unwrap(),
        )
        .await;
    }

    async fn report_match_abrupt_close(&self, match_close: &MatchAbrubtClose) {
        self.publish_with_retry(
            self.get_queue_name("match", "abrupt_close"),
            serde_json::to_vec(&match_close).unwrap(),
        )
        .await;
    }

    async fn report_match_created(&self, created_match: &CreatedMatch) {
        self.publish_with_retry(
            self.get_queue_name("match", "created"),
            serde_json::to_vec(&created_match).unwrap(),
        )
        .await;
    }

    async fn create_ai_task(&self, task: &crate::models::Task) {
        self.publish_with_retry(
            self.get_queue_name("ai", "task"),
            serde_json::to_vec(&task).unwrap(),
        )
        .await;
    }

    async fn report_match_result(&self, match_result: &MatchResult) {
        self.publish_with_retry(
            self.get_queue_name("match", "result"),
            serde_json::to_vec(&match_result).unwrap(),
        )
        .await;
    }

    async fn send_health_check(&self, client_id: String) {
        self.publish_with_retry(
            self.get_queue_name("health_check", "check"),
            client_id.into_bytes(),
        )
        .await;
    }

    async fn on_health_check<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<String, Fut>,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let queue = self.get_queue_name("health_check", "check").to_string();
        setup_queue_and_listen(self.amqp_url.clone(), queue, move |delivery| {
            let callback = callback.clone();
            async move {
                debug!("Received healthcheck event");
                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                    error!("Failed to ack delivery: {:?}", e);
                    return;
                }
                let client_id = String::from_utf8(delivery.data.to_vec()).unwrap();
                callback(client_id).await;
            }
        })
        .await;
    }

    async fn on_ai_register<F, Fut>(&self, callback: F)
    where
        F: MessageHandler<crate::models::AIPlayerRegister, Fut>,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let queue = self.get_queue_name("ai", "register").to_string();
        setup_queue_and_listen(self.amqp_url.clone(), queue, move |delivery| {
            let callback = callback.clone();
            async move {
                debug!("Received AI register event");
                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                    error!("Failed to ack delivery: {:?}", e);
                    return;
                }
                let ai_register: crate::models::AIPlayerRegister =
                    serde_json::from_slice(&delivery.data).unwrap();
                callback(ai_register).await;
            }
        })
        .await;
    }

    async fn register_ai_player(&self, ai_player: &crate::models::AIPlayerRegister) {
        self.publish_with_retry(
            self.get_queue_name("ai", "register"),
            serde_json::to_vec(&ai_player).unwrap(),
        )
        .await;
    }
}
