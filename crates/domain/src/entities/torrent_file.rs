use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TorrentFile {
    pub id: Option<i32>,
    pub torrent_id: i32,
    pub path: String,      // File path within the torrent
    pub length: i64,       // File size in bytes
    pub offset: i64,       // Byte offset within the torrent
}

impl TorrentFile {
    pub fn new(torrent_id: i32, path: String, length: i64, offset: i64) -> Self {
        Self {
            id: None,
            torrent_id,
            path,
            length,
            offset,
        }
    }

    pub fn end_offset(&self) -> i64 {
        self.offset + self.length
    }

    pub fn contains_byte(&self, byte_offset: i64) -> bool {
        byte_offset >= self.offset && byte_offset < self.end_offset()
    }

    pub fn file_name(&self) -> Option<&str> {
        self.path.split('/').last()
    }
}
