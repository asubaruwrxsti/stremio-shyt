use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TorrentStatus {
    Parsing,
    Connecting,
    Downloading,
    Seeding,
    Paused,
    Completed,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Torrent {
    pub id: Option<i32>,
    pub info_hash: String,         // SHA1 hash as hex string
    pub name: String,
    pub total_size: i64,
    pub piece_length: i32,
    pub piece_count: i32,
    pub file_path: Option<String>,
    pub status: TorrentStatus,
    pub progress: f32,             // 0.0 to 1.0
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl Torrent {
    pub fn new(
        info_hash: String,
        name: String,
        total_size: i64,
        piece_length: i32,
        piece_count: i32,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            id: None,
            info_hash,
            name,
            total_size,
            piece_length,
            piece_count,
            file_path: None,
            status: TorrentStatus::Parsing,
            progress: 0.0,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_id(
        id: i32,
        info_hash: String,
        name: String,
        total_size: i64,
        piece_length: i32,
        piece_count: i32,
        file_path: Option<String>,
        status: TorrentStatus,
        progress: f32,
        created_at: SystemTime,
        updated_at: SystemTime,
    ) -> Self {
        Self {
            id: Some(id),
            info_hash,
            name,
            total_size,
            piece_length,
            piece_count,
            file_path,
            status,
            progress,
            created_at,
            updated_at,
        }
    }

    pub fn update_progress(&mut self, downloaded_pieces: i32) {
        self.progress = (downloaded_pieces as f32) / (self.piece_count as f32);
        self.updated_at = SystemTime::now();
        
        if self.progress >= 1.0 {
            self.status = TorrentStatus::Completed;
        }
    }

    pub fn set_status(&mut self, status: TorrentStatus) {
        self.status = status;
        self.updated_at = SystemTime::now();
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.status, TorrentStatus::Completed) || self.progress >= 1.0
    }
}
