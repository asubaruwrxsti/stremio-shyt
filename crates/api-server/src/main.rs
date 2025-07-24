use application::TorrentApp;
use domain::StreamingService;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
    http::{StatusCode, HeaderMap, header},
    body::Body,
};
use domain::entities::{Torrent, TorrentStatus};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use anyhow::Result;

mod config;
use config::Config;

#[derive(Clone)]
struct AppState {
    torrent_app: Arc<TorrentApp>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AddTorrentRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
struct TorrentInfo {
    id: Option<i32>,
    info_hash: String,
    name: String,
    total_size: i64,
    piece_length: i32,
    piece_count: i32,
    status: TorrentStatus,
    progress: f32,
}

impl From<Torrent> for TorrentInfo {
    fn from(torrent: Torrent) -> Self {
        Self {
            id: torrent.id,
            info_hash: torrent.info_hash,
            name: torrent.name,
            total_size: torrent.total_size,
            piece_length: torrent.piece_length,
            piece_count: torrent.piece_count,
            status: torrent.status,
            progress: torrent.progress,
        }
    }
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    message: String,
    version: String,
    environment: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("api_server=debug,tower_http=debug")
        .init();

    info!("🚀 Starting Stremio BitTorrent API Server");

    // Load configuration from environment
    let config = Config::from_env();
    
    info!("💾 Using database: {}", config.database_path);
    info!("🌐 API server will bind to: {}:{}", config.api_host, config.api_port);

    // Initialize the torrent application with configuration
    let torrent_app = Arc::new(TorrentApp::new_with_config(
        &config.database_path,
        &config.download_dir,
        config.streaming_buffer_size_mb,
    ));
    let app_state = AppState { torrent_app };

    // Build our application with routes
    let app = Router::new()
        // Basic torrent management endpoints
        .route("/api/torrents", get(list_torrents).post(add_torrent))
        .route("/api/torrents/:id", get(get_torrent))
        
        // Streaming endpoints
        .route("/api/torrents/:id/files", get(get_streamable_files))
        .route("/api/torrents/:id/stream/:file_index", post(create_stream_session))
        .route("/api/stream/:session_id", get(stream_content))
        .route("/api/streams", get(list_active_streams))
        
        // System info endpoints
        .route("/api/status", get(get_system_status))
        
        // Health check
        .route("/health", get(health_check))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    // Run the server
    let bind_address = format!("{}:{}", config.api_host, config.api_port);
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    info!("🌐 API Server listening on http://{}", bind_address);
    info!("📖 API Documentation:");
    info!("   GET  /api/torrents          - List all torrents");
    info!("   POST /api/torrents          - Add torrent by URL");
    info!("   GET  /api/torrents/:id      - Get torrent details");
    info!("   GET  /api/torrents/:id/files - Get streamable files");
    info!("   POST /api/torrents/:id/stream/:file_index - Create stream session");
    info!("   GET  /api/stream/:session_id - Stream content (supports range requests)");
    info!("   GET  /api/streams            - List active streams");
    info!("   GET  /api/status             - System status");
    info!("   GET  /health                 - Health check");

    axum::serve(listener, app).await?;

    Ok(())
}

// Handler functions
async fn list_torrents(State(state): State<AppState>) -> impl IntoResponse {
    match state.torrent_app.torrent_service.get_all_torrents().await {
        Ok(torrents) => {
            let torrent_infos: Vec<TorrentInfo> = torrents.into_iter().map(Into::into).collect();
            Json(torrent_infos).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to list torrents: {}", e)).into_response()
        }
    }
}

async fn add_torrent(
    State(state): State<AppState>,
    Json(payload): Json<AddTorrentRequest>,
) -> impl IntoResponse {
    info!("📥 Adding torrent from URL: {}", payload.url);
    
    // Fetch the torrent file from URL
    let torrent_data = match fetch_torrent_from_url(&payload.url).await {
        Ok(data) => data,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("Failed to fetch torrent: {}", e)).into_response();
        }
    };
    
    // Parse the torrent file
    let _torrent_info = match parse_torrent_file(&torrent_data) {
        Ok(info) => info,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("Failed to parse torrent: {}", e)).into_response();
        }
    };
    
    // Add to database via torrent service
    match state.torrent_app.torrent_service.add_torrent_from_file(torrent_data).await {
        Ok(torrent) => {
            info!("✅ Successfully added torrent: {}", torrent.name);
            let torrent_info: TorrentInfo = torrent.into();
            (StatusCode::CREATED, Json(torrent_info)).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to add torrent to database: {}", e)).into_response()
        }
    }
}

// Helper function to fetch torrent file from URL
async fn fetch_torrent_from_url(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    info!("🌐 Fetching torrent file from: {}", url);
    
    let response = reqwest::get(url).await?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()).into());
    }
    
    let content_type = response.headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("");
    
    if !content_type.contains("application/x-bittorrent") && !url.ends_with(".torrent") {
        info!("⚠️  Warning: Content-Type is '{}', expected torrent file", content_type);
    }
    
    let bytes = response.bytes().await?;
    info!("📦 Downloaded {} bytes", bytes.len());
    
    Ok(bytes.to_vec())
}

// Helper function to parse and validate torrent file
fn parse_torrent_file(data: &[u8]) -> Result<bip_metainfo::Metainfo, String> {
    let metainfo = bip_metainfo::Metainfo::from_bytes(data)
        .map_err(|e| format!("Failed to parse torrent file: {}", e))?;
    
    info!("📋 Torrent info:");
    info!("   Files: {}", metainfo.info().files().count());
    info!("   Piece length: {} bytes", metainfo.info().piece_length());
    info!("   Pieces: {}", metainfo.info().pieces().count());
    
    Ok(metainfo)
}

async fn get_torrent(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    match state.torrent_app.torrent_service.get_torrent(id).await {
        Ok(torrent) => {
            let torrent_info: TorrentInfo = torrent.into();
            Json(torrent_info).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get torrent: {}", e)).into_response()
        }
    }
}

async fn get_system_status() -> impl IntoResponse {
    let status = StatusResponse {
        message: "Stremio BitTorrent API Server is running".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        environment: std::env::var("ENV").unwrap_or_else(|_| "development".to_string()),
    };
    Json(status)
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// Streaming endpoint handlers
async fn get_streamable_files(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    match state.torrent_app.streaming_service.get_streamable_files(id).await {
        Ok(files) => Json(files).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get streamable files: {}", e)).into_response()
    }
}

async fn create_stream_session(
    State(state): State<AppState>,
    Path((torrent_id, file_index)): Path<(i32, usize)>,
) -> impl IntoResponse {
    match state.torrent_app.streaming_service.create_stream_session(torrent_id, file_index).await {
        Ok(session) => {
            info!("🎬 Created streaming session {} for torrent {} file {}", session.id, torrent_id, file_index);
            Json(session).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create stream session: {}", e)).into_response()
    }
}

async fn stream_content(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Get session to determine file size for range parsing
    let session = match state.torrent_app.streaming_service.get_stream_session(&session_id).await {
        Ok(session) => session,
        Err(e) => return (StatusCode::NOT_FOUND, format!("Stream session not found: {}", e)).into_response()
    };
    
    // Parse range header if present
    let range = headers.get(header::RANGE)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| parse_range_header(s, session.file_size as u64));
    
    match state.torrent_app.streaming_service.stream_content(&session_id, range).await {
        Ok(data) => {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(header::CONTENT_TYPE, "application/octet-stream".parse().unwrap());
            response_headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
            
            if data.len() > 0 {
                (StatusCode::OK, response_headers, Body::from(data)).into_response()
            } else {
                (StatusCode::NO_CONTENT, response_headers, Body::empty()).into_response()
            }
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to stream content: {}", e)).into_response()
    }
}

async fn list_active_streams(State(state): State<AppState>) -> impl IntoResponse {
    match state.torrent_app.streaming_service.get_active_sessions().await {
        Ok(sessions) => Json(sessions).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to list active streams: {}", e)).into_response()
    }
}

// Helper function to parse HTTP Range header
fn parse_range_header(range_str: &str, total_size: u64) -> Option<domain::entities::StreamRange> {
    if let Some(bytes_range) = range_str.strip_prefix("bytes=") {
        if let Some((start_str, end_str)) = bytes_range.split_once('-') {
            let start = start_str.parse::<u64>().ok()?;
            let end = if end_str.is_empty() { 
                None 
            } else { 
                end_str.parse::<u64>().ok() 
            };
            
            return Some(domain::entities::StreamRange { 
                start, 
                end, 
                total_size 
            });
        }
    }
    None
}
