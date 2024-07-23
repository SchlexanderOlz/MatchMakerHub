use redis::{Commands, Connection, Pipeline};
use crate::{
    models::{self, GameMode}, DataAdapter, Filter, Insertable, Removable, Searchable
};

mod io;

const BASE_SERVER: &str = "htps://";


pub trait RedisNameable {
    fn name() -> String;
}

pub trait RedisInsertWriter {
    fn write(&self, pipe: &mut Pipeline, base_key: &str) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait RedisOutputReader
where
    Self: Sized,
{
    fn read(connection: &mut Connection, base_key: &str) -> Result<Self, Box<dyn std::error::Error>>;
}

impl Removable for RedisAdapter {
    fn remove(&mut self, uuid: String) -> Result<(), Box<dyn std::error::Error>> {
        Ok(self.connection.del(uuid)?)
    }
}

impl<T> Insertable<T> for RedisAdapter
where T: RedisInsertWriter + RedisNameable + Clone
{
    fn insert(&mut self, data: T) -> Result<(), Box<dyn std::error::Error>> {
        let counter: i64 = self.connection.incr("uuid_inc", 1)?;
        let key = format!("{}:{}", counter, T::name());

        let mut pipe = redis::pipe();
        pipe.atomic();
        data.write(&mut pipe, &key)?;
        pipe.set(key, "");
        pipe.query(&mut self.connection)?;

        Ok(())
    }
}

impl<O, F>
    Searchable<
        O,
        F,
    > for RedisAdapter
where O: RedisOutputReader + RedisNameable, F: Filter<O> + Default
{
    fn all(
        &mut self,
    ) -> Result<impl Iterator<Item = O>, Box<dyn std::error::Error>> {
        self.filter(F::default())
    }

    fn filter(
        &mut self,
        filter: F,
    ) -> Result<impl Iterator<Item = O>, Box<dyn std::error::Error>> {
        // ERROR: The filter is currently ignored
        _ = filter;

        let mut iter: redis::Iter<String> = self.connection.scan_match(format!("*:{}", O::name()))?;
        let mut connection = self.client.get_connection()?;

        let iter = std::iter::from_fn(move || {
            while let Some(key) = iter.next() {
                let res = O::read(&mut connection, &key).ok()?;
                if filter.is_ok(&res) {
                    return Some(res);
                }
            }
            None
        }
        );

        Ok(iter)
    }

    fn get(&mut self, uuid: String) -> Result<O, Box<dyn std::error::Error>> {
        O::read(&mut self.connection, &uuid)
    }
}

impl<T, O, F>
    DataAdapter<
    T,
    O,
    F
    > for RedisAdapter
where T: Clone + RedisInsertWriter + RedisNameable, F: Filter<O> + Default, O: RedisOutputReader + RedisNameable
{
}

pub struct RedisAdapter {
    pub client: redis::Client,
    pub connection: redis::Connection,
}

impl RedisAdapter {
    pub fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = redis::Client::open(url)?;
        Ok(Self {
            connection: client.get_connection()?,
            client,
        })
    }
}