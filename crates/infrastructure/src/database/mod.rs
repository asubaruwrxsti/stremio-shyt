use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};

pub mod schema;
pub use schema::*;

pub type SqlitePool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
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
