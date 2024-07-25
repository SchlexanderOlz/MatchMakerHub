pub mod adapters;
pub mod models;

const BASE_SERVER: &str = "127.0.0.1:3456"; // TODO: Default server address should later be changed to a hostname resolved by a dns

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{
        adapters::{Insertable, Removable, Searchable, Gettable},
        models::{DBGameServer, GameMode, GameServerFilter},
    };

    #[test]

    fn test_redis_adapter_insert_game_server() {
        use crate::adapters::redis::RedisAdapter;
        use crate::models::{GameMode, GameServer};

        let mut adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();

        let game_server = GameServer {
            name: "Test Server".to_owned(),
            modes: vec![GameMode {
                name: "Test Mode".to_owned(),
                player_count: 10,
                computer_lobby: false,
            }],
            server: "127.0.0.1:3456".to_owned(),

            token: "test_token".to_owned(),
        };
        adapter.insert(game_server).unwrap();
    }

    #[test]
    fn test_redis_adapter_all_game_server() {
        use crate::adapters::redis::RedisAdapter;
        use crate::models::GameServer;

        let mut adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();

        let game_server = GameServer {
            name: "Test Server".to_owned(),
            modes: vec![GameMode {
                name: "Test Mode".to_owned(),
                player_count: 10,
                computer_lobby: false,
            }],
            server: "127.0.0.1:3456".to_owned(),
            token: "test_token".to_owned(),
        };
        adapter.insert(game_server.clone()).unwrap();

        let found_server = adapter
            .all()
            .unwrap()
            .collect::<Vec<DBGameServer>>();

        for game in &found_server {
            println!("{:?}", game);
        }

        assert!(found_server.len() > 0);
        assert!(found_server.iter().any(|x| x.name == game_server.name));
    }

    #[test]
    fn test_redis_adapter_remove_game_server() {
        use crate::adapters::redis::RedisAdapter;
        use crate::models::GameServer;

        let mut adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();

        let game_server = GameServer {
            name: "Test Server".to_owned(),
            modes: vec![GameMode {
                name: "Test Mode".to_owned(),
                player_count: 10,
                computer_lobby: false,
            }],
            server: "0.0.0.0".to_owned(),
            token: "test_token".to_owned(),
        };
        let uuid = adapter.insert(game_server.clone()).unwrap();

        adapter.remove(&uuid).unwrap();

        let game: Result<DBGameServer, Box<dyn Error>> = adapter.get(&uuid);
        assert!(game.is_err());
    }
}
