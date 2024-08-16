use std::{collections::HashMap, hash::Hash, time::{Duration, SystemTime}};

use redis::{Commands, Connection, FromRedisValue, Pipeline, RedisWrite, ToRedisArgs};

use super::{
    NotifyOnRedisEvent, RedisAdapter, RedisIdentifiable, RedisInsertWriter, RedisOutputReader, RedisUpdater, EVENT_PREFIX
};

impl<T> RedisInsertWriter for Vec<T>
where
    T: RedisInsertWriter,
{
    fn write(
        &self,
        pipe: &mut redis::Pipeline,
        base_key: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (i, mode) in self.iter().enumerate() {
            mode.write(pipe, format!("{base_key}:{}", i).as_str())?;
        }
        Ok(())
    }
}

impl<T> RedisOutputReader for Vec<T>
where
    T: RedisOutputReader,
{
    fn read(conn: &mut Connection, base_key: &str) -> Result<Vec<T>, Box<dyn std::error::Error>> {
        let mut converted = Vec::new();
        let mut i = 0;
        loop {
            let key = format!("{base_key}:{}", i);
            let res = T::read(conn, &key);

            if res.is_err() {
                return Ok(converted);
            }
            converted.push(res.unwrap());
            i += 1;
        }
    }
}

impl<T> RedisUpdater<T> for Vec<T>
where
    T: RedisInsertWriter,
{
    fn update(&self, pipe: &mut Pipeline, uuid: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.write(pipe, uuid)
    }
}

impl <K, V> RedisInsertWriter for HashMap<K, V> 
where
    V: ToRedisArgs,
    K: ToRedisArgs 
{
    fn write(&self, pipe: &mut Pipeline, base_key: &str) -> Result<(), Box<dyn std::error::Error>> {
        for (key, val) in self {
            pipe.hset(format!("{}", base_key), key, val);
        }
        Ok(())
    }
}

impl <K, V> RedisOutputReader for HashMap<K, V> 
where
    V: FromRedisValue,
    K: FromRedisValue + std::cmp::Eq + Hash
{
    fn read(conn: &mut Connection, base_key: &str) -> Result<HashMap<K, V>, Box<dyn std::error::Error>> {
        Ok(conn.hgetall(base_key)?)
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
    Ok(tokio::task::spawn(async move {
        let mut connection = connection.as_pubsub();
        let _ = connection.psubscribe(channel); // TODO: Handle error
        loop {
            let msg = connection.get_message().unwrap();
            let payload = msg.get_payload::<T>().unwrap();
            handler(payload);
        }
    }))
}

impl<T> NotifyOnRedisEvent for T
where
    T: RedisIdentifiable,
{
    fn on_update<O>(
        connection: &RedisAdapter,
        handler: impl FnMut(O) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        O: FromRedisValue,
    {
        Ok(loop_on_redis_event(format!("{}:update:*:{}", EVENT_PREFIX, T::name()), connection.client.clone(), handler)?)
    }

    fn on_delete<O>(
        connection: &RedisAdapter,
        handler: impl FnMut(O) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        O: FromRedisValue,
    {
        Ok(loop_on_redis_event(format!("{}:delete:*:{}", EVENT_PREFIX, T::name()), connection.client.clone(), handler)?)
    }

    fn on_insert<O>(
        connection: &RedisAdapter,
        handler: impl FnMut(O) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        O: FromRedisValue,
    {
        Ok(loop_on_redis_event(format!("{}:insert:*:{}", EVENT_PREFIX, T::name()), connection.client.clone(), handler)?)
    }
}

macro_rules! impl_redis_writer_primitive {
    ($($type:ty),*) => {
        $(
            impl RedisInsertWriter for $type {
                fn write(&self, pipe: &mut redis::Pipeline, base_key: &str) -> Result<(), Box<dyn std::error::Error>> {
                    pipe.set(base_key, self);
                    Ok(())
                }
            }
        )*
    };
}

macro_rules! impl_redis_reader_primitive {
    ($($type:ty),*) => {
        $(
            impl RedisOutputReader for $type {
                fn read(conn: &mut Connection, base_key: &str) -> Result<$type, Box<dyn std::error::Error>> {
                    Ok(conn.get(base_key)?)
                }
            }
        )*
    };
}

impl_redis_writer_primitive!(
    bool, i8, i16, i32, i64, isize, u8, u16, u32, u64, f32, f64, String, usize
);
impl_redis_reader_primitive!(
    bool, i8, i16, i32, i64, isize, u8, u16, u32, u64, f32, f64, String, usize
);

impl RedisInsertWriter for SystemTime {
    fn write(
        &self,
        pipe: &mut redis::Pipeline,
        base_key: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        pipe.set(
            base_key,
            self.duration_since(SystemTime::UNIX_EPOCH)?.as_secs(),
        );
        Ok(())
    }
}

impl RedisOutputReader for SystemTime {
    fn read(
        connection: &mut Connection,
        base_key: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(std::time::UNIX_EPOCH + Duration::from_secs(connection.get(base_key)?))
    }
}
