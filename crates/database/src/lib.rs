use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};

pub mod schema;
pub mod interfaces;
pub mod repositories;

// Define type alias for SQLite connection pool
pub type SqlitePool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create a new SQLite database instance
    pub fn new(database_path: &str) -> Self {
        let manager = ConnectionManager::<SqliteConnection>::new(database_path);
        let pool = r2d2::Pool::builder()
            .build(manager)
            .expect("Failed to create SQLite connection pool");
        Database { pool }
    }

    pub fn get_pool(&self) -> &SqlitePool {
        &self.pool
    }
}
