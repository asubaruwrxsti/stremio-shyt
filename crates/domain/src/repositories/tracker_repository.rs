use crate::entities::Tracker;
use crate::errors::DomainError;
use async_trait::async_trait;

#[async_trait]
pub trait TrackerRepository: Send + Sync {
    async fn find_by_torrent_id(&self, torrent_id: i32) -> Result<Vec<Tracker>, DomainError>;
    async fn find_active(&self, torrent_id: i32) -> Result<Vec<Tracker>, DomainError>;
    async fn save(&self, tracker: &Tracker) -> Result<Tracker, DomainError>;
    async fn update(&self, tracker: &Tracker) -> Result<Tracker, DomainError>;
    async fn save_batch(&self, trackers: &[Tracker]) -> Result<Vec<Tracker>, DomainError>;
}
