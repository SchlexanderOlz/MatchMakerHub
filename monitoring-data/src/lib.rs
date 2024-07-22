use std::{fmt::Debug, ops::Neg};

use redis::Iter;

pub mod models;
pub mod adapters;


const BASE_SERVER: &str = "127.0.0.1:3456"; // TODO: Default server address should later be changed to a hostname resolved by a dns


pub trait DataAdapter<T, G, F> : Insertable<T> + Searchable<T, G, F> + Removable
where T: Clone
 {
    // fn host(url: &str) -> Result<(), Box<dyn std::error::Error>>;
    // fn update<T>(&mut self, data: T) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait Insertable<T> 
where T: Clone
{
    fn insert(&mut self, data: T) -> Result<(), Box<dyn std::error::Error>>; 
}

pub trait Searchable<T, G, F> 
where T: Clone
{
    fn all(&mut self) -> Result<impl Iterator<Item = G>, Box<dyn std::error::Error>>;
    fn filter(&mut self, filter: F) -> Result<impl Iterator<Item = G>, Box<dyn std::error::Error>>;
    fn get(&mut self, uuid: String) -> Result<G, Box<dyn std::error::Error>>;
}

pub trait Removable {
    fn remove(&mut self, uuid: String) -> Result<(), Box<dyn std::error::Error>>;
}
