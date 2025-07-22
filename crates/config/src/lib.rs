use dotenv::dotenv;
use dotenv::from_path;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Config {
    pub database_dns: String,
    pub api_base_url: String,
    pub content_api_url: String,
    pub tmdb_api_key: String,
}

impl Config {
    /// Load configuration from a specified `.env` file path or default to the root `.env` file.
    pub fn from_env(env_path: Option<&str>) -> Self {
        // Load the specified `.env` file or default to the root `.env` file
        if let Some(path) = env_path {
            from_path(path).expect(&format!("Failed to load .env file from path: {}", path));
        } else {
            // Default to `.env` in the root directory
            dotenv().ok();
        }

        // If `DATABASE_DNS` is empty, fall back to SQLite
        let database_dns = env::var("DATABASE_DNS").unwrap_or_else(|_| "".to_string());

        let database_dns = if database_dns.is_empty() {
            // Compute a relative path to the `database` crate
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.pop(); // Move up one level to the `crates` directory
            path.push("database"); // Navigate to the `database` crate directory
            path.push("stremio.db"); // Append the database file name

            // Check if the file exists
            if !path.exists() {
                println!(
                    "Database file does not exist. Creating it at: {}",
                    path.display()
                );
                fs::File::create(&path).expect("Failed to create the database file");
            } else {
                println!("Database file already exists at: {}", path.display());
            }

            // Return the SQLite connection string
            format!("sqlite://{}", path.to_string_lossy())
        } else {
            // Use the provided `DATABASE_DNS`
            database_dns
        };

        Self {
            database_dns,
            api_base_url: env::var("API_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string()),
            content_api_url: env::var("CONTENT_API_URL")
                .unwrap_or_else(|_| "https://api.themoviedb.org/3".to_string()),
            tmdb_api_key: env::var("TMDB_API_KEY").expect("TMDB_API_KEY must be set"),
        }
    }
}
