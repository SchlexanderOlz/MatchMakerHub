pub mod adapters;
pub mod models;
pub mod prelude;

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{
        adapters::{Gettable, Insertable, Removable, Updateable},
        models::{DBGameServer, GameServerUpdater},
    };

    #[test]
    #[cfg(feature = "redis")]

    fn test_redis_adapter_insert_game_server() {
        use crate::adapters::redis::publisher::native::RedisInfoPublisher;
        use crate::adapters::redis::RedisAdapter;
        use crate::models::{GameServer};

        let adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();
        let publisher = RedisInfoPublisher::new(adapter.client.get_connection().unwrap());
        let adapter = adapter.with_publisher(publisher);


        let game_server = GameServer {
            region: "eu".to_owned(),
            game: "Test Server".to_owned(),
            mode: "Test Mode".to_owned(),
            server_pub: "127.0.0.1:3456".to_owned(),
            server_priv: "127.0.0.1:3456".to_owned(),
            healthy: true,
        };
        adapter.insert(game_server).unwrap();
    }

    #[test]
    #[cfg(feature = "redis")]
    fn test_redis_adapter_all_game_server() {
        use crate::adapters::redis::publisher::native::RedisInfoPublisher;
        use crate::adapters::redis::{RedisAdapter};
        use crate::models::GameServer;

        let adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();
        let publisher = RedisInfoPublisher::new(adapter.client.get_connection().unwrap());
        let adapter = adapter.with_publisher(publisher);



        let game_server = GameServer {
            region: "eu".to_owned(),
            game: "Test Server".to_owned(),
            mode: "Test Mode".to_owned(),
            server_pub: "127.0.0.1:3456".to_owned(),
            server_priv: "127.0.0.1:3456".to_owned(),
            healthy: true,
        };
        adapter.insert(game_server.clone()).unwrap();

        let found_server = adapter.all().unwrap().collect::<Vec<DBGameServer>>();

        for game in &found_server {
            println!("{:?}", game);
        }

        assert!(found_server.len() > 0);
        assert!(found_server.iter().any(|x| x.game == game_server.game));
    }

    #[test]
    #[cfg(feature = "redis")]
    fn test_redis_adapter_remove_game_server() {
        use crate::adapters::redis::publisher::native::RedisInfoPublisher;
        use crate::adapters::redis::{RedisAdapter};
        use crate::models::GameServer;

        let adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();
        let publisher = RedisInfoPublisher::new(adapter.client.get_connection().unwrap());
        let adapter = adapter.with_publisher(publisher);

        let game_server = GameServer {
            region: "eu".to_owned(),
            game: "Test Server".to_owned(),
            mode: "Test Mode".to_owned(),
            server_pub: "127.0.0.1:3456".to_owned(),
            server_priv: "127.0.0.1:3456".to_owned(),
            healthy: true,
        };
        let uuid = adapter.insert(game_server.clone()).unwrap();

        adapter.remove(&uuid).unwrap();

        let game: Result<DBGameServer, Box<dyn Error>> = adapter.get(&uuid);
        assert!(game.is_err());
    }

    #[test]
    #[cfg(feature = "redis")]
    fn test_redis_adapter_update_game_server() {
        use crate::adapters::redis::publisher::native::RedisInfoPublisher;
        use crate::adapters::redis::{RedisAdapter};
        use crate::models::{GameServer};

        let adapter = RedisAdapter::connect("redis://0.0.0.0:6379").unwrap();
        let publisher = RedisInfoPublisher::new(adapter.client.get_connection().unwrap());
        let adapter = adapter.with_publisher(publisher);


        let game_server = GameServer {
            region: "eu".to_owned(),
            game: "Test Server".to_owned(),
            mode: "Test Mode".to_owned(),
            server_pub: "127.0.0.1:3456".to_owned(),
            server_priv: "127.0.0.1:3456".to_owned(),
            healthy: true,
        };
        let uuid = adapter.insert(game_server.clone()).unwrap();

        let mut update = GameServerUpdater::default();
        update.game = Some("CSS Battle (Cum Sum Sus Battle)".to_owned());
        adapter.update(&uuid, update).unwrap();

        let result: DBGameServer = adapter.get(&uuid).unwrap();

        assert!(result.game == "CSS Battle (Cum Sum Sus Battle)");
    }
}
