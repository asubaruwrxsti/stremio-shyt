use application::TorrentApp;
use axum::{
    extract::{Multipart, Path, Query, State},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use domain::entities::{Torrent, TorrentStatus};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

// Simple API Response type
#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
        }
    }
    
    fn error(message: String) -> ApiResponse<T> {
        ApiResponse {
            success: false,
            data: None,
            message: Some(message),
        }
    }
}

#[derive(Serialize)]
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

#[derive(Deserialize)]
struct AddTorrentRequest {
    url: Option<String>,
}

#[derive(Deserialize)]
struct QueryParams {
    status: Option<String>,
}

#[derive(Clone)]
struct AppState {
    torrent_app: Arc<TorrentApp>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("api_server=debug,tower_http=debug")
        .init();

    info!("🚀 Starting Stremio BitTorrent API Server");

    // Get database path from environment or use default
    let database_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "stremio.db".to_string());
    info!("💾 Using database: {}", database_path);

    // Initialize the torrent application
    let torrent_app = Arc::new(TorrentApp::new(&database_path));
    let app_state = AppState { torrent_app };

    // Build our application with routes
    let app = Router::new()
        // Torrent management endpoints
        .route("/api/torrents", get(list_torrents).post(add_torrent))
        .route("/api/torrents/:id", get(get_torrent))
        .route("/api/torrents/:id/start", post(start_torrent))
        .route("/api/torrents/:id/pause", post(pause_torrent))
        .route("/api/torrents/:id/resume", post(resume_torrent))
        
        // Upload torrent file endpoint
        .route("/api/torrents/upload", post(upload_torrent_file))
        
        // System info endpoints
        .route("/api/status", get(get_system_status))
        
        // Health check
        .route("/health", get(health_check))
        
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    // Run the server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    info!("🌐 API Server listening on http://0.0.0.0:8080");
    info!("📖 API Documentation:");
    info!("   GET  /api/torrents          - List all torrents");
    info!("   POST /api/torrents          - Add torrent by URL");
    info!("   POST /api/torrents/upload   - Upload .torrent file");
    info!("   GET  /api/torrents/:id      - Get torrent details");
    info!("   POST /api/torrents/:id/start - Start torrent download");
    info!("   POST /api/torrents/:id/pause - Pause torrent");
    info!("   POST /api/torrents/:id/resume - Resume torrent");
    info!("   GET  /api/status             - System status");
    info!("   GET  /health                 - Health check");

    axum::serve(listener, app).await?;

    Ok(())
}

// API Handlers

/// Health check endpoint
async fn health_check() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse::success("BitTorrent API is running"))
}

/// Get system status
async fn get_system_status(State(state): State<AppState>) -> impl IntoResponse {
    match state.torrent_app.torrent_service.get_all_torrents().await {
        Ok(torrents) => {
            let total_torrents = torrents.len();
            let active_torrents = torrents.iter().filter(|t| matches!(t.status, TorrentStatus::Downloading)).count();
            let completed_torrents = torrents.iter().filter(|t| matches!(t.status, TorrentStatus::Completed)).count();
            
            let status = serde_json::json!({
                "total_torrents": total_torrents,
                "active_torrents": active_torrents,
                "completed_torrents": completed_torrents,
                "paused_torrents": torrents.iter().filter(|t| matches!(t.status, TorrentStatus::Paused)).count(),
            });
            
            Json(ApiResponse::success(status))
        }
        Err(e) => {
            warn!("Failed to get system status: {}", e);
            Json(ApiResponse::error(e.to_string()))
        }
    }
}

async fn list_torrents(
    State(state): State<AppState>,
    Query(params): Query<QueryParams>
) -> impl IntoResponse {
    let torrents_result = if let Some(status) = params.status {
        match status.as_str() {
            "active" => state.torrent_app.torrent_service.get_active_torrents().await,
            _ => state.torrent_app.torrent_service.get_all_torrents().await,
        }
    } else {
        state.torrent_app.torrent_service.get_all_torrents().await
    };

    match torrents_result {
        Ok(torrents) => {
            let torrent_infos: Vec<TorrentInfo> = torrents.into_iter().map(TorrentInfo::from).collect();
            Json(ApiResponse::success(torrent_infos))
        }
        Err(e) => {
            warn!("Failed to list torrents: {}", e);
            Json(ApiResponse::error(e.to_string()))
        }
    }
}

async fn add_torrent(
    State(state): State<AppState>,
    Json(request): Json<AddTorrentRequest>
) -> impl IntoResponse {
    if let Some(url) = request.url {
        if url.starts_with("magnet:") {
            return Json(ApiResponse::error("Magnet links not yet implemented".to_string()));
        }
        
        match download_torrent_from_url(&url).await {
            Ok(torrent_data) => {
                match state.torrent_app.torrent_service.add_torrent_from_file(torrent_data).await {
                    Ok(torrent) => Json(ApiResponse::success(TorrentInfo::from(torrent))),
                    Err(e) => {
                        warn!("Failed to add torrent from URL {}: {}", url, e);
                        Json(ApiResponse::error(e.to_string()))
                    }
                }
            }
            Err(e) => {
                warn!("Failed to download torrent from URL {}: {}", url, e);
                Json(ApiResponse::error(format!("Failed to download torrent: {}", e)))
            }
        }
    } else {
        Json(ApiResponse::error("URL required".to_string()))
    }
}

async fn upload_torrent_file(
    State(state): State<AppState>,
    mut multipart: Multipart
) -> impl IntoResponse {
    while let Some(field) = multipart.next_field().await.unwrap() {
        if field.name() == Some("torrent") {
            match field.bytes().await {
                Ok(torrent_data) => {
                    match state.torrent_app.torrent_service.add_torrent_from_file(torrent_data.to_vec()).await {
                        Ok(torrent) => return Json(ApiResponse::success(TorrentInfo::from(torrent))),
                        Err(e) => {
                            warn!("Failed to add uploaded torrent: {}", e);
                            return Json(ApiResponse::error(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read uploaded file: {}", e);
                    return Json(ApiResponse::error("Failed to read file".to_string()));
                }
            }
        }
    }
    
    Json(ApiResponse::error("No torrent file provided".to_string()))
}

async fn get_torrent(
    State(state): State<AppState>,
    Path(id): Path<i32>
) -> impl IntoResponse {
    match state.torrent_app.torrent_service.get_torrent(id).await {
        Ok(torrent) => Json(ApiResponse::success(TorrentInfo::from(torrent))),
        Err(e) => {
            warn!("Failed to get torrent {}: {}", id, e);
            Json(ApiResponse::error(e.to_string()))
        }
    }
}

async fn start_torrent(
    State(state): State<AppState>,
    Path(id): Path<i32>
) -> impl IntoResponse {
    match state.torrent_app.torrent_service.start_download(id).await {
        Ok(_) => {
            info!("Started torrent download: {}", id);
            Json(ApiResponse::success("Download started"))
        }
        Err(e) => {
            warn!("Failed to start torrent {}: {}", id, e);
            Json(ApiResponse::error(e.to_string()))
        }
    }
}

async fn pause_torrent(
    State(state): State<AppState>,
    Path(id): Path<i32>
) -> impl IntoResponse {
    match state.torrent_app.torrent_service.pause_torrent(id).await {
        Ok(_) => {
            info!("Paused torrent: {}", id);
            Json(ApiResponse::success("Torrent paused"))
        }
        Err(e) => {
            warn!("Failed to pause torrent {}: {}", id, e);
            Json(ApiResponse::error(e.to_string()))
        }
    }
}

async fn resume_torrent(
    State(state): State<AppState>,
    Path(id): Path<i32>
) -> impl IntoResponse {
    match state.torrent_app.torrent_service.resume_torrent(id).await {
        Ok(_) => {
            info!("Resumed torrent: {}", id);
            Json(ApiResponse::success("Torrent resumed"))
        }
        Err(e) => {
            warn!("Failed to resume torrent {}: {}", id, e);
            Json(ApiResponse::error(e.to_string()))
        }
    }
}

/// Helper function to download torrent from URL
async fn download_torrent_from_url(url: &str) -> Result<Vec<u8>, anyhow::Error> {
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("HTTP {}: {}", response.status(), response.status().canonical_reason().unwrap_or("Unknown")));
    }
    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}
