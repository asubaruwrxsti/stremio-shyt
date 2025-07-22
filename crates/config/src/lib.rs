use dotenv::dotenv;
use dotenv::from_path;
use std::env;

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
            dotenv().ok(); // Default to `.env` in the root directory
        }

        // Load environment variables into the configuration struct
        Self {
            database_dns: env::var("DATABASE_DNS").expect("DATABASE_DNS must be set"),
            api_base_url: env::var("API_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string()),
            content_api_url: env::var("CONTENT_API_URL")
                .unwrap_or_else(|_| "https://api.themoviedb.org/3".to_string()),
            tmdb_api_key: env::var("TMDB_API_KEY").expect("TMDB_API_KEY must be set"),
        }
    }
}
