use std::{
    collections::{HashMap, HashSet}, fmt::format, sync::{Arc, Mutex}
};

use crate::models::{DBProposedMatch, DBSearcher, Match, ProposedMatch, ProposedMatchUpdate};

use super::{
    DataAdapter, Gettable, Insertable, MatchAck, MatchHandler, Matcher, Removable, Searchable, Updateable
};
use redis::{Commands, Connection, Pipeline};

mod io;


pub struct RedisAdapter {
    client: redis::Client,
    connection: redis::Connection,
    handlers: Arc<Mutex<Vec<Arc<dyn MatchHandler>>>>,
}

impl From<redis::Client> for RedisAdapter {
    fn from(client: redis::Client) -> Self {
        let connection = client.get_connection().unwrap();
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
            connection: client.get_connection().unwrap(),
            client,
            handlers: self.handlers.clone(),
        }
    }
}

impl RedisAdapter {
    pub fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = redis::Client::open(url)?;
        Ok(Self::from(client))
    }

    pub fn run_match_check(&self) -> tokio::task::JoinHandle<()> {
        let self_clone = self.clone();
        tokio::task::spawn(async move {
            self_clone.start_match_check().unwrap();
        })
    }

    fn start_match_check(
        mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // NOTE: Result should be '!' for Ok values. This is currently expermintal tough and therefore not implemented here.
        let mut connection = self.client.get_connection()?;
        let mut connection = connection.as_pubsub();

        connection.subscribe("*:match:*")?;

        let mut found_servers: HashMap<String, String> = HashMap::new();
        let mut found_players: HashMap<String, Vec<String>> = HashMap::new();

        loop {
            let msg = connection.get_message().unwrap();

            let channel_name = msg.get_channel_name();
            let channel = channel_name.split(":").collect::<Vec<&str>>();

            let last = channel.last().unwrap();
            let uuid = channel.first().unwrap().to_string();

            let payload = msg.get_payload::<String>().unwrap();
            if last.to_string() == "server".to_string() {
                found_servers.insert(channel.first().unwrap().to_string(), payload);
                continue;
            }

            if last.to_string() == "done".to_string() {
                Self::on_done_msg(
                    &mut self,
                    &uuid,
                    payload,
                    &found_servers,
                    &found_players,
                )?;
                continue;
            }

            if channel.get(channel.len() - 2).unwrap().to_string() == "players".to_string() {
                if let Some(players) = found_players.get_mut(&uuid) {
                    players.push(payload);
                    continue;
                }
                found_players.insert(channel.first().unwrap().to_string(), vec![payload]);
            }
        }
    }

    fn on_done_msg(
        &mut self,
        uuid: &String,
        payload: String,
        found_servers: &HashMap<String, String>,
        found_players: &HashMap<String, Vec<String>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = found_servers.get(uuid);
        if server.is_none() {
            // TODO: Handle the server error accordingly
            todo!("Handle the server error accordingly");
        }
        let server = server.unwrap();

        let players = found_players.get(uuid);
        if players.is_none() {
            todo!("Handle the player error accordingly");
        }
        let players = players.unwrap();
        if players.len() as i32 != payload.parse::<i32>()? - 1 {
            todo!("Handle the player count error accordingly");
        }

        let new_match = Match {
            address: server.to_string(),
            players: players.clone(),
        };

        let proposed_match = ProposedMatch {
            match_uuid: uuid.to_string(),
            server: server.to_string(),
            invited: players.clone(),
            accepted: vec![],
            rejected: vec![],
        };

        let proposed_uuid = self.insert(proposed_match)?;

        let mut handles = self.handlers
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .map(move |fun| {
                let match_clone = new_match.clone();
                tokio::task::spawn(async move {
                    fun(match_clone)
                })
            });

        let mut self_clone = self.clone();

        tokio::task::spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build().unwrap();
            
            // TODO: The bellow code using the HashSet can probably be moved to the redis-db by using the Hashset type in redis
            // TODO: Transactions are to be used here
            handles.for_each(|handle| {
                match runtime.block_on(handle) {
                    Ok(MatchAck::Accept(players)) => {
                        let proposed_state: DBProposedMatch = self_clone.get(&proposed_uuid).unwrap();
                        let combined: HashSet<_> = HashSet::from_iter(proposed_state.accepted.into_iter().chain(players));
                        let update = ProposedMatchUpdate {
                            accepted: Some(combined.into_iter().collect()),
                            ..Default::default()
                        };
                        self_clone.update(&proposed_uuid, update).unwrap();
                    }
                    Ok(MatchAck::Decline(players)) => {
                        let proposed_state: DBProposedMatch = self_clone.get(&proposed_uuid).unwrap();
                        let combined: HashSet<_> = HashSet::from_iter(proposed_state.rejected.into_iter().chain(players));
                        let update = ProposedMatchUpdate {
                            rejected: Some(combined.into_iter().collect()),
                            ..Default::default()
                        };
                        self_clone.update(&proposed_uuid, update).unwrap();
                    }
                    Err(err) => {
                        todo!("{:?}", err)
                    }
                }
            });

            let proposed_state: DBProposedMatch = self_clone.get(&proposed_uuid).unwrap();
            if proposed_state.accepted.len() == proposed_state.invited.len() {
                let _: i32 = self_clone.connection.publish(format!("{}:match:accepted", proposed_uuid), true).unwrap();
            }

            if proposed_state.rejected.len() + proposed_state.accepted.len() == proposed_state.invited.len() {
                let _: i32 = self_clone.connection.publish(format!("{}:match:accepted", proposed_uuid), false).unwrap();
            }

            let mut connection = self_clone.connection.as_pubsub();
            connection.subscribe(format!("{}:match:accepted", proposed_uuid)).unwrap();
            loop {
                let msg = connection.get_message().unwrap();
                let accepted: bool = msg.get_payload().unwrap();
            }
        });
        Ok(())
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
        let iter = self
            .connection
            .scan_match(format!("{}*", uuid))?
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
    T: RedisInsertWriter + RedisIdentifiable + Clone,
{
    fn insert(&mut self, data: T) -> Result<String, Box<dyn std::error::Error>> {
        let key = T::next_uuid(&mut self.connection)?;

        let mut pipe = redis::pipe();
        pipe.atomic();
        data.write(&mut pipe, &key)?;
        pipe.set(key.clone(), "");
        pipe.query(&mut self.connection)?;

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

impl<O> Gettable<O> for RedisAdapter
where
    O: RedisOutputReader + RedisIdentifiable,
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
        O::read(&mut self.connection, uuid)
    }
}

impl<O, F> Searchable<O, F> for RedisAdapter
where
    O: RedisOutputReader + RedisIdentifiable,
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

impl<T, U> Updateable<T, U> for RedisAdapter
where
    U: RedisUpdater<T> + Clone,
{
    fn update(&mut self, uuid: &str, data: U) -> Result<(), Box<dyn std::error::Error>> {
        let mut pipe = redis::pipe();
        pipe.atomic();
        data.clone().update(&mut pipe, uuid)?;
        pipe.query(&mut self.connection)?;
        Ok(())
    }
}

impl Matcher for RedisAdapter
where
    Self: Gettable<DBSearcher> + Removable,
{
    // NOTE: This function is a temporary inefficient implementation and will be migrated to a server-side lua script using channels
    fn on_match(&mut self, handler: impl MatchHandler) {
        self.handlers.lock().unwrap().push(Arc::new(handler));
    }
}

impl<T, O, F, U> DataAdapter<T, O, F, U> for RedisAdapter
where
    T: Clone + RedisInsertWriter + RedisIdentifiable,
    F: RedisFilter<O> + Default,
    O: RedisOutputReader + RedisIdentifiable,
    U: RedisUpdater<T> + Clone,
{
}
