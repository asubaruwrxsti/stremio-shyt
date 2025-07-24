use crate::entities::StreamSession;
use crate::errors::DomainError;
use crate::repositories::TorrentRepository;
use crate::services::piece_manager::PieceManager;
use crate::services::piece_downloader::PieceDownloader;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, VecDeque};
use tokio::time::{Duration, Instant};

/// Production-ready streaming buffer that prefetches pieces for smooth playback
pub struct StreamingBuffer {
    piece_manager: Arc<PieceManager>,
    piece_downloader: Arc<PieceDownloader>,
    torrent_repository: Arc<dyn TorrentRepository>,
    buffers: Arc<Mutex<HashMap<String, SessionBuffer>>>,
    buffer_size_mb: usize,
}

#[derive(Debug)]
struct SessionBuffer {
    _session_id: String,
    torrent_id: i32,
    current_position: u64,
    buffer_queue: VecDeque<BufferedPiece>,
    last_access: Instant,
    buffer_ahead_pieces: usize,
}

#[derive(Debug, Clone)]
struct BufferedPiece {
    piece_index: usize,
    data: Vec<u8>,
    offset_in_torrent: u64,
}

impl StreamingBuffer {
    pub fn new(
        piece_manager: Arc<PieceManager>,
        piece_downloader: Arc<PieceDownloader>,
        torrent_repository: Arc<dyn TorrentRepository>,
        buffer_size_mb: usize,
    ) -> Self {
        Self {
            piece_manager,
            piece_downloader,
            torrent_repository,
            buffers: Arc::new(Mutex::new(HashMap::new())),
            buffer_size_mb,
        }
    }

    /// Initialize buffer for a streaming session
    pub async fn initialize_session_buffer(&self, session: &StreamSession, file_offset: u64) -> Result<(), DomainError> {
        let torrent = self.torrent_repository.find_by_id(session.torrent_id).await?
            .ok_or_else(|| DomainError::NotFound(format!("Torrent {} not found", session.torrent_id)))?;

        let piece_size = torrent.piece_length as u64;
        let buffer_pieces = (self.buffer_size_mb * 1024 * 1024) / piece_size as usize;

        let session_buffer = SessionBuffer {
            _session_id: session.id.clone(),
            torrent_id: session.torrent_id,
            current_position: file_offset,
            buffer_queue: VecDeque::new(),
            last_access: Instant::now(),
            buffer_ahead_pieces: buffer_pieces.max(10), // At least 10 pieces ahead
        };

        {
            let mut buffers = self.buffers.lock().unwrap();
            buffers.insert(session.id.clone(), session_buffer);
        }

        // Start background buffering
        self.start_background_buffering(session.id.clone()).await?;

        Ok(())
    }

    /// Get buffered data for a range
    pub async fn get_buffered_data(&self, session_id: &str, start_offset: u64, length: u64) -> Result<Vec<u8>, DomainError> {
        let torrent_id = {
            let mut buffers = self.buffers.lock().unwrap();
            let buffer = buffers.get_mut(session_id)
                .ok_or_else(|| DomainError::NotFound(format!("Buffer for session {} not found", session_id)))?;

            buffer.last_access = Instant::now();
            buffer.current_position = start_offset;

            buffer.torrent_id
        };
        
        let pieces_needed = self.calculate_pieces_for_range(torrent_id, start_offset, length).await?;

        // Check if we have the pieces in buffer
        let mut result_data = Vec::new();
        let mut missing_pieces = Vec::new();

        {
            let buffers = self.buffers.lock().unwrap();
            let buffer = buffers.get(session_id).unwrap();

            for piece_index in pieces_needed {
                if let Some(buffered_piece) = buffer.buffer_queue.iter().find(|p| p.piece_index == piece_index) {
                    // Calculate the portion of this piece we need
                    let piece_data = self.extract_piece_range(
                        &buffered_piece.data,
                        buffered_piece.offset_in_torrent,
                        start_offset,
                        length,
                        result_data.len() as u64,
                    );
                    result_data.extend_from_slice(&piece_data);
                } else {
                    missing_pieces.push(piece_index);
                }
            }
        }

        // If we're missing pieces, try to download them immediately
        if !missing_pieces.is_empty() {
            for piece_index in missing_pieces {
                // Priority download for missing pieces
                self.piece_manager.request_piece(
                    torrent_id,
                    piece_index,
                    crate::services::piece_manager::PiecePriority::Urgent,
                    session_id.to_string(),
                ).await?;

                // Try to read from piece manager (might be available now)
                if let Ok(piece_data) = self.piece_manager.read_piece_data(torrent_id, piece_index).await {
                    let torrent = self.torrent_repository.find_by_id(torrent_id).await?
                        .ok_or_else(|| DomainError::NotFound(format!("Torrent {} not found", torrent_id)))?;
                    
                    let piece_offset = piece_index as u64 * torrent.piece_length as u64;
                    let piece_data_slice = self.extract_piece_range(
                        &piece_data,
                        piece_offset,
                        start_offset,
                        length,
                        result_data.len() as u64,
                    );
                    result_data.extend_from_slice(&piece_data_slice);
                }
            }
        }

        // Update buffer after access
        self.update_buffer_position(session_id, start_offset + length).await?;

        Ok(result_data)
    }

    async fn calculate_pieces_for_range(&self, torrent_id: i32, start_offset: u64, length: u64) -> Result<Vec<usize>, DomainError> {
        let torrent = self.torrent_repository.find_by_id(torrent_id).await?
            .ok_or_else(|| DomainError::NotFound(format!("Torrent {} not found", torrent_id)))?;

        let piece_size = torrent.piece_length as u64;
        let start_piece = (start_offset / piece_size) as usize;
        let end_piece = ((start_offset + length - 1) / piece_size) as usize;

        Ok((start_piece..=end_piece).collect())
    }

    fn extract_piece_range(&self, piece_data: &[u8], piece_offset: u64, range_start: u64, range_length: u64, _already_extracted: u64) -> Vec<u8> {
        let piece_end = piece_offset + piece_data.len() as u64 - 1;
        let range_end = range_start + range_length - 1;

        // Calculate intersection
        let extract_start = range_start.max(piece_offset);
        let extract_end = range_end.min(piece_end);

        if extract_start > extract_end {
            return Vec::new();
        }

        // Calculate offsets within the piece
        let piece_start_offset = (extract_start - piece_offset) as usize;
        let piece_end_offset = (extract_end - piece_offset + 1) as usize;

        piece_data[piece_start_offset..piece_end_offset].to_vec()
    }

    async fn start_background_buffering(&self, session_id: String) -> Result<(), DomainError> {
        let piece_manager = Arc::clone(&self.piece_manager);
        let _piece_downloader = Arc::clone(&self.piece_downloader);
        let torrent_repository = Arc::clone(&self.torrent_repository);
        let buffers = Arc::clone(&self.buffers);

        tokio::spawn(async move {
            loop {
                // Check if session still exists and is active
                let (torrent_id, current_pos, buffer_ahead) = {
                    let buffers_guard = buffers.lock().unwrap();
                    if let Some(buffer) = buffers_guard.get(&session_id) {
                        // Check if session is still active (accessed within last 5 minutes)
                        if buffer.last_access.elapsed() > Duration::from_secs(300) {
                            break; // Session inactive, stop buffering
                        }
                        (buffer.torrent_id, buffer.current_position, buffer.buffer_ahead_pieces)
                    } else {
                        break; // Session not found, stop buffering
                    }
                };

                // Get torrent info
                if let Ok(Some(torrent)) = torrent_repository.find_by_id(torrent_id).await {
                    let piece_size = torrent.piece_length as u64;
                    let current_piece = (current_pos / piece_size) as usize;

                    // Prefetch pieces ahead
                    for i in 0..buffer_ahead {
                        let piece_index = current_piece + i;
                        if piece_index < torrent.piece_count as usize {
                            // Check if piece is already available
                            if let Ok(false) = piece_manager.is_piece_available(torrent_id, piece_index).await {
                                // Request piece with high priority for buffering
                                let _ = piece_manager.request_piece(
                                    torrent_id,
                                    piece_index,
                                    crate::services::piece_manager::PiecePriority::High,
                                    session_id.clone(),
                                ).await;
                            }
                        }
                    }
                }

                // Wait before next buffering cycle
                tokio::time::sleep(Duration::from_secs(1)).await;
            }

            // Clean up session buffer when done
            let mut buffers_guard = buffers.lock().unwrap();
            buffers_guard.remove(&session_id);
        });

        Ok(())
    }

    async fn update_buffer_position(&self, session_id: &str, new_position: u64) -> Result<(), DomainError> {
        let mut buffers = self.buffers.lock().unwrap();
        if let Some(buffer) = buffers.get_mut(session_id) {
            buffer.current_position = new_position;
            buffer.last_access = Instant::now();

            // Remove pieces that are behind current position to free memory
            let torrent = futures::executor::block_on(async {
                self.torrent_repository.find_by_id(buffer.torrent_id).await
            })?;

            if let Some(torrent) = torrent {
                let piece_size = torrent.piece_length as u64;
                let current_piece = (new_position / piece_size) as usize;
                
                // Keep only pieces that are current or ahead
                buffer.buffer_queue.retain(|piece| piece.piece_index >= current_piece.saturating_sub(2));
            }
        }

        Ok(())
    }

    /// Clean up inactive sessions
    pub async fn cleanup_inactive_sessions(&self) {
        let mut buffers = self.buffers.lock().unwrap();
        let inactive_threshold = Duration::from_secs(600); // 10 minutes

        buffers.retain(|_, buffer| {
            buffer.last_access.elapsed() < inactive_threshold
        });
    }
}
