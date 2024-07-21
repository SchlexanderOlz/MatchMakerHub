use redis::{Commands, JsonCommands};
use serde_json::{json, Value};

use crate::{models::{self, Game}, DataAdapter, FilterFor};

pub mod redis_adapter;


const BASE_SERVER: &str = "htps://";
pub struct RedisData {}

pub struct RedisData2 {

}

pub struct RedisAdapter {
    pub client: redis::Client,
    pub connection: redis::Connection
}

impl RedisAdapter {
    pub fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = redis::Client::open(url)?;
        Ok(Self { connection: client.get_connection()?, client })
    }
}


impl Into<serde_json::Value> for models::Game {
    fn into(self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl From<serde_json::Value> for models::Game {
    fn from(data: serde_json::Value) -> Self {
        serde_json::from_value(data).unwrap()
    }
}


#[derive(Debug, Clone)]
pub struct GameFilter {
    pub name: String
}

impl FilterFor<models::Game> for GameFilter {} // TODO: Write a proc-macro


impl DataAdapter<models::Game, GameFilter, serde_json::Value> for RedisAdapter
 {
    fn insert(&mut self, data: models::Game) -> Result<(), Box<dyn std::error::Error>>
     {
        let res: Result<(), redis::RedisError> = self.connection.json_arr_append::<_, _, Value, _>("game", "$", &data.clone().into()); // TODO: Change the path to the game-name for games. Will improve efficiency
        if let Err(e) = res {
            // TODO: Find the correcet error here
            println!("Error: {:?}", e);
            println!("{}", e.detail().unwrap());
            println!("{}", e.category());

            self.connection.json_set("game", "$", &json!([]))?;
            self.insert(data)?;
        }

        Ok(())
    }

    fn find(&self, filter: GameFilter) -> Result<Vec<models::Game>, Box<dyn std::error::Error>> {
        Ok(vec![])
    }

    fn remove(&mut self, data: models::Game) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}




// Continue here later writing the missing implementations
// Write the implementations for the (non-redis) Node Adapter using specification for the individual types