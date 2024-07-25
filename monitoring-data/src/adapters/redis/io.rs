// NOTE: The content of this file is, in best case, temporary and will be rewritten to automated proc-macros

use redis::{Commands, Connection};

use crate::models;

use super::{RedisInsertWriter, RedisNameable, RedisOutputReader};

impl RedisNameable for models::DBGameServer {
    fn name() -> String {
        "game_server".to_owned()
    }
}

impl RedisNameable for models::GameServer {
    fn name() -> String {
        "game_server".to_owned()
    }
}

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
