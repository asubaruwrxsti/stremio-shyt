use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    pub id: Option<i32>,
    pub torrent_id: i32,
    pub piece_index: i32,
    pub hash: String,           // SHA1 hash as hex string
    pub downloaded: bool,
    pub verified: bool,
}

impl Piece {
    pub fn new(torrent_id: i32, piece_index: i32, hash: String) -> Self {
        Self {
            id: None,
            torrent_id,
            piece_index,
            hash,
            downloaded: false,
            verified: false,
        }
    }

    pub fn mark_downloaded(&mut self) {
        self.downloaded = true;
    }

    pub fn mark_verified(&mut self, verified: bool) {
        self.verified = verified;
        if !verified {
            // If verification failed, mark as not downloaded
            self.downloaded = false;
        }
    }

    pub fn is_complete(&self) -> bool {
        self.downloaded && self.verified
    }
}
