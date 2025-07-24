use crate::entities::Piece;
use crate::errors::DomainError;
use crate::repositories::{PieceRepository, TorrentRepository};
use std::sync::Arc;

/// Service for managing piece downloads and verification
/// Handles: download pieces → verify SHA1 → write to file/stream
pub struct DownloadService {
    piece_repository: Arc<dyn PieceRepository>,
    torrent_repository: Arc<dyn TorrentRepository>,
}

impl DownloadService {
    pub fn new(
        piece_repository: Arc<dyn PieceRepository>,
        torrent_repository: Arc<dyn TorrentRepository>,
    ) -> Self {
        Self {
            piece_repository,
            torrent_repository,
        }
    }

    /// Get the next N pieces that need to be downloaded
    /// This implements the "start downloading first N pieces" part of your flow
    pub async fn get_next_pieces_to_download(
        &self,
        torrent_id: i32,
        count: i32,
    ) -> Result<Vec<Piece>, DomainError> {
        self.piece_repository
            .find_next_needed(torrent_id, count)
            .await
    }

    /// Mark a piece as downloaded and verify its hash
    /// This implements: verify SHA1 hash of each piece
    pub async fn complete_piece(
        &self,
        torrent_id: i32,
        piece_index: i32,
        data: Vec<u8>,
    ) -> Result<bool, DomainError> {
        let mut piece = self
            .piece_repository
            .find_by_torrent_and_index(torrent_id, piece_index)
            .await?
            .ok_or(DomainError::ValidationError(format!(
                "Piece {} not found",
                piece_index
            )))?;

        // Verify SHA1 hash
        let calculated_hash = self.calculate_sha1(&data);
        let verified = calculated_hash == piece.hash;

        if verified {
            piece.mark_downloaded();
            piece.mark_verified(true);

            // Write piece data to local file or stream buffer
            self.write_piece_data(torrent_id, piece_index, data).await?;
            println!("✅ Piece {} verified and written", piece_index);
        } else {
            piece.mark_verified(false);
            eprintln!("❌ Piece {} failed verification", piece_index);
        }

        self.piece_repository.update(&piece).await?;

        if !verified {
            return Err(DomainError::PieceVerificationFailed(piece_index));
        }

        Ok(verified)
    }

    /// Calculate SHA1 hash of piece data
    fn calculate_sha1(&self, data: &[u8]) -> String {
        use sha1::{Digest, Sha1};
        let mut hasher = Sha1::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Write piece data to local file or stream buffer
    /// This implements: write to local file or stream buffer
    async fn write_piece_data(
        &self,
        torrent_id: i32,
        piece_index: i32,
        data: Vec<u8>,
    ) -> Result<(), DomainError> {
        let torrent = self
            .torrent_repository
            .find_by_id(torrent_id)
            .await?
            .ok_or(DomainError::TorrentNotFound(torrent_id))?;

        if let Some(file_path) = &torrent.file_path {
            // Calculate byte offset for this piece
            let offset = (piece_index as i64) * (torrent.piece_length as i64);

            // Implement actual file writing with proper offset
            use tokio::fs::OpenOptions;
            use tokio::io::{AsyncSeekExt, AsyncWriteExt};

            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(file_path)
                .await
                .map_err(|e| {
                    DomainError::IoError(format!("Failed to open file {}: {}", file_path, e))
                })?;

            file.seek(std::io::SeekFrom::Start(offset as u64))
                .await
                .map_err(|e| {
                    DomainError::IoError(format!("Failed to seek to offset {}: {}", offset, e))
                })?;

            file.write_all(&data)
                .await
                .map_err(|e| DomainError::IoError(format!("Failed to write piece data: {}", e)))?;

            file.flush()
                .await
                .map_err(|e| DomainError::IoError(format!("Failed to flush file: {}", e)))?;
        } else {
            // Store in memory buffer for streaming
            // For now, just create a temporary file
            let temp_path = format!(
                "./downloads/torrent_{}_piece_{}.tmp",
                torrent_id, piece_index
            );
            if let Some(parent) = std::path::Path::new(&temp_path).parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| DomainError::IoError(e.to_string()))?;
            }

            tokio::fs::write(&temp_path, data)
                .await
                .map_err(|e| DomainError::IoError(e.to_string()))?;
        }

        Ok(())
    }

    /// Check if torrent download is complete
    pub async fn is_download_complete(&self, torrent_id: i32) -> Result<bool, DomainError> {
        let torrent = self
            .torrent_repository
            .find_by_id(torrent_id)
            .await?
            .ok_or(DomainError::TorrentNotFound(torrent_id))?;

        let downloaded_pieces = self.piece_repository.count_downloaded(torrent_id).await?;

        Ok(downloaded_pieces >= torrent.piece_count)
    }

    /// Get download progress for a torrent
    pub async fn get_download_progress(&self, torrent_id: i32) -> Result<f32, DomainError> {
        let torrent = self
            .torrent_repository
            .find_by_id(torrent_id)
            .await?
            .ok_or(DomainError::TorrentNotFound(torrent_id))?;

        let downloaded_pieces = self.piece_repository.count_downloaded(torrent_id).await?;

        Ok((downloaded_pieces as f32) / (torrent.piece_count as f32))
    }

    /// Prepare torrent for streaming (prioritize first pieces)
    /// Returns whether the torrent is ready for streaming
    pub async fn prepare_for_streaming(&self, torrent_id: i32) -> Result<bool, DomainError> {
        let torrent = self
            .torrent_repository
            .find_by_id(torrent_id)
            .await?
            .ok_or(DomainError::TorrentNotFound(torrent_id))?;

        // Check if we have enough pieces downloaded to start streaming
        let downloaded_pieces = self.piece_repository.count_downloaded(torrent_id).await?;
        let min_pieces_for_streaming = (torrent.piece_count as f32 * 0.05).max(10.0) as i32; // Need at least 5% or 10 pieces

        let ready_for_streaming = downloaded_pieces >= min_pieces_for_streaming;

        if ready_for_streaming {
            println!("🎬 Torrent '{}' is ready for streaming", torrent.name);
            println!("✅ Downloaded {} pieces (minimum: {})", downloaded_pieces, min_pieces_for_streaming);
        } else {
            println!(
                "⚠️  Torrent '{}' needs {} more pieces for streaming (have {}, need {})",
                torrent.name,
                min_pieces_for_streaming - downloaded_pieces,
                downloaded_pieces,
                min_pieces_for_streaming
            );
        }

        Ok(ready_for_streaming)
    }
}
