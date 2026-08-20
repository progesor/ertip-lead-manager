use crate::db::Database;

pub struct AppState {
    pub database: Database,
}

impl AppState {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}
