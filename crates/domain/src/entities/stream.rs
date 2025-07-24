use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamSession {
    pub id: String,
    pub torrent_id: i32,
    pub file_index: usize,
    pub file_name: String,
    pub file_size: i64,
    pub mime_type: String,
    pub started_at: SystemTime,
    pub last_accessed: SystemTime,
    pub bytes_served: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamRange {
    pub start: u64,
    pub end: Option<u64>,
    pub total_size: u64,
}

impl StreamRange {
    pub fn new(start: u64, end: Option<u64>, total_size: u64) -> Self {
        let end = end.unwrap_or(total_size.saturating_sub(1));
        Self {
            start,
            end: Some(end.min(total_size.saturating_sub(1))),
            total_size,
        }
    }

    pub fn length(&self) -> u64 {
        self.end.unwrap_or(self.total_size.saturating_sub(1)) - self.start + 1
    }

    pub fn content_range_header(&self) -> String {
        format!(
            "bytes {}-{}/{}",
            self.start,
            self.end.unwrap_or(self.total_size.saturating_sub(1)),
            self.total_size
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileInfo {
    pub index: usize,
    pub name: String,
    pub path: String,
    pub size: i64,
    pub offset: i64, // Start position in the torrent
    pub mime_type: String,
    pub is_streamable: bool,
}

impl FileInfo {
    pub fn is_video(&self) -> bool {
        self.mime_type.starts_with("video/")
    }

    pub fn is_audio(&self) -> bool {
        self.mime_type.starts_with("audio/")
    }

    pub fn detect_mime_type(filename: &str) -> String {
        let extension = std::path::Path::new(filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            // Video formats
            "mp4" => "video/mp4",
            "mkv" => "video/x-matroska",
            "avi" => "video/x-msvideo",
            "mov" => "video/quicktime",
            "wmv" => "video/x-ms-wmv",
            "flv" => "video/x-flv",
            "webm" => "video/webm",
            "m4v" => "video/x-m4v",
            "3gp" => "video/3gpp",
            "ogv" => "video/ogg",

            // Audio formats
            "mp3" => "audio/mpeg",
            "flac" => "audio/flac",
            "wav" => "audio/wav",
            "aac" => "audio/aac",
            "ogg" => "audio/ogg",
            "m4a" => "audio/x-m4a",
            "wma" => "audio/x-ms-wma",

            // Default
            _ => "application/octet-stream",
        }.to_string()
    }
}
