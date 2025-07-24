use crate::entities::Torrent;
use crate::errors::DomainError;
use crate::repositories::{PieceRepository, TorrentRepository};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use std::path::PathBuf;
use sha1::Digest;

#[derive(Debug, Clone)]
pub struct PieceRequest {
    pub piece_index: usize,
    pub priority: PiecePriority,
    pub requester: String, // Session ID or download ID
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PiecePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Urgent = 3, // For streaming
}

pub struct PieceManager {
    piece_repository: Arc<dyn PieceRepository>,
    torrent_repository: Arc<dyn TorrentRepository>,
    pending_requests: Arc<Mutex<HashMap<i32, VecDeque<PieceRequest>>>>,
    download_dir: String,
}

impl PieceManager {
    pub fn new(
        piece_repository: Arc<dyn PieceRepository>,
        torrent_repository: Arc<dyn TorrentRepository>,
        download_dir: String,
    ) -> Self {
        Self {
            piece_repository,
            torrent_repository,
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            download_dir,
        }
    }

    /// Request a piece with specific priority
    pub async fn request_piece(&self, torrent_id: i32, piece_index: usize, priority: PiecePriority, requester: String) -> Result<(), DomainError> {
        let request = PieceRequest {
            piece_index,
            priority,
            requester,
        };

        let mut requests = self.pending_requests.lock().unwrap();
        let torrent_requests = requests.entry(torrent_id).or_insert_with(VecDeque::new);
        
        // Insert in priority order
        let insert_pos = torrent_requests.iter()
            .position(|r| r.priority < priority)
            .unwrap_or(torrent_requests.len());
        
        torrent_requests.insert(insert_pos, request);
        
        Ok(())
    }

    /// Check if a piece is already downloaded
    pub async fn is_piece_available(&self, torrent_id: i32, piece_index: usize) -> Result<bool, DomainError> {
        if let Some(piece) = self.piece_repository.find_by_torrent_and_index(torrent_id, piece_index as i32).await? {
            Ok(piece.downloaded && piece.verified)
        } else {
            Ok(false)
        }
    }

    /// Read piece data from the downloaded file
    pub async fn read_piece_data(&self, torrent_id: i32, piece_index: usize) -> Result<Vec<u8>, DomainError> {
        // Check if piece is available
        if !self.is_piece_available(torrent_id, piece_index).await? {
            return Err(DomainError::NotFound(format!("Piece {} not available for torrent {}", piece_index, torrent_id)));
        }

        // Get torrent info
        let torrent = self.torrent_repository.find_by_id(torrent_id).await?
            .ok_or_else(|| DomainError::NotFound(format!("Torrent {} not found", torrent_id)))?;

        // Calculate file path
        let file_path = self.get_torrent_file_path(&torrent)?;
        
        // Calculate piece offset and size
        let piece_size = torrent.piece_length as u64;
        let piece_offset = piece_index as u64 * piece_size;
        
        // Determine actual piece size (last piece might be smaller)
        let total_pieces = torrent.piece_count as usize;
        let actual_piece_size = if piece_index == total_pieces - 1 {
            // Last piece - calculate remaining bytes
            let remaining = torrent.total_size as u64 - piece_offset;
            remaining.min(piece_size) as usize
        } else {
            piece_size as usize
        };

        // Read piece data from file
        let mut file = File::open(&file_path).await
            .map_err(|e| DomainError::IoError(format!("Failed to open file {}: {}", file_path.display(), e)))?;

        file.seek(SeekFrom::Start(piece_offset)).await
            .map_err(|e| DomainError::IoError(format!("Failed to seek to piece offset: {}", e)))?;

        let mut buffer = vec![0u8; actual_piece_size];
        let bytes_read = file.read_exact(&mut buffer).await
            .map_err(|e| DomainError::IoError(format!("Failed to read piece data: {}", e)))?;

        if bytes_read != actual_piece_size {
            return Err(DomainError::IoError(format!("Expected {} bytes, read {}", actual_piece_size, bytes_read)));
        }

        Ok(buffer)
    }

    /// Read a range of data across multiple pieces
    pub async fn read_range(&self, torrent_id: i32, start_offset: u64, length: u64) -> Result<Vec<u8>, DomainError> {
        let torrent = self.torrent_repository.find_by_id(torrent_id).await?
            .ok_or_else(|| DomainError::NotFound(format!("Torrent {} not found", torrent_id)))?;

        let piece_size = torrent.piece_length as u64;
        let start_piece = (start_offset / piece_size) as usize;
        let end_offset = start_offset + length - 1;
        let end_piece = (end_offset / piece_size) as usize;

        let mut result = Vec::new();
        let mut current_offset = start_offset;
        let mut remaining_length = length;

        for piece_index in start_piece..=end_piece {
            // Ensure piece is available (prioritize if not)
            if !self.is_piece_available(torrent_id, piece_index).await? {
                self.request_piece(torrent_id, piece_index, PiecePriority::Urgent, "streaming".to_string()).await?;
                
                // In a real implementation, you would wait for the piece to be downloaded
                // For now, return an error indicating the piece is not ready
                return Err(DomainError::NotFound(format!("Piece {} not ready for streaming", piece_index)));
            }

            let piece_data = self.read_piece_data(torrent_id, piece_index).await?;
            
            // Calculate range within this piece
            let piece_start_offset = piece_index as u64 * piece_size;
            let piece_end_offset = piece_start_offset + piece_data.len() as u64 - 1;

            let read_start = if current_offset > piece_start_offset {
                (current_offset - piece_start_offset) as usize
            } else {
                0
            };

            let read_end = if current_offset + remaining_length - 1 < piece_end_offset {
                ((current_offset + remaining_length - 1) - piece_start_offset) as usize + 1
            } else {
                piece_data.len()
            };

            if read_start < piece_data.len() && read_end > read_start {
                let chunk = &piece_data[read_start..read_end];
                result.extend_from_slice(chunk);
                
                let chunk_length = chunk.len() as u64;
                current_offset += chunk_length;
                remaining_length = remaining_length.saturating_sub(chunk_length);
            }

            if remaining_length == 0 {
                break;
            }
        }

        Ok(result)
    }

    fn get_torrent_file_path(&self, torrent: &Torrent) -> Result<PathBuf, DomainError> {
        let download_path = PathBuf::from(&self.download_dir);
        let file_name = torrent.file_path.as_ref()
            .map(|p| PathBuf::from(p).file_name().unwrap().to_string_lossy().to_string())
            .unwrap_or_else(|| torrent.name.clone());
        
        Ok(download_path.join(file_name))
    }

    /// Get next piece that should be downloaded for a torrent
    pub fn get_next_piece_request(&self, torrent_id: i32) -> Option<PieceRequest> {
        let mut requests = self.pending_requests.lock().unwrap();
        if let Some(torrent_requests) = requests.get_mut(&torrent_id) {
            torrent_requests.pop_front()
        } else {
            None
        }
    }

    /// Mark a piece as completed
    pub async fn mark_piece_completed(&self, torrent_id: i32, piece_index: usize, data: Vec<u8>) -> Result<(), DomainError> {
        // Verify piece hash
        let piece = self.piece_repository.find_by_torrent_and_index(torrent_id, piece_index as i32).await?
            .ok_or_else(|| DomainError::NotFound(format!("Piece {} not found", piece_index)))?;

        let computed_hash = sha1::Sha1::digest(&data);
        let expected_hash = hex::decode(&piece.hash)
            .map_err(|e| DomainError::ParseError(format!("Invalid piece hash: {}", e)))?;

        if computed_hash.as_slice() != expected_hash.as_slice() {
            return Err(DomainError::ValidationError("Piece hash verification failed".to_string()));
        }

        // Update piece status - we'll need to modify the piece and save it
        let mut updated_piece = piece.clone();
        updated_piece.downloaded = true;
        updated_piece.verified = true;
        self.piece_repository.update(&updated_piece).await?;

        Ok(())
    }
}
