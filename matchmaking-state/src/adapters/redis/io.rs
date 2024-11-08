use std::{
    any::Any,
    collections::HashMap,
    hash::Hash,
    time::{Duration, SystemTime},
};

use redis::{Commands, Connection, FromRedisValue, Pipeline, RedisWrite, ToRedisArgs};

use crate::{adapters::InfoPublisher, models};

use super::{
    NotifyOnRedisEvent, Publishable, RedisAdapter, RedisIdentifiable, RedisInsertWriter,
    RedisOutputReader, RedisUpdater,
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

impl<K, V> RedisInsertWriter for HashMap<K, V>
where
    V: ToRedisArgs,
    K: ToRedisArgs,
{
    fn write(&self, pipe: &mut Pipeline, base_key: &str) -> Result<(), Box<dyn std::error::Error>> {
        for (key, val) in self {
            pipe.hset(format!("{}", base_key), key, val);
        }
        Ok(())
    }
}

impl<K, V> RedisOutputReader for HashMap<K, V>
where
    V: FromRedisValue,
    K: FromRedisValue + std::cmp::Eq + Hash,
{
    fn read(
        conn: &mut Connection,
        base_key: &str,
    ) -> Result<HashMap<K, V>, Box<dyn std::error::Error>> {
        Ok(conn.hgetall(base_key)?)
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

impl<T> Publishable<redis::Connection> for T
where
    T: ToRedisArgs,
{
    fn publish(
        &self,
        connection: &mut redis::Connection,
        channel: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        connection.publish(channel, self)?;
        Ok(())
    }
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
