#[derive(Debug, Clone)]
pub struct Match {
    pub region: String,
    pub game: String,
    pub players: Vec<String>,
    pub mode: String,
    pub ai: bool,
}

unsafe impl Send for Match {}
unsafe impl Sync for Match {}

