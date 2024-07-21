use serde::{Deserialize, Serialize};

pub enum FilterOptions {
    Game,
    Match,
    Searcher
}

pub trait TypeInfo {
    fn get_type_info(&self) -> FilterOptions;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub name: String
}


impl TypeInfo for Game {
    fn get_type_info(&self) -> FilterOptions {
        FilterOptions::Game
    }

}

pub struct Searcher {
    pub id: String
}

impl TypeInfo for Searcher {
    fn get_type_info(&self) -> FilterOptions {
        FilterOptions::Searcher
    }
}

pub struct Match {

}

impl TypeInfo for Match {
    fn get_type_info(&self) -> FilterOptions {
        FilterOptions::Match
    }
}