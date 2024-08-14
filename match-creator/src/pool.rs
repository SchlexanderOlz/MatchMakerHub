use std::sync::{Arc, Mutex};

use matchmaking_state::{adapters::{redis::{NotifyOnRedisEvent, RedisAdapter}, Gettable}, models::{DBGameServer, GameServer}};

#[derive(Clone)]
pub struct GameServerPool {
    pub servers: Arc<Mutex<Vec<DBGameServer>>>,
    connection: Arc<Mutex<RedisAdapter>>
}

impl Into<Vec<DBGameServer>> for GameServerPool {
    fn into(self) -> Vec<DBGameServer> {
        self.servers.lock().unwrap().clone()
    }
}

impl GameServerPool {
    pub fn new(adapter: Arc<Mutex<RedisAdapter>>) -> Self {
        Self {
            servers: Arc::new(Mutex::new(Vec::new())),
            connection: adapter
        }
    }

    #[inline]
    pub fn get_connection(&self) -> Arc<Mutex<RedisAdapter>> {
        self.connection.clone()
    }


    pub fn populate(&mut self) {
        let servers = self.connection.lock().unwrap().all().unwrap().collect::<Vec<DBGameServer>>(); 
        *self.servers.lock().unwrap() = servers;
    }

    #[inline]
    pub fn get_server_by_address(&self, address: &str) -> Option<DBGameServer> {
        self.servers.lock().unwrap().iter().find(|s| s.server == address).cloned()
    }

    pub fn start_auto_update(&self) {
        let adapter = self.connection.clone();
        let servers = self.servers.clone();

        GameServerPool::auto_update(adapter, servers);
    }

    /// TODO: With the current implementation of event handlers, the auto-update cannot be stopped once started.
    pub fn auto_update(adapter: Arc<Mutex<RedisAdapter>>, servers: Arc<Mutex<Vec<DBGameServer>>>) -> () {
        let adapter_lock = adapter.lock().unwrap();
        let adapter_copy = adapter.clone();
        let server_copy = servers.clone();

        GameServer::on_insert(&adapter_lock, move |uuid: String| {
            let mut server_lock = server_copy.lock().unwrap();
            let server: DBGameServer = adapter_copy.lock().unwrap().get(uuid.as_str()).unwrap(); // TODO: Handle error

            if server_lock.contains(&server) {
                return;
            }
            server_lock.push(server)
        }).unwrap();

        let adapter_copy = adapter.clone();
        let server_copy = servers.clone();
        GameServer::on_update(&adapter_lock, move |uuid: String| {
            let server: DBGameServer = adapter_copy.lock().unwrap().get(uuid.as_str()).unwrap(); // TODO: Handle error

            let mut servers = server_copy.lock().unwrap();
            let index = servers.iter().position(|s| s.uuid == server.uuid).unwrap(); // TODO: Handle error

            servers[index] = server;
        }).unwrap();

        let adapter_copy = adapter.clone();
        GameServer::on_delete(&adapter_lock, move |uuid: String| {
            let server: DBGameServer = adapter_copy.lock().unwrap().get(uuid.as_str()).unwrap(); // TODO: Handle error

            let mut servers = servers.lock().unwrap();
            servers.retain(|s| s.uuid != server.uuid);
        }).unwrap();
    }
}