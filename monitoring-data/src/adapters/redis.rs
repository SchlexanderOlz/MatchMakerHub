use super::{DataAdapter, Gettable, Insertable, Removable, Searchable};
use redis::{Commands, Connection, Pipeline};

mod io;

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

pub trait RedisFilter<T> {
    fn is_ok(&self, check: &T) -> bool;
}

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
    fn read(
        connection: &mut Connection,
        base_key: &str,
    ) -> Result<Self, Box<dyn std::error::Error>>;
}

impl Removable for RedisAdapter
{
    fn remove(&mut self, uuid: &str) -> Result<(), Box<dyn std::error::Error>> {
        let iter = self
            .connection
            .scan_match(format!("{}:*", uuid))?
            .into_iter()
            .collect::<Vec<String>>();

        redis::transaction(&mut self.connection, iter.as_slice(), |conn, pipe| {
            iter.iter().for_each(|key| {
                pipe.del(key).ignore();
            });
            pipe.query(conn)
        })?;
        Ok(())
    }
}

impl<T> Insertable<T> for RedisAdapter
where
    T: RedisInsertWriter + RedisNameable + Clone,
{
    fn insert(&mut self, data: T) -> Result<String, Box<dyn std::error::Error>> {
        let counter: i64 = self.connection.incr("uuid_inc", 1)?;
        let key = format!("{}:{}", counter, T::name());

        let mut pipe = redis::pipe();
        pipe.atomic();
        data.write(&mut pipe, &key)?;
        pipe.set(key, "");
        pipe.query(&mut self.connection)?;

        Ok(counter.to_string())
    }
}

impl<O> Gettable<O> for RedisAdapter
where
    O: RedisOutputReader + RedisNameable,
{
    fn all(&mut self) -> Result<impl Iterator<Item = O>, Box<dyn std::error::Error>> {
        let mut iter: redis::Iter<String> =
            self.connection.scan_match(format!("*:{}", O::name()))?;
        let mut connection = self.client.get_connection()?;

        let iter = std::iter::from_fn(move || {
            if let Some(key) = iter.next() {
                let res = O::read(&mut connection, &key).ok()?;
                return Some(res);
            }
            None
        });

        Ok(iter)
    }

    fn get(&mut self, uuid: &str) -> Result<O, Box<dyn std::error::Error>> {
        O::read(&mut self.connection, &format!("{uuid}:{}", O::name()))
    }
}

impl<O, F> Searchable<O, F> for RedisAdapter
where
    O: RedisOutputReader + RedisNameable,
    F: RedisFilter<O> + Default,
{
    fn filter(&mut self, filter: F) -> Result<impl Iterator<Item = O>, Box<dyn std::error::Error>> {
        let mut iter: redis::Iter<String> =
            self.connection.scan_match(format!("*:{}", O::name()))?;
        let mut connection = self.client.get_connection()?;

        let iter = std::iter::from_fn(move || {
            while let Some(key) = iter.next() {
                let res = O::read(&mut connection, &key).ok()?;
                if filter.is_ok(&res) {
                    return Some(res);
                }
            }
            None
        });

        Ok(iter)
    }
}

impl<T, O, F> DataAdapter<T, O, F> for RedisAdapter
where
    T: Clone + RedisInsertWriter + RedisNameable,
    F: RedisFilter<O> + Default,
    O: RedisOutputReader + RedisNameable, // TODO: The Deletable should not be in the same trait as T
{
}
