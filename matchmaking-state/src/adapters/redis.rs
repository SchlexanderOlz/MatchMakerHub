use std::{
    collections::HashMap, future::Future, sync::{Arc, Mutex}
};

use crate::models::Match;

use super::{
    DataAdapter, Gettable, InfoPublisher, Insertable, Matcher, Publishable, Removable, Searchable,
    Updateable,
};
pub use redis::{Commands, Connection, FromRedisValue, Msg, Pipeline, PubSub, ToRedisArgs};
use tracing::{error, info};

mod io;
pub mod publisher;

#[derive(Default, Debug)]
pub struct MatchProposal {
    pub found_region: Option<String>,
    pub found_mode: Option<String>,
    pub found_game: Option<String>,
    pub found_players: HashMap<String, Vec<String>>,
    pub found_ai: Option<bool>,
}

impl MatchProposal {
    #[inline]
    pub fn is_complete(&self) -> bool {
        return !self.found_players.is_empty()
            && self.found_region.is_some()
            && self.found_ai.is_some();
    }
}

pub type RedisAdapterDefault = RedisAdapter<redis::Connection>;

// TODO: There are definetly some thread-mutability issues in the RedisAdapter due to the excesive use of Arc<Mutex>. Fix this in a #Refractoring
pub struct RedisAdapter<I> {
    pub client: redis::Client,
    auto_delete: Option<i64>,
    connection: Arc<Mutex<redis::Connection>>,
    publisher: Option<Arc<Mutex<dyn InfoPublisher<I> + Send + Sync>>>,
    handlers: Arc<Mutex<Vec<Arc<dyn Send + Sync + 'static + Fn(Match) -> ()>>>>,
}

impl<I> From<redis::Client> for RedisAdapter<I> {
    fn from(client: redis::Client) -> Self {
        let connection =
            Arc::new(Mutex::new(client.get_connection().expect(
                format!("Could not connect to redis server at {:?}", client).as_str(),
            )));
        Self {
            client,
            connection,
            publisher: None,
            handlers: Arc::new(Mutex::new(Vec::new())),
            auto_delete: None,
        }
    }
}

impl<I> Clone for RedisAdapter<I> {
    fn clone(&self) -> Self {
        let client = self.client.clone();
        Self {
            connection: Arc::new(Mutex::new(client.get_connection().unwrap())),
            publisher: self.publisher.clone(),
            client,
            handlers: self.handlers.clone(),
            auto_delete: self.auto_delete,
        }
    }
}

impl<I> RedisAdapter<I>
where
    I: 'static,
    std::string::String: Publishable<I>,
{
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

    pub fn with_publisher(
        mut self,
        publisher: impl InfoPublisher<I> + Send + Sync + 'static,
    ) -> Self {
        self.publisher = Some(Arc::new(Mutex::new(publisher)));
        self
    }

    pub fn with_auto_timeout(mut self, timeout: i64) -> Self {
        self.auto_delete = Some(timeout);
        self
    }

    pub fn reconnect(&self) -> Result<Connection, Box<dyn std::error::Error>> {
        Ok(self.client.get_connection()?)
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
        info!("Subscribed to match events");

        self.acc_searchers(connection)
    }

    fn acc_searchers(mut self, mut connection: PubSub) -> Result<(), Box<dyn std::error::Error>> {
        let mut match_proposal = MatchProposal::default();

        // TODO: Multithread this as soon as the problem with the order of messages is fixed.
        loop {
            let msg = connection.get_message().unwrap();
            info!("Message received: {:?}", msg);
            self.handle_msg(msg, &mut match_proposal);
        }
    }

    fn handle_msg(&mut self, msg: Msg, match_proposal: &mut MatchProposal) {
        let payload = msg.get_payload::<String>().unwrap();

        info!("Payload: {:?}", payload);

        let channel = msg.get_channel_name().split(":").collect::<Vec<&str>>();

        let last = channel.last().unwrap();
        let uuid = channel.first().unwrap().to_string();

        if last.to_string() == "region".to_string() {
            match_proposal.found_region = Some(payload);
            return;
        }

        if last.to_string() == "mode".to_string() {
            match_proposal.found_mode = Some(payload);
            return;
        }

        if last.to_string() == "game".to_string() {
            match_proposal.found_game = Some(payload);
            return;
        }

        if last.to_string() == "ai".to_string() {
            match_proposal.found_ai = Some(payload.parse::<i32>().unwrap() == 1);
            return;
        }

        if last.to_string() == "done".to_string() {
            self.on_done_msg(&uuid, payload, &match_proposal).unwrap();
            // TODO: The order of messages is likely but not guaranteed. This could be a potential error and should be handled accordingly.
            return;
        }

        if channel.get(channel.len() - 2).unwrap().to_string() == "players".to_string() {
            if let Some(players) = match_proposal.found_players.get_mut(&uuid) {
                players.push(payload);
                return;
            }
            match_proposal
                .found_players
                .insert(channel.first().unwrap().to_string(), vec![payload]);
        }
    }

    fn on_done_msg(
        &mut self,
        uuid: &String,
        payload: String,
        match_proposal: &MatchProposal,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
        let region = match_proposal.found_region.as_ref();
        if region.is_none() {
            todo!("Handle the server error accordingly");
        }
        let region = region.unwrap().clone();

        let players = match_proposal.found_players.get(uuid);
        if players.is_none() || players.unwrap().len() == 0 {
            todo!("Handle the player error accordingly");
        }
        let players = players.unwrap().clone();
        if players.len() as i32 != payload.parse::<i32>()? {
            todo!("Handle the player count error accordingly");
        }

        let new_match = Match {
            region,
            players: players.clone(),
            mode: match_proposal.found_mode.as_ref().unwrap().clone(),
            game: match_proposal.found_game.as_ref().unwrap().clone(),
            ai: match_proposal.found_ai.unwrap(),
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

        let self_clone = self.clone();
        Ok(tokio::task::spawn(async move {
            // TODO: Tasks should be joined in async
            for handle in handles {
                handle.await.unwrap();
            }

            players.iter().for_each(|player| {
                if let Err(err) =
                    self_clone.remove(&player.splitn(3, ":").take(2).collect::<Vec<_>>().join(":"))
                {
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

pub trait RedisExpireable {
    fn expire(&self, pipe: &mut Pipeline, base_key: &str, timeout: i64) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait RedisInsertWriter {
    fn write(&self, pipe: &mut Pipeline, base_key: &str) -> Result<(), Box<dyn std::error::Error>>;
}

/// TODO: Currently functions in this trait require the arguments to be static. This solution prohibits removing existing handlers.
/// This should be fixed by using some Context-Manager which provides the PubSub connection and handler. When the Context-Manager is dropped the handler and handler-thread should be killed.
/// This trait should also be moved to the super-module
pub trait NotifyOnRedisEvent<I> {
    fn on_update<T>(
        connection: &RedisAdapter<I>,
        handler: impl FnMut(T) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        T: FromRedisValue;

    fn on_insert<T>(
        connection: &RedisAdapter<I>,
        handler: impl FnMut(T) -> () + Send + Sync + 'static,
    ) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>>
    where
        T: FromRedisValue;

    fn on_delete<T>(
        connection: &RedisAdapter<I>,
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

impl<I> Removable for RedisAdapter<I>
where
    std::string::String: Publishable<I>,
{
    fn remove(&self, uuid: &str) -> Result<(), Box<dyn std::error::Error>> {
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

        if let Some(publisher) = self.publisher.as_ref() {
            publisher
                .lock()
                .unwrap()
                .publish(&uuid.to_string(), format!("remove:{uuid}"))?;
        }
        Ok(())
    }
}

impl<T, I> Insertable<T> for RedisAdapter<I>
where
    T: RedisInsertWriter + RedisExpireable + RedisIdentifiable + Clone,
    std::string::String: Publishable<I>,
{
    fn insert(&self, data: T) -> Result<String, Box<dyn std::error::Error>> {
        let key = { T::next_uuid(&mut self.connection.lock().unwrap())? };

        let mut pipe = redis::pipe();
        pipe.atomic();
        data.write(&mut pipe, &key)?;
        pipe.set(key.clone(), "");

        if let Some(auto_delete) = self.auto_delete {
            pipe.expire(key.clone(), auto_delete);
            data.expire(&mut pipe, &key, auto_delete)?;
        }

        'query: {
            let mut connection = self.connection.lock().unwrap();
            pipe.query(&mut connection)?;

            if self.publisher.is_none() {
                break 'query;
            }

            self.publisher
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .publish(&key, format!("insert:{key}"))?;
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

impl<'a, O, I> Gettable<'a, O> for RedisAdapter<I>
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

impl<'a, O, F, I> Searchable<'a, O, F> for RedisAdapter<I>
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

impl<T, U, I> Updateable<T, U> for RedisAdapter<I>
where
    U: RedisUpdater<T> + Clone,
    std::string::String: Publishable<I>,
{
    fn update(&self, uuid: &str, data: U) -> Result<(), Box<dyn std::error::Error>> {
        let mut pipe = redis::pipe();
        pipe.atomic();
        data.clone().update(&mut pipe, uuid)?;

        let mut connection = self.connection.lock().unwrap();
        pipe.query(&mut connection)?;

        if let Some(publisher) = self.publisher.as_ref() {
            publisher
                .lock()
                .unwrap()
                .publish(&uuid.to_string(), format!("update:{uuid}"))?;
        }
        Ok(())
    }
}

impl<I> Matcher for RedisAdapter<I> {
    fn on_match<T>(&self, handler: T)
    where
        T: Send + Sync + 'static + Fn(Match) -> (),
    {
        self.handlers.lock().unwrap().push(Arc::new(handler));
    }
}

impl<'a, T, O, F, U> DataAdapter<'a, T, O, F, U> for RedisAdapter<redis::Connection>
where
    T: Clone + RedisInsertWriter + RedisExpireable + RedisIdentifiable + 'a,
    O: RedisOutputReader + RedisIdentifiable + 'a,
    F: RedisFilter<O> + Default + 'a,
    U: RedisUpdater<T> + Clone + 'a,
{
}
