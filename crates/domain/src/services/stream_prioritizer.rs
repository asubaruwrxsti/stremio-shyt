use crate::entities::{StreamSession, Torrent};
use crate::repositories::TorrentRepository;
use crate::services::piece_manager::{PieceManager, PiecePriority};
use crate::errors::DomainError;
use std::sync::Arc;

/// Service responsible for prioritizing pieces for streaming
pub struct StreamPrioritizer {
    piece_manager: Arc<PieceManager>,
    torrent_repository: Arc<dyn TorrentRepository>,
}

impl StreamPrioritizer {
    pub fn new(
        piece_manager: Arc<PieceManager>,
        torrent_repository: Arc<dyn TorrentRepository>,
    ) -> Self {
        Self {
            piece_manager,
            torrent_repository,
        }
    }

    /// Prioritize pieces for streaming based on current position and buffer strategy
    pub async fn prioritize_for_streaming(
        &self,
        session: &StreamSession,
        current_position: u64,
        buffer_ahead_mb: usize,
    ) -> Result<(), DomainError> {
        let torrent = self.torrent_repository.find_by_id(session.torrent_id).await?
            .ok_or_else(|| DomainError::NotFound(format!("Torrent {} not found", session.torrent_id)))?;

        let piece_size = torrent.piece_length as u64;
        let buffer_size = (buffer_ahead_mb * 1024 * 1024) as u64;

        // Calculate piece range for current streaming position
        let start_piece = (current_position / piece_size) as usize;
        let end_piece = ((current_position + buffer_size) / piece_size) as usize;
        let total_pieces = torrent.piece_count as usize;

        // Prioritize immediate pieces as urgent
        let urgent_pieces = 5; // Next 5 pieces are urgent
        for piece_index in start_piece..=std::cmp::min(start_piece + urgent_pieces, total_pieces - 1) {
            if !self.piece_manager.is_piece_available(session.torrent_id, piece_index).await? {
                self.piece_manager.request_piece(
                    session.torrent_id,
                    piece_index,
                    PiecePriority::Urgent,
                    session.id.clone(),
                ).await?;
            }
        }

        // Prioritize buffer pieces as high
        for piece_index in (start_piece + urgent_pieces + 1)..=std::cmp::min(end_piece, total_pieces - 1) {
            if !self.piece_manager.is_piece_available(session.torrent_id, piece_index).await? {
                self.piece_manager.request_piece(
                    session.torrent_id,
                    piece_index,
                    PiecePriority::High,
                    session.id.clone(),
                ).await?;
            }
        }

        Ok(())
    }

    /// Sequential piece prioritization for typical video streaming
    pub async fn prioritize_sequential(
        &self,
        torrent_id: i32,
        file_offset: u64,
        file_size: u64,
        session_id: String,
    ) -> Result<(), DomainError> {
        let torrent = self.torrent_repository.find_by_id(torrent_id).await?
            .ok_or_else(|| DomainError::NotFound(format!("Torrent {} not found", torrent_id)))?;

        let piece_size = torrent.piece_length as u64;
        let start_piece = (file_offset / piece_size) as usize;
        let end_piece = ((file_offset + file_size - 1) / piece_size) as usize;

        // Prioritize the first part of the file as urgent (for quick playback start)
        let critical_pieces = 10; // First 10 pieces are critical
        for piece_index in start_piece..=std::cmp::min(start_piece + critical_pieces, end_piece) {
            if !self.piece_manager.is_piece_available(torrent_id, piece_index).await? {
                self.piece_manager.request_piece(
                    torrent_id,
                    piece_index,
                    PiecePriority::Urgent,
                    session_id.clone(),
                ).await?;
            }
        }

        // Rest of the file as normal priority
        for piece_index in (start_piece + critical_pieces + 1)..=end_piece {
            if !self.piece_manager.is_piece_available(torrent_id, piece_index).await? {
                self.piece_manager.request_piece(
                    torrent_id,
                    piece_index,
                    PiecePriority::Normal,
                    session_id.clone(),
                ).await?;
            }
        }

        Ok(())
    }

    /// Calculate optimal piece request pattern for streaming
    pub fn calculate_streaming_pattern(
        &self,
        torrent: &Torrent,
        current_position: u64,
        bandwidth_kbps: u32,
    ) -> StreamingPattern {
        let piece_size = torrent.piece_length as u64;
        let current_piece = (current_position / piece_size) as usize;
        
        // Calculate how many pieces we can buffer based on bandwidth
        // Assume we want 30 seconds of buffer at current bandwidth
        let buffer_seconds = 30;
        let bytes_per_second = (bandwidth_kbps * 1024) / 8; // Convert to bytes per second
        let buffer_bytes = bytes_per_second * buffer_seconds;
        let buffer_pieces = (buffer_bytes as u64 / piece_size) as usize;

        StreamingPattern {
            urgent_start: current_piece,
            urgent_count: 3, // Always keep 3 pieces urgent
            high_start: current_piece + 3,
            high_count: std::cmp::min(buffer_pieces, 20), // Max 20 pieces high priority
            normal_start: current_piece + 3 + std::cmp::min(buffer_pieces, 20),
            normal_count: 50, // Background download
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamingPattern {
    pub urgent_start: usize,
    pub urgent_count: usize,
    pub high_start: usize,
    pub high_count: usize,
    pub normal_start: usize,
    pub normal_count: usize,
}

impl StreamingPattern {
    /// Apply this pattern to piece requests
    pub async fn apply_to_piece_manager(
        &self,
        piece_manager: &PieceManager,
        torrent_id: i32,
        session_id: String,
        total_pieces: usize,
    ) -> Result<(), DomainError> {
        // Request urgent pieces
        for i in 0..self.urgent_count {
            let piece_index = self.urgent_start + i;
            if piece_index < total_pieces {
                if !piece_manager.is_piece_available(torrent_id, piece_index).await? {
                    piece_manager.request_piece(
                        torrent_id,
                        piece_index,
                        PiecePriority::Urgent,
                        session_id.clone(),
                    ).await?;
                }
            }
        }

        // Request high priority pieces
        for i in 0..self.high_count {
            let piece_index = self.high_start + i;
            if piece_index < total_pieces {
                if !piece_manager.is_piece_available(torrent_id, piece_index).await? {
                    piece_manager.request_piece(
                        torrent_id,
                        piece_index,
                        PiecePriority::High,
                        session_id.clone(),
                    ).await?;
                }
            }
        }

        // Request normal priority pieces
        for i in 0..self.normal_count {
            let piece_index = self.normal_start + i;
            if piece_index < total_pieces {
                if !piece_manager.is_piece_available(torrent_id, piece_index).await? {
                    piece_manager.request_piece(
                        torrent_id,
                        piece_index,
                        PiecePriority::Normal,
                        session_id.clone(),
                    ).await?;
                }
            }
        }

        Ok(())
    }
}
