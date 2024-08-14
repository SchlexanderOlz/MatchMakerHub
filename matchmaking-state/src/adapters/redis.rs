use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::models::{DBSearcher, Match};

use super::{DataAdapter, Gettable, Insertable, Matcher, Removable, Searchable, Updateable};
use redis::{Commands, Connection, FromRedisValue, Msg, Pipeline, PubSub};
use tracing::{error, info};

mod io;

const EVENT_PREFIX: &str = "events";

// TODO: There are definetly some thread-mutability issues in the RedisAdapter due to the excesive use of Arc<Mutex>. Fix this in a #Refractoring
pub struct RedisAdapter {
    client: redis::Client,
    connection: Arc<Mutex<redis::Connection>>,
    handlers: Arc<Mutex<Vec<Arc<dyn Send + Sync + 'static + Fn(Match) -> ()>>>>,
}

impl From<redis::Client> for RedisAdapter {
    fn from(client: redis::Client) -> Self {
        let connection =
            Arc::new(Mutex::new(client.get_connection().expect(
                format!("Could not connect to redis server at {:?}", client).as_str(),
            )));
        Self {
            client,
            connection,
            handlers: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Clone for RedisAdapter {
    fn clone(&self) -> Self {
        let client = self.client.clone();
        Self {
            connection: Arc::new(Mutex::new(client.get_connection().unwrap())),
            client,
            handlers: self.handlers.clone(),
        }
    }
}

impl RedisAdapter {
    /// Connects to a redis server using the given url.

    ///
    /// # Arguments
    ///
    /// * `url` - The url to connect to the redis server.
    ///     - *format*: `redis://[<username>][:<password>@]<hostname>[:port][/<db>]`
    ///     - *example*: `redis://john:password@127.0.0.1:6379/0`
    ///
    /// # Returns
    ///
    /// A `Result` with the any connection error. If Ok a new `RedisAdapter` object is returned.
    pub fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = redis::Client::open(url)?;
        Ok(Self::from(client))
    }

    /// Starts the match check in a new task. Creates a new seperate connection to the redis server.
    ///
    /// # Returns
    ///
    /// A `tokio::task::JoinHandle` that represents the spawned task.
    pub fn start_match_check(&self) -> tokio::task::JoinHandle<()> {
        let self_clone = self.clone();
        tokio::task::spawn(async move {
            self_clone.match_check().unwrap();
        })
    }

    /// Starts the match check in the current thread. Creates a new seperate connection to the redis server using it as a pubsub connection for events.
    ///
    /// # Returns
    ///
    /// A `Result` with the error if any occured. Under normal conditions this function will not exit and therefore the result should be `!`.
    /// This is currently an experimental feature in Rust and therefore not implemented here yet.
    pub fn match_check(self) -> Result<(), Box<dyn std::error::Error>> {
        // NOTE: Result should be '!' for Ok values. This is currently expermintal tough and therefore not implemented here.
        let mut connection = self.client.get_connection()?;
        let mut connection = connection.as_pubsub();

        connection.psubscribe("*:match:*")?;
        info!("Subscribed");

        self.acc_searchers(connection)
    }

    fn acc_searchers(mut self, mut connection: PubSub) -> Result<(), Box<dyn std::error::Error>> {
        let mut found_servers: HashMap<String, String> = HashMap::new();
        let mut found_players: HashMap<String, Vec<String>> = HashMap::new();

        // TODO: Multithread this as soon as the problem with the order of messages is fixed.
        loop {
            let msg = connection.get_message().unwrap();
            info!("Message received: {:?}", msg);
            self.handle_msg(msg, &mut found_servers, &mut found_players);
        }
    }

    fn handle_msg(
        &mut self,
        msg: Msg,
        found_servers: &mut HashMap<String, String>,
        found_players: &mut HashMap<String, Vec<String>>,
    ) {
        let payload = msg.get_payload::<String>().unwrap();

        info!("Payload: {:?}", payload);

        let channel = msg.get_channel_name().split(":").collect::<Vec<&str>>();

        let last = channel.last().unwrap();
        let uuid = channel.first().unwrap().to_string();

        if last.to_string() == "server".to_string() {
            found_servers.insert(channel.first().unwrap().to_string(), payload);
            return;
        }

        if last.to_string() == "done".to_string() {
            self.on_done_msg(&uuid, payload, &found_servers, &found_players)
                .unwrap();
            // TODO: The order of messages is likely but not guaranteed. This could be a potential error and should be handled accordingly.
            return;
        }

        if channel.get(channel.len() - 2).unwrap().to_string() == "players".to_string() {
            if let Some(players) = found_players.get_mut(&uuid) {
                players.push(payload);
                return;
            }
            found_players.insert(channel.first().unwrap().to_string(), vec![payload]);
        }
    }

    fn on_done_msg(
        &mut self,
        uuid: &String,
        payload: String,
        found_servers: &HashMap<String, String>,
        found_players: &HashMap<String, Vec<String>>,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
        let server = found_servers.get(uuid);
        if server.is_none() {
            todo!("Handle the server error accordingly");
        }
        let server = server.unwrap();

        let players = found_players.get(uuid);
        if players.is_none() || players.unwrap().len() == 0 {
            todo!("Handle the player error accordingly");
        }
        let players = players.unwrap().clone();
        if players.len() as i32 != payload.parse::<i32>()? {
            todo!("Handle the player count error accordingly");
        }

        let example_searcher: DBSearcher = self.get(players.get(0).unwrap())?;

        let new_match = Match {
            address: server.to_string(),
            players: players.clone(),
            mode: example_searcher.mode,
            game: example_searcher.game,
        };

        let handles: Vec<_> = self
            .handlers
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .map(move |fun| {
                let match_clone = new_match.clone();
                tokio::task::spawn(async move { fun(match_clone) })
            })
            .collect();

        let mut self_clone = self.clone();
        Ok(tokio::task::spawn(async move {
            // TODO: Tasks should be joined in async
            for handle in handles {
                handle.await.unwrap();
            }

            players.iter().for_each(|player| {
                if let Err(err) = self_clone.remove(player) {
                    error!("Error removing player 'uuid: {}': {}", player, err);
                }
            });
        }))
    }
}

pub trait RedisFilter<T> {
    fn is_ok(&self, check: &T) -> bool;
}

pub trait RedisUpdater<T> {
    fn update(&self, pipe: &mut Pipeline, uuid: &str) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait RedisIdentifiable {
    fn name() -> String;
    fn next_uuid(connection: &mut Connection) -> Result<String, Box<dyn std::error::Error>> {
        let counter: i64 = connection.incr("uuid_inc", 1)?;
        Ok(format!("{}:{}", counter, Self::name()))
    }
}

pub trait RedisInsertWriter {
    fn write(&self, pipe: &mut Pipeline, base_key: &str) -> Result<(), Box<dyn std::error::Error>>;
}

/// TODO: Currently functions in this trait require the arguments to be static. This solution prohibits removing existing handlers.
/// This should be fixed by using some Context-Manager which provides the PubSub connection and handler. When the Context-Manager is dropped the handler and handler-thread should be killed.
/// This trait should also be moved to the super-module
pub trait NotifyOnRedisEvent {
    fn on_update<T>(
        connection: &RedisAdapter,
        handler: impl FnMut(T) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        T: FromRedisValue;

    fn on_insert<T>(
        connection: &RedisAdapter,
        handler: impl FnMut(T) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        T: FromRedisValue;

    fn on_delete<T>(
        connection: &RedisAdapter,
        handler: impl FnMut(T) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        T: FromRedisValue;
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

impl Removable for RedisAdapter {
    fn remove(&mut self, uuid: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut connection = self.connection.lock().unwrap();
        let iter = connection
            .scan_match(format!("{}*", uuid))?
            .into_iter()
            .collect::<Vec<String>>();

        redis::transaction(&mut connection, iter.as_slice(), |conn, pipe| {
            iter.iter().for_each(|key| {
                pipe.del(key).ignore();
            });
            pipe.query(conn)
        })?;

        connection.publish(format!("{}:remove:{}", EVENT_PREFIX, uuid), uuid)?;
        Ok(())
    }
}

impl<T> Insertable<T> for RedisAdapter
where
    T: RedisInsertWriter + RedisIdentifiable + Clone,
{
    fn insert(&mut self, data: T) -> Result<String, Box<dyn std::error::Error>> {
        let key = { T::next_uuid(&mut self.connection.lock().unwrap())? };

        let mut pipe = redis::pipe();
        pipe.atomic();
        data.write(&mut pipe, &key)?;
        pipe.set(key.clone(), "");

        {
            let mut connection = self.connection.lock().unwrap();
            pipe.query(&mut connection)?;
            connection.publish(format!("{}:insert:{}", EVENT_PREFIX, key), key.clone())?;
        }

        let mut split = key.split(":");
        Ok(split
            .next()
            .expect(format!("Invalid id on object of type {}", T::name()).as_str())
            .to_string()
            + ":"
            + split
                .next()
                .expect(format!("Invalid id on object of type {}", T::name()).as_str()))
    }
}

impl<'a, O> Gettable<'a, O> for RedisAdapter
where
    O: RedisOutputReader + RedisIdentifiable,
{
    type Type = Box<dyn Iterator<Item = O> + 'a>;

    fn all(&'a self) -> Result<Self::Type, Box<dyn std::error::Error>> {
        let mut iter = self
            .connection
            .lock()
            .unwrap()
            .scan_match(format!("*:{}", O::name()))?
            .collect::<Vec<String>>()
            .into_iter();

        let connection_ref = self.connection.clone();
        let iter_fun = std::iter::from_fn(move || {
            if let Some(key) = iter.next() {
                let res = O::read(&mut connection_ref.lock().unwrap(), &key).ok()?;
                return Some(res);
            }
            None
        });

        Ok(Box::new(iter_fun))
    }

    fn get(&self, uuid: &str) -> Result<O, Box<dyn std::error::Error>> {
        O::read(&mut self.connection.lock().unwrap(), uuid)
    }
}

impl<'a, O, F> Searchable<'a, O, F> for RedisAdapter
where
    O: RedisOutputReader + RedisIdentifiable + 'a,
    F: RedisFilter<O> + Default + 'a,
{
    type Type = Box<dyn Iterator<Item = O> + 'a>;

    fn filter(&'a self, filter: F) -> Result<Self::Type, Box<dyn std::error::Error>> {
        let mut iter = self
            .connection
            .lock()
            .unwrap()
            .scan_match(format!("*:{}", O::name()))?
            .collect::<Vec<String>>()
            .into_iter();

        let connection_ref = self.connection.clone();
        let iter = std::iter::from_fn(move || {
            while let Some(key) = iter.next() {
                let res = O::read(&mut connection_ref.lock().unwrap(), &key).ok()?;
                if filter.is_ok(&res) {
                    return Some(res);
                }
            }
            None
        });

        Ok(Box::new(iter))
    }
}

impl<T, U> Updateable<T, U> for RedisAdapter
where
    U: RedisUpdater<T> + Clone,
{
    fn update(&mut self, uuid: &str, data: U) -> Result<(), Box<dyn std::error::Error>> {
        let mut pipe = redis::pipe();
        pipe.atomic();
        data.clone().update(&mut pipe, uuid)?;

        let mut connection = self.connection.lock().unwrap();
        pipe.query(&mut connection)?;

        connection.publish(format!("{}:update:{}", EVENT_PREFIX, uuid), uuid)?;
        Ok(())
    }
}

impl Matcher for RedisAdapter {
    // NOTE: This function is a temporary inefficient implementation and will be migrated to a server-side lua script using channels
    fn on_match<T>(&self, handler: T)
    where
        T: Send + Sync + 'static + Fn(Match) -> (),
    {
        self.handlers.lock().unwrap().push(Arc::new(handler));
    }
}

impl<'a, T, O, F, U> DataAdapter<'a, T, O, F, U> for RedisAdapter
where
    T: Clone + RedisInsertWriter + RedisIdentifiable + 'a,
    F: RedisFilter<O> + Default + 'a,
    O: RedisOutputReader + RedisIdentifiable + 'a,
    U: RedisUpdater<T> + Clone + 'a,
{
}
