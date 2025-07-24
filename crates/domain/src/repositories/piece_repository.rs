use crate::entities::Piece;
use crate::errors::DomainError;
use async_trait::async_trait;

#[async_trait]
pub trait PieceRepository: Send + Sync {
    async fn find_by_torrent_id(&self, torrent_id: i32) -> Result<Vec<Piece>, DomainError>;
    async fn find_by_torrent_and_index(&self, torrent_id: i32, piece_index: i32) -> Result<Option<Piece>, DomainError>;
    async fn save(&self, piece: &Piece) -> Result<Piece, DomainError>;
    async fn update(&self, piece: &Piece) -> Result<Piece, DomainError>;
    async fn save_batch(&self, pieces: &[Piece]) -> Result<Vec<Piece>, DomainError>;
    async fn count_downloaded(&self, torrent_id: i32) -> Result<i32, DomainError>;
    async fn find_next_needed(&self, torrent_id: i32, limit: i32) -> Result<Vec<Piece>, DomainError>;
}
