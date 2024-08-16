use crate::models::Match;

#[cfg(feature = "redis")]
pub mod redis;

pub trait DataAdapter<'a, T, O, F, U>:
    Insertable<T> + Searchable<'a, O, F> + Removable + Gettable<'a, O> + Updateable<T, U> + Matcher
{
}

pub trait Updateable<T, U> {
    fn update(&self, uuid: &str, change: U) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait Insertable<T> {
    fn insert(&self, data: T) -> Result<String, Box<dyn std::error::Error>>;
}

pub trait Searchable<'a, O, F> {
    type Type: Iterator<Item = O>;

    fn filter(&'a self, filter: F) -> Result<Self::Type, Box<dyn std::error::Error>>;
}

pub trait Gettable<'a, O> {
    type Type: Iterator<Item = O>;

    fn get(&'a self, uuid: &str) -> Result<O, Box<dyn std::error::Error>>;
    fn all(&'a self) -> Result<Self::Type, Box<dyn std::error::Error>>;
}

pub trait Removable {
    fn remove(&mut self, uuid: &str) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait Matcher {
    fn on_match<T>(&self, handler: T)
    where
        T: Send + Sync + 'static + Fn(Match) -> ();
}
