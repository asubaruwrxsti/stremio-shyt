use crate::entities::{Peer, Torrent};
use crate::errors::DomainError;
use crate::repositories::{PieceRepository, PeerRepository, TorrentRepository};
use crate::services::piece_manager::PieceManager;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct PieceDownloader {
    piece_repository: Arc<dyn PieceRepository>,
    peer_repository: Arc<dyn PeerRepository>,
    torrent_repository: Arc<dyn TorrentRepository>,
    piece_manager: Arc<PieceManager>,
    download_dir: String,
}

impl PieceDownloader {
    pub fn new(
        piece_repository: Arc<dyn PieceRepository>,
        peer_repository: Arc<dyn PeerRepository>,
        torrent_repository: Arc<dyn TorrentRepository>,
        piece_manager: Arc<PieceManager>,
        download_dir: String,
    ) -> Self {
        Self {
            piece_repository,
            peer_repository,
            torrent_repository,
            piece_manager,
            download_dir,
        }
    }

    /// Start downloading pieces for a torrent
    pub async fn start_downloading(&self, torrent_id: i32) -> Result<(), DomainError> {
        let torrent = self.torrent_repository.find_by_id(torrent_id).await?
            .ok_or_else(|| DomainError::NotFound(format!("Torrent {} not found", torrent_id)))?;

        // Get available peers
        let peers = self.peer_repository.find_by_torrent_id(torrent_id).await?;
        
        if peers.is_empty() {
            return Err(DomainError::ValidationError("No peers available for download".to_string()));
        }

        // Start download tasks
        let mut download_tasks = Vec::new();
        
        for peer in peers.into_iter().take(5) { // Use up to 5 peers concurrently
            let downloader = self.clone();
            let torrent_clone = torrent.clone();
            
            let task = tokio::spawn(async move {
                downloader.download_from_peer(torrent_clone, peer).await
            });
            
            download_tasks.push(task);
        }

        // Wait for at least one task to complete successfully
        let results = futures::future::join_all(download_tasks).await;
        
        for result in results {
            match result {
                Ok(Ok(_)) => return Ok(()),
                Ok(Err(e)) => eprintln!("Download task failed: {}", e),
                Err(e) => eprintln!("Download task panicked: {}", e),
            }
        }

        Err(DomainError::NetworkError("All download tasks failed".to_string()))
    }

    async fn download_from_peer(&self, torrent: Torrent, peer: Peer) -> Result<(), DomainError> {
        // Connect to peer
        let peer_addr = format!("{}:{}", peer.ip, peer.port);
        let mut stream = TcpStream::connect(&peer_addr).await
            .map_err(|e| DomainError::NetworkError(format!("Failed to connect to peer {}: {}", peer_addr, e)))?;

        // Perform BitTorrent handshake
        self.perform_handshake(&mut stream, &torrent).await?;

        // Download pieces in priority order
        loop {
            // Get next piece to download
            let piece_request = self.piece_manager.get_next_piece_request(torrent.id.unwrap());
            
            if let Some(request) = piece_request {
                match self.download_piece(&mut stream, &torrent, request.piece_index).await {
                    Ok(piece_data) => {
                        // Verify and store piece
                        self.piece_manager.mark_piece_completed(
                            torrent.id.unwrap(), 
                            request.piece_index, 
                            piece_data
                        ).await?;
                    }
                    Err(e) => {
                        eprintln!("Failed to download piece {}: {}", request.piece_index, e);
                        // Continue with next piece
                        continue;
                    }
                }
            } else {
                // No more pieces to download
                break;
            }
        }

        Ok(())
    }

    async fn perform_handshake(&self, stream: &mut TcpStream, torrent: &Torrent) -> Result<(), DomainError> {
        // BitTorrent handshake format:
        // - 1 byte: protocol name length (19)
        // - 19 bytes: protocol name ("BitTorrent protocol")
        // - 8 bytes: reserved flags
        // - 20 bytes: info hash
        // - 20 bytes: peer ID

        let mut handshake = Vec::new();
        handshake.push(19u8); // Protocol name length
        handshake.extend_from_slice(b"BitTorrent protocol"); // Protocol name
        handshake.extend_from_slice(&[0u8; 8]); // Reserved flags
        
        // Convert hex info hash to bytes
        let info_hash_bytes = hex::decode(&torrent.info_hash)
            .map_err(|e| DomainError::ParseError(format!("Invalid info hash: {}", e)))?;
        handshake.extend_from_slice(&info_hash_bytes);
        
        // Generate peer ID (20 bytes)
        let peer_id = format!("-RS0001-{}", rand::random::<u64>());
        let peer_id_bytes = peer_id.as_bytes();
        let mut peer_id_20 = [0u8; 20];
        peer_id_20[..peer_id_bytes.len().min(20)].copy_from_slice(&peer_id_bytes[..peer_id_bytes.len().min(20)]);
        handshake.extend_from_slice(&peer_id_20);

        // Send handshake
        stream.write_all(&handshake).await
            .map_err(|e| DomainError::NetworkError(format!("Failed to send handshake: {}", e)))?;

        // Read handshake response
        let mut response = vec![0u8; 68]; // 1 + 19 + 8 + 20 + 20
        stream.read_exact(&mut response).await
            .map_err(|e| DomainError::NetworkError(format!("Failed to read handshake response: {}", e)))?;

        // Verify handshake response
        if response[0] != 19 || &response[1..20] != b"BitTorrent protocol" {
            return Err(DomainError::ValidationError("Invalid handshake response".to_string()));
        }

        // Verify info hash
        let received_info_hash = &response[28..48];
        if received_info_hash != info_hash_bytes {
            return Err(DomainError::ValidationError("Info hash mismatch in handshake".to_string()));
        }

        Ok(())
    }

    async fn download_piece(&self, stream: &mut TcpStream, torrent: &Torrent, piece_index: usize) -> Result<Vec<u8>, DomainError> {
        let piece_length = if piece_index == (torrent.piece_count - 1) as usize {
            // Last piece might be smaller
            let total_size = torrent.total_size as u64;
            let full_pieces_size = (torrent.piece_count - 1) as u64 * torrent.piece_length as u64;
            (total_size - full_pieces_size) as usize
        } else {
            torrent.piece_length as usize
        };

        let mut piece_data = Vec::new();
        let block_size = 16384; // 16KB blocks
        let num_blocks = (piece_length + block_size - 1) / block_size;

        for block_index in 0..num_blocks {
            let block_offset = block_index * block_size;
            let block_length = std::cmp::min(block_size, piece_length - block_offset);

            // Send request message
            let request_msg = self.create_request_message(piece_index as u32, block_offset as u32, block_length as u32);
            stream.write_all(&request_msg).await
                .map_err(|e| DomainError::NetworkError(format!("Failed to send request: {}", e)))?;

            // Read piece message response
            let block_data = self.read_piece_message(stream, block_length).await?;
            piece_data.extend_from_slice(&block_data);
        }

        // Verify piece data length
        if piece_data.len() != piece_length {
            return Err(DomainError::ValidationError(format!(
                "Piece data length mismatch: expected {}, got {}", 
                piece_length, piece_data.len()
            )));
        }

        Ok(piece_data)
    }

    fn create_request_message(&self, piece_index: u32, block_offset: u32, block_length: u32) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(&13u32.to_be_bytes()); // Message length
        message.push(6u8); // Request message ID
        message.extend_from_slice(&piece_index.to_be_bytes());
        message.extend_from_slice(&block_offset.to_be_bytes());
        message.extend_from_slice(&block_length.to_be_bytes());
        message
    }

    async fn read_piece_message(&self, stream: &mut TcpStream, expected_length: usize) -> Result<Vec<u8>, DomainError> {
        // Read message length
        let mut length_bytes = [0u8; 4];
        stream.read_exact(&mut length_bytes).await
            .map_err(|e| DomainError::NetworkError(format!("Failed to read message length: {}", e)))?;
        
        let message_length = u32::from_be_bytes(length_bytes) as usize;
        
        // Read message ID
        let mut message_id = [0u8; 1];
        stream.read_exact(&mut message_id).await
            .map_err(|e| DomainError::NetworkError(format!("Failed to read message ID: {}", e)))?;
        
        if message_id[0] != 7 { // Piece message ID
            return Err(DomainError::ValidationError(format!("Expected piece message, got ID {}", message_id[0])));
        }

        // Read piece index and block offset
        let mut piece_info = [0u8; 8];
        stream.read_exact(&mut piece_info).await
            .map_err(|e| DomainError::NetworkError(format!("Failed to read piece info: {}", e)))?;

        // Read block data
        let block_length = message_length - 9; // Total length - ID - piece index - block offset
        let mut block_data = vec![0u8; block_length];
        stream.read_exact(&mut block_data).await
            .map_err(|e| DomainError::NetworkError(format!("Failed to read block data: {}", e)))?;

        if block_data.len() != expected_length {
            return Err(DomainError::ValidationError(format!(
                "Block data length mismatch: expected {}, got {}",
                expected_length, block_data.len()
            )));
        }

        Ok(block_data)
    }
}

impl Clone for PieceDownloader {
    fn clone(&self) -> Self {
        Self {
            piece_repository: Arc::clone(&self.piece_repository),
            peer_repository: Arc::clone(&self.peer_repository),
            torrent_repository: Arc::clone(&self.torrent_repository),
            piece_manager: Arc::clone(&self.piece_manager),
            download_dir: self.download_dir.clone(),
        }
    }
}
