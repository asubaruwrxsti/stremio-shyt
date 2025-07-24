use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_path: String,
    pub api_host: String,
    pub api_port: u16,
    pub download_dir: String,
    pub max_peers: usize,
    pub piece_timeout_seconds: u64,
    pub connection_timeout_seconds: u64,
    pub streaming_buffer_size_mb: usize,
    pub max_concurrent_streams: usize,
    pub stream_chunk_size_kb: usize,
    pub content_api_url: String,
    pub tmdb_api_key: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        // Load .env file if it exists
        if let Err(e) = dotenv::dotenv() {
            println!("Warning: Could not load .env file: {}", e);
        }

        let config = Config {
            database_path: env::var("DATABASE_PATH")
                .unwrap_or_else(|_| "stremio.db".to_string()),
            
            api_host: env::var("API_HOST")
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            
            api_port: env::var("API_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .map_err(|e| format!("Invalid API_PORT: {}", e))?,
            
            download_dir: env::var("DOWNLOAD_DIR")
                .unwrap_or_else(|_| "downloads".to_string()),
            
            max_peers: env::var("MAX_PEERS")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .map_err(|e| format!("Invalid MAX_PEERS: {}", e))?,
            
            piece_timeout_seconds: env::var("PIECE_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .map_err(|e| format!("Invalid PIECE_TIMEOUT_SECONDS: {}", e))?,
            
            connection_timeout_seconds: env::var("CONNECTION_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|e| format!("Invalid CONNECTION_TIMEOUT_SECONDS: {}", e))?,
            
            streaming_buffer_size_mb: env::var("STREAMING_BUFFER_SIZE_MB")
                .unwrap_or_else(|_| "64".to_string())
                .parse()
                .map_err(|e| format!("Invalid STREAMING_BUFFER_SIZE_MB: {}", e))?,
            
            max_concurrent_streams: env::var("MAX_CONCURRENT_STREAMS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|e| format!("Invalid MAX_CONCURRENT_STREAMS: {}", e))?,
            
            stream_chunk_size_kb: env::var("STREAM_CHUNK_SIZE_KB")
                .unwrap_or_else(|_| "256".to_string())
                .parse()
                .map_err(|e| format!("Invalid STREAM_CHUNK_SIZE_KB: {}", e))?,
            
            content_api_url: env::var("CONTENT_API_URL")
                .unwrap_or_else(|_| "https://api.themoviedb.org/3".to_string()),
            
            tmdb_api_key: env::var("TMDB_API_KEY").ok(),
        };

        // Create download directory if it doesn't exist
        std::fs::create_dir_all(&config.download_dir)?;

        Ok(config)
    }

    pub fn print_config(&self) {
        println!("📋 Configuration loaded:");
        println!("  🗄️  Database: {}", self.database_path);
        println!("  🌐 API Server: {}:{}", self.api_host, self.api_port);
        println!("  📁 Download Directory: {}", self.download_dir);
        println!("  👥 Max Peers: {}", self.max_peers);
        println!("  ⏱️  Piece Timeout: {}s", self.piece_timeout_seconds);
        println!("  🔗 Connection Timeout: {}s", self.connection_timeout_seconds);
        println!("  💾 Streaming Buffer: {}MB", self.streaming_buffer_size_mb);
        println!("  🎬 Max Concurrent Streams: {}", self.max_concurrent_streams);
        println!("  📦 Stream Chunk Size: {}KB", self.stream_chunk_size_kb);
        println!("  🎭 Content API: {}", self.content_api_url);
        
        if self.tmdb_api_key.is_some() {
            println!("  🔑 TMDB API Key: ✅ Configured");
        } else {
            println!("  🔑 TMDB API Key: ❌ Not configured");
        }
    }

    pub fn api_address(&self) -> String {
        format!("{}:{}", self.api_host, self.api_port)
    }
}
