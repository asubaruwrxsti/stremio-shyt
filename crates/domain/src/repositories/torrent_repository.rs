use crate::entities::Torrent;
use crate::errors::DomainError;
use async_trait::async_trait;

#[async_trait]
pub trait TorrentRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<Torrent>, DomainError>;
    async fn find_by_info_hash(&self, info_hash: &str) -> Result<Option<Torrent>, DomainError>;
    async fn save(&self, torrent: &Torrent) -> Result<Torrent, DomainError>;
    async fn update(&self, torrent: &Torrent) -> Result<Torrent, DomainError>;
    async fn delete(&self, id: i32) -> Result<(), DomainError>;
    async fn find_all(&self) -> Result<Vec<Torrent>, DomainError>;
    async fn find_active(&self) -> Result<Vec<Torrent>, DomainError>;
}
