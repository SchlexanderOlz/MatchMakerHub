pub mod adapters;
pub mod models;

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{
        adapters::{Gettable, Insertable, Removable, Updateable},
        models::{DBGameServer, GameMode, GameServerUpdater},
    };

    #[test]
    #[cfg(feature = "redis")]

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
    #[cfg(feature = "redis")]
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

        let found_server = adapter.all().unwrap().collect::<Vec<DBGameServer>>();

        for game in &found_server {
            println!("{:?}", game);
        }

        assert!(found_server.len() > 0);
        assert!(found_server.iter().any(|x| x.name == game_server.name));
    }

    #[test]
    #[cfg(feature = "redis")]
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

    #[test]
    #[cfg(feature = "redis")]
    fn test_redis_adapter_update_game_server() {
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
            server: "0.0.0.0".to_owned(),
            token: "test_token".to_owned(),
        };
        let uuid = adapter.insert(game_server.clone()).unwrap();

        let mut update = GameServerUpdater::default();
        update.name = Some("CSS Battle (Cum Sum Sus Battle)".to_owned());
        adapter.update(&uuid, update).unwrap();

        let result: DBGameServer = adapter.get(&uuid).unwrap();

        assert!(result.name == "CSS Battle (Cum Sum Sus Battle)");
    }
}
