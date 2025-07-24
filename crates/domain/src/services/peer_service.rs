use crate::entities::{Peer, PeerStatus, Piece};
use crate::errors::DomainError;
use crate::repositories::{PeerRepository, TorrentRepository};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Service for managing peer connections and piece requests
/// Handles: initiate peer connections → request pieces
pub struct PeerService {
    peer_repository: Arc<dyn PeerRepository>,
    torrent_repository: Arc<dyn TorrentRepository>,
}

impl PeerService {
    pub fn new(
        peer_repository: Arc<dyn PeerRepository>,
        torrent_repository: Arc<dyn TorrentRepository>,
    ) -> Self {
        Self { 
            peer_repository,
            torrent_repository,
        }
    }

    /// Connect to peers for a torrent
    /// This implements: initiate peer connections
    pub async fn connect_to_peers(&self, torrent_id: i32) -> Result<Vec<Peer>, DomainError> {
        // Get torrent info_hash first
        let torrent = self.torrent_repository.find_by_id(torrent_id).await?
            .ok_or_else(|| DomainError::TorrentNotFound(torrent_id))?;
        
        let peers = self.peer_repository.find_by_torrent_id(torrent_id).await?;
        let mut connected_peers = Vec::new();

        for mut peer in peers {
            if peer.status == PeerStatus::Disconnected {
                match self.connect_to_peer(&peer, &torrent.info_hash).await {
                    Ok(()) => {
                        peer.set_status(PeerStatus::Connected);
                        let updated_peer = self.peer_repository.update(&peer).await?;
                        connected_peers.push(updated_peer);
                    }
                    Err(e) => {
                        peer.set_status(PeerStatus::Disconnected);
                        self.peer_repository.update(&peer).await?;
                        eprintln!("Failed to connect to peer {}: {}", peer.socket_addr(), e);
                    }
                }
            }
        }

        Ok(connected_peers)
    }

    /// Connect to a peer and perform BitTorrent handshake
    pub async fn connect_to_peer(&self, peer: &Peer, info_hash: &str) -> Result<(), DomainError> {
        let socket_addr = format!("{}:{}", peer.ip, peer.port);
        
        println!("🤝 Attempting to connect to peer: {}", socket_addr);

        // Create TCP connection with timeout
        let stream = tokio::time::timeout(
            Duration::from_secs(10),
            TcpStream::connect(&socket_addr)
        ).await
            .map_err(|_| DomainError::PeerConnectionError(format!("Connection timeout to {}", socket_addr)))?
            .map_err(|e| DomainError::PeerConnectionError(format!("Failed to connect to {}: {}", socket_addr, e)))?;

        let mut stream = stream;

        // 1. Send handshake message following BitTorrent protocol
        // Format: <pstrlen><pstr><reserved><info_hash><peer_id>
        // pstrlen = 19, pstr = "BitTorrent protocol"
        let protocol_name = b"BitTorrent protocol";
        let mut handshake = Vec::with_capacity(68);
        
        handshake.push(19u8); // pstrlen
        handshake.extend_from_slice(protocol_name); // pstr
        handshake.extend_from_slice(&[0u8; 8]); // reserved bytes
        
        // Convert info_hash from hex string to bytes
        let info_hash_bytes = hex::decode(info_hash)
            .map_err(|e| DomainError::PeerConnectionError(format!("Invalid info hash: {}", e)))?;
        
        if info_hash_bytes.len() != 20 {
            return Err(DomainError::PeerConnectionError("Info hash must be 20 bytes".to_string()));
        }
        
        handshake.extend_from_slice(&info_hash_bytes);
        
        // Generate our peer_id (20 bytes) - format: -ST0001-xxxxxxxxxxxx
        let mut peer_id = [0u8; 20];
        peer_id[..8].copy_from_slice(b"-ST0001-");
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u32;
        peer_id[8..12].copy_from_slice(&timestamp.to_be_bytes());
        peer_id[12..16].copy_from_slice(&rand::random::<u32>().to_be_bytes());
        peer_id[16..20].copy_from_slice(&rand::random::<u32>().to_be_bytes());
        handshake.extend_from_slice(&peer_id);

        // Send handshake
        if let Err(e) = stream.write_all(&handshake).await {
            return Err(DomainError::PeerConnectionError(format!("Failed to send handshake: {}", e)));
        }

        // 2. Read handshake response
        let mut response = vec![0u8; 68];
        if let Err(e) = stream.read_exact(&mut response).await {
            return Err(DomainError::PeerConnectionError(format!("Failed to read handshake response: {}", e)));
        }

        // Validate handshake response
        if response[0] != 19 || &response[1..20] != b"BitTorrent protocol" {
            return Err(DomainError::PeerConnectionError("Invalid handshake response".to_string()));
        }

        // 3. Send bitfield message (indicating we have no pieces yet)
        let bitfield_msg = [0u8, 0u8, 0u8, 1u8, 5u8]; // length=1, id=5 (bitfield), empty bitfield
        if let Err(e) = stream.write_all(&bitfield_msg).await {
            return Err(DomainError::PeerConnectionError(format!("Failed to send bitfield: {}", e)));
        }

        // 4. Send interested message
        let interested_msg = [0u8, 0u8, 0u8, 1u8, 2u8]; // length=1, id=2 (interested)
        if let Err(e) = stream.write_all(&interested_msg).await {
            return Err(DomainError::PeerConnectionError(format!("Failed to send interested: {}", e)));
        }

        println!("✅ Successfully completed BitTorrent handshake with {}", socket_addr);
        
        // TODO: In production, you'd keep this connection alive and handle incoming messages
        // For now, we close it after handshake
        drop(stream);

        Ok(())
    }

    /// Request a piece from available peers
    pub async fn request_piece(&self, torrent_id: i32, piece: &Piece) -> Result<(), DomainError> {
        use tokio::net::TcpStream;

        let connected_peers = self.peer_repository.find_connected(torrent_id).await?;

        if connected_peers.is_empty() {
            return Err(DomainError::PeerConnectionError(
                "No connected peers available".to_string(),
            ));
        }

        // Select best peer based on connection quality, speed, availability
        let selected_peer = self.select_best_peer_for_piece(&connected_peers, piece).await?;

        println!(
            "📬 Requesting piece {} from peer {}:{}",
            piece.piece_index, selected_peer.ip, selected_peer.port
        );

        // Connect to peer for piece request
        let socket_addr = format!("{}:{}", selected_peer.ip, selected_peer.port);
        let mut stream = TcpStream::connect(&socket_addr).await
            .map_err(|e| DomainError::PeerConnectionError(format!("Failed to connect for piece request: {}", e)))?;

        // Send piece request using BitTorrent REQUEST message
        // Format: <len=0013><id=6><index><begin><length>
        let piece_length = 32768u32; // 32KB standard piece size
        let num_blocks = (piece_length + 16383) / 16384; // 16KB blocks

        for block in 0..num_blocks {
            let begin = block * 16384;
            let length = if block == num_blocks - 1 {
                piece_length - begin // Last block might be smaller
            } else {
                16384
            };

            // Build REQUEST message
            let mut request_msg = Vec::with_capacity(17);
            request_msg.extend_from_slice(&13u32.to_be_bytes()); // length = 13
            request_msg.push(6u8); // message id = 6 (REQUEST)
            request_msg.extend_from_slice(&(piece.piece_index as u32).to_be_bytes()); // piece index
            request_msg.extend_from_slice(&begin.to_be_bytes()); // begin offset
            request_msg.extend_from_slice(&length.to_be_bytes()); // block length

            // Send request
            if let Err(e) = stream.write_all(&request_msg).await {
                return Err(DomainError::PeerConnectionError(format!("Failed to send piece request: {}", e)));
            }

            println!("� Sent request for piece {} block {} (offset: {}, length: {})", 
                piece.piece_index, block, begin, length);
        }

        // In production, you'd:
        // 1. Wait for PIECE messages in response
        // 2. Reassemble the complete piece from blocks
        // 3. Verify the piece hash
        // 4. Mark piece as downloaded
        // For now, we'll simulate this in the download service

        println!("✅ Piece request sent successfully");
        Ok(())
    }

    /// Select the best peer for requesting a specific piece
    async fn select_best_peer_for_piece<'a>(&self, peers: &'a [Peer], _piece: &Piece) -> Result<&'a Peer, DomainError> {
        // In production, this would consider:
        // - Peer's bitfield (which pieces they have)
        // - Connection speed/latency
        // - Current request queue length
        // - Peer reputation/reliability
        
        // For now, select first available peer
        peers.first()
            .ok_or_else(|| DomainError::PeerConnectionError("No peers available".to_string()))
    }

    /// Get connected peers for a torrent
    pub async fn get_connected_peers(&self, torrent_id: i32) -> Result<Vec<Peer>, DomainError> {
        self.peer_repository.find_connected(torrent_id).await
    }

    /// Disconnect from a peer
    pub async fn disconnect_peer(&self, peer_id: i32) -> Result<(), DomainError> {
        // For now, we don't have find_by_id, so this is a simplified implementation
        // In practice, you'd track the peer connection and update its status

        println!("🔌 Requested disconnect from peer {}", peer_id);

        // Close actual network connection would be handled here
        // For example: close TCP socket, cleanup peer state, etc.

        Ok(())
    }

    /// Clean up old/stale peer connections
    pub async fn cleanup_old_peers(&self, torrent_id: i32) -> Result<(), DomainError> {
        // Remove peers not seen in last 24 hours
        self.peer_repository.delete_old(torrent_id, 24).await
    }

    /// Add new peers from tracker response
    pub async fn add_peers(&self, peers: Vec<Peer>) -> Result<Vec<Peer>, DomainError> {
        self.peer_repository.save_batch(&peers).await
    }
}
