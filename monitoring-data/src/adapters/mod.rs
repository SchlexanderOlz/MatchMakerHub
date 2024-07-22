use std::fmt::format;

use redis::{Commands, Connection, JsonCommands, Pipeline, ToRedisArgs};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    models::{self, DBGameServer, GameMode, GameServer, GameServerFilter, RedisInsert},
    DataAdapter, Insertable, Removable, Searchable,
};

pub mod redis_adapter;

const BASE_SERVER: &str = "htps://";

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

pub trait RedisInsertWriter {
    fn write(&self, pipe: &mut Pipeline, base_key: &str) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait RedisOutputReader
where
    Self: Sized,
{
    fn read(connection: &mut Connection, base_key: &str) -> Result<Self, Box<dyn std::error::Error>>;
}

impl RedisInsertWriter for models::GameServer {
    fn write(&self, pipe: &mut Pipeline, base_key: &str) -> Result<(), Box<dyn std::error::Error>> {
        pipe.set(format!("{base_key}:name"), self.name.clone())
            .set(format!("{base_key}:server"), self.server.clone())
            .set(format!("{base_key}:token"), self.token.clone())
            .set(
                format!("{base_key}:modes:name"),
                self.modes
                    .iter()
                    .map(|x| x.name.clone())
                    .collect::<Vec<String>>(),
            )
            .set(
                format!("{base_key}:modes:player_count"),
                self.modes
                    .iter()
                    .map(|x| x.player_count)
                    .collect::<Vec<u32>>(),
            );
        Ok(())
    }
}

impl RedisOutputReader for models::DBGameServer {
    fn read(connection: &mut Connection, base_key: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut pipe = redis::pipe();
        pipe.atomic();
        pipe.get(format!("{base_key}:name"))
            .get(format!("{base_key}:modes:name"))
            .get(format!("{base_key}:modes:player_count"))
            .get(format!("{base_key}:server"))
            .get(format!("{base_key}:token"));

        let res: (String, Vec<String>, Vec<u32>, String, String) = pipe.query(connection)?;
        Ok(Self {
            uuid: base_key.to_owned(),
            name: res.0,
            modes: res
                .1
                .into_iter()
                .zip(res.2)
                .map(|x| GameMode {
                    name: x.0,
                    player_count: x.1,
                })
                .collect(),
            server: res.3,
            token: res.4,
        })
    }
}

impl Removable for RedisAdapter {
    fn remove(&mut self, uuid: String) -> Result<(), Box<dyn std::error::Error>> {
        Ok(self.connection.del(uuid)?)
    }
}

impl Insertable<models::GameServer> for RedisAdapter
{
    fn insert(&mut self, data: models::GameServer) -> Result<(), Box<dyn std::error::Error>> {
        let counter: i64 = self.connection.incr("uuid_inc", 1)?;
        let key = format!("{}:game_server", counter);

        let mut pipe = redis::pipe();
        pipe.atomic();
        data.write(&mut pipe, &key)?;
        pipe.set(key, "");
        pipe.query(&mut self.connection)?;

        Ok(())
    }
}

impl
    Searchable<
        models::GameServer,
        models::DBGameServer,
        models::GameServerFilter,
    > for RedisAdapter
{
    fn all(
        &mut self,
    ) -> Result<impl Iterator<Item = models::DBGameServer>, Box<dyn std::error::Error>> {
        self.filter(models::GameServerFilter { game: None })
    }

    fn filter(
        &mut self,
        filter: models::GameServerFilter,
    ) -> Result<impl Iterator<Item = models::DBGameServer>, Box<dyn std::error::Error>> {
        // ERROR: The filter is currently ignored
        _ = filter;

        let mut iter: redis::Iter<String> = self.connection.scan_match("*:game_server")?;
        let mut connection = self.client.get_connection()?;

        let iter = std::iter::from_fn(move || match iter.next() {
            Some(key) => {
                let game: models::DBGameServer = DBGameServer::read(&mut connection, &key).ok()?;
                Some(game)
            }
            _ => None,
        });

        Ok(iter)
    }

    fn get(&mut self, uuid: String) -> Result<models::DBGameServer, Box<dyn std::error::Error>> {
        DBGameServer::read(&mut self.connection, &uuid)
    }
}

impl
    DataAdapter<
        models::GameServer,
        models::DBGameServer,
        models::GameServerFilter,
    > for RedisAdapter
{
}
