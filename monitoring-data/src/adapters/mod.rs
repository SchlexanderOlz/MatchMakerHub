#[cfg(feature = "redis")]
pub mod redis;

pub trait DataAdapter<T, O, F, U>:
    Insertable<T> + Searchable<O, F> + Removable + Gettable<O> + Updateable<T, U>
{
}

pub trait Updateable<T, U> {
    fn update(&mut self, uuid: &str, change: U) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait Insertable<T> {
    fn insert(&mut self, data: T) -> Result<String, Box<dyn std::error::Error>>;
}

pub trait Searchable<O, F> {
    fn filter(&mut self, filter: F) -> Result<impl Iterator<Item = O>, Box<dyn std::error::Error>>;
}

pub trait Gettable<O> {
    fn get(&mut self, uuid: &str) -> Result<O, Box<dyn std::error::Error>>;
    fn all(&mut self) -> Result<impl Iterator<Item = O>, Box<dyn std::error::Error>>;
}

pub trait Removable {
    fn remove(&mut self, uuid: &str) -> Result<(), Box<dyn std::error::Error>>;
}
