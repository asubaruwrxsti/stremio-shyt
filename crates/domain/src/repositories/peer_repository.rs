use crate::entities::Peer;
use crate::errors::DomainError;
use async_trait::async_trait;

#[async_trait]
pub trait PeerRepository: Send + Sync {
    async fn find_by_torrent_id(&self, torrent_id: i32) -> Result<Vec<Peer>, DomainError>;
    async fn find_connected(&self, torrent_id: i32) -> Result<Vec<Peer>, DomainError>;
    async fn save(&self, peer: &Peer) -> Result<Peer, DomainError>;
    async fn update(&self, peer: &Peer) -> Result<Peer, DomainError>;
    async fn save_batch(&self, peers: &[Peer]) -> Result<Vec<Peer>, DomainError>;
    async fn delete_old(&self, torrent_id: i32, hours: u32) -> Result<(), DomainError>;
}
