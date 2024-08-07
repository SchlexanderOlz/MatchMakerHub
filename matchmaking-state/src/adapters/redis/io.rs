use std::time::{Duration, SystemTime};

use redis::{Commands, Connection, FromRedisValue, Pipeline};

use super::{
    NotifyOnRedisEvent, RedisIdentifiable, RedisInsertWriter, RedisOutputReader, RedisUpdater,
    EVENT_PREFIX,
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

fn loop_on_redis_event<T>(
    mut connection: redis::PubSub<'static>,
    mut handler: impl FnMut(T) -> () + Send + Sync + 'static,
) -> tokio::task::JoinHandle<()>
where
    T: FromRedisValue,
{
    tokio::task::spawn(async move {
        loop {
            let msg = connection.get_message().unwrap();
            let payload = msg.get_payload::<T>().unwrap();
            handler(payload);
        }
    })
}

impl<T> NotifyOnRedisEvent for T
where
    T: RedisIdentifiable,
{
    fn on_update<O>(
        mut connection: redis::PubSub<'static>,
        handler: impl FnMut(O) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        O: FromRedisValue,
    {
        let _ = connection.subscribe(format!("{}:update:*:{}:*", EVENT_PREFIX, T::name()))?;

        Ok(loop_on_redis_event(connection, handler))
    }

    fn on_delete<O>(
        mut connection: redis::PubSub<'static>,
        handler: impl FnMut(O) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        O: FromRedisValue,
    {
        let _ = connection.subscribe(format!("{}:delete:*:{}:*", EVENT_PREFIX, T::name()))?;

        Ok(loop_on_redis_event(connection, handler))
    }

    fn on_insert<O>(
        mut connection: redis::PubSub<'static>,
        handler: impl FnMut(O) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        O: FromRedisValue,
    {
        let _ = connection.subscribe(format!("{}:create:*:{}:*", EVENT_PREFIX, T::name()))?;

        Ok(loop_on_redis_event(connection, handler))
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
