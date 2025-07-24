use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_path: String,
    pub api_host: String,
    pub api_port: u16,
    pub download_dir: String,
    pub streaming_buffer_size_mb: usize,
}

impl Config {
    pub fn from_env() -> Self {
        // Load .env file if it exists
        dotenv::dotenv().ok();

        Config {
            database_path: env::var("DATABASE_PATH")
                .unwrap_or_else(|_| "stremio.db".to_string()),
            
            api_host: env::var("API_HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            
            api_port: env::var("API_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            
            download_dir: env::var("DOWNLOAD_DIR")
                .unwrap_or_else(|_| "downloads".to_string()),
            
            streaming_buffer_size_mb: env::var("STREAMING_BUFFER_SIZE_MB")
                .unwrap_or_else(|_| "64".to_string())
                .parse()
                .unwrap_or(64),
        }
    }
}
