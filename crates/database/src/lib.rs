use config::Config;
use sqlx::SqlitePool;

/// Initialize the SQLite database connection pool
pub async fn init_db() -> SqlitePool {
    let config = Config::from_env(None);

    SqlitePool::connect(&config.database_dns)
        .await
        .expect("Failed to connect to the SQLite database")
}
