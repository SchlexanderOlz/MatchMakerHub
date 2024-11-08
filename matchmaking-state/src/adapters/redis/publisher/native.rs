use redis::{Connection, FromRedisValue};

use crate::adapters::{
    redis::{NotifyOnRedisEvent, RedisAdapter, RedisIdentifiable},
    InfoPublisher, Publishable,
};

const EVENT_PREFIX: &str = "events";
#[derive(Default)]
pub struct RedisInfoPublisher {
    connection: Option<Connection>,
}

impl RedisInfoPublisher {
    #[inline]
    pub fn new(connection: Connection) -> Self {
        Self {
            connection: Some(connection),
        }
    }
}

impl InfoPublisher<redis::Connection> for RedisInfoPublisher {
    fn publish(
        &mut self,
        created: &dyn Publishable<redis::Connection>,
        event: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        created.publish(
            self.connection.as_mut().unwrap(),
            format!("{}:{}", EVENT_PREFIX, event),
        )?;
        Ok(())
    }
}

fn loop_on_redis_event<T>(
    channel: String,
    client: redis::Client,
    mut handler: impl FnMut(T) -> () + Send + Sync + 'static,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
where
    T: FromRedisValue,
{
    let mut connection = client.get_connection()?;
    Ok(tokio::task::spawn_blocking(move || {
        let mut connection = connection.as_pubsub();
        connection.psubscribe(channel).unwrap(); // TODO: Handle error
        loop {
            let msg = connection.get_message().unwrap();
            let payload = msg.get_payload::<T>().unwrap();
            handler(payload);
        }
    }))
}

impl<T> NotifyOnRedisEvent<redis::Connection> for T
where
    T: RedisIdentifiable,
{
    fn on_update<O>(
        connection: &RedisAdapter<redis::Connection>,
        handler: impl FnMut(O) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        O: FromRedisValue,
    {
        Ok(loop_on_redis_event(
            format!("{}:update:*:{}", EVENT_PREFIX, T::name()),
            connection.client.clone(),
            handler,
        )?)
    }

    fn on_delete<O>(
        connection: &RedisAdapter<redis::Connection>,
        handler: impl FnMut(O) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        O: FromRedisValue,
    {
        Ok(loop_on_redis_event(
            format!("{}:delete:*:{}", EVENT_PREFIX, T::name()),
            connection.client.clone(),
            handler,
        )?)
    }

    fn on_insert<O>(
        connection: &RedisAdapter<redis::Connection>,
        handler: impl FnMut(O) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        O: FromRedisValue,
    {
        Ok(loop_on_redis_event(
            format!("{}:insert:*:{}", EVENT_PREFIX, T::name()),
            connection.client.clone(),
            handler,
        )?)
    }
}
