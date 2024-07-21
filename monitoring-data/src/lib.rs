use std::{fmt::Debug, ops::Neg};

use models::TypeInfo;

pub mod models;
pub mod adapters;


const BASE_SERVER: &str = "127.0.0.1:3456"; // TODO: Default server address should later be changed to a hostname resolved by a dns


pub trait DataAdapter<T, K, D>
where T: Into<D> + From<D> + Clone, K: FilterFor<T> + Clone 
 {
    // fn host(url: &str) -> Result<(), Box<dyn std::error::Error>>;

    fn insert(&mut self, data: T) -> Result<(), Box<dyn std::error::Error>>; 
    fn find(&self, filter: K) -> Result<Vec<T>, Box<dyn std::error::Error>>;
    fn remove(&mut self, data: T) -> Result<(), Box<dyn std::error::Error>>;
    // fn update<T>(&mut self, data: T) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait FilterFor<T> {
}
