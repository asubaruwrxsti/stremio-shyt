use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TrackerStatus {
    Active,
    Failed,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tracker {
    pub id: Option<i32>,
    pub torrent_id: i32,
    pub url: String,
    pub status: TrackerStatus,
    pub last_announce: Option<SystemTime>,
    pub next_announce: Option<SystemTime>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub completed: Option<i32>,
}

impl Tracker {
    pub fn new(torrent_id: i32, url: String) -> Self {
        Self {
            id: None,
            torrent_id,
            url,
            status: TrackerStatus::Active,
            last_announce: None,
            next_announce: None,
            seeders: None,
            leechers: None,
            completed: None,
        }
    }

    pub fn mark_announce_success(&mut self, interval: u32) {
        let now = SystemTime::now();
        self.last_announce = Some(now);
        self.next_announce = Some(now + std::time::Duration::from_secs(interval as u64));
        self.status = TrackerStatus::Active;
    }

    pub fn mark_announce_failed(&mut self) {
        self.status = TrackerStatus::Failed;
    }

    pub fn update_stats(&mut self, seeders: i32, leechers: i32, completed: i32) {
        self.seeders = Some(seeders);
        self.leechers = Some(leechers);
        self.completed = Some(completed);
    }

    pub fn should_announce(&self) -> bool {
        match self.next_announce {
            Some(next) => SystemTime::now() >= next,
            None => true, // First announce
        }
    }
}
