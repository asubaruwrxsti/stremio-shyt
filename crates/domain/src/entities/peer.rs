use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PeerStatus {
    Disconnected,
    Connecting,
    Connected,
    Banned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Peer {
    pub id: Option<i32>,
    pub torrent_id: i32,
    pub ip: String,
    pub port: u16,
    pub peer_id: Option<String>,
    pub last_seen: SystemTime,
    pub status: PeerStatus,
}

impl Peer {
    pub fn new(torrent_id: i32, ip: String, port: u16) -> Self {
        Self {
            id: None,
            torrent_id,
            ip,
            port,
            peer_id: None,
            last_seen: SystemTime::now(),
            status: PeerStatus::Disconnected,
        }
    }

    pub fn with_peer_id(mut self, peer_id: String) -> Self {
        self.peer_id = Some(peer_id);
        self
    }

    pub fn set_status(&mut self, status: PeerStatus) {
        self.status = status;
        self.last_seen = SystemTime::now();
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.status, PeerStatus::Connected)
    }

    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}
