use crate::entities::{Peer, Tracker};
use crate::errors::DomainError;
use crate::repositories::{PeerRepository, TorrentRepository, TrackerRepository};
use std::sync::Arc;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// Bencoded value types for parsing tracker responses
#[derive(Debug, Clone)]
enum BencodedValue {
    String(Vec<u8>),
    Int(i64),
    List(Vec<BencodedValue>),
    Dict(HashMap<Vec<u8>, BencodedValue>),
}

/// Simple bencoding parser
struct BencodedParser<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> BencodedParser<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn parse(&mut self) -> Result<BencodedValue, String> {
        if self.position >= self.data.len() {
            return Err("Unexpected end of data".to_string());
        }

        match self.data[self.position] {
            b'i' => self.parse_int(),
            b'l' => self.parse_list(),
            b'd' => self.parse_dict(),
            b'0'..=b'9' => self.parse_string(),
            _ => Err(format!("Invalid bencoded data at position {}", self.position)),
        }
    }

    fn parse_int(&mut self) -> Result<BencodedValue, String> {
        self.position += 1; // skip 'i'
        let end = self.find_byte(b'e')?;
        let int_str = std::str::from_utf8(&self.data[self.position..end])
            .map_err(|_| "Invalid integer encoding")?;
        let value = int_str.parse::<i64>()
            .map_err(|_| "Invalid integer value")?;
        self.position = end + 1;
        Ok(BencodedValue::Int(value))
    }

    fn parse_string(&mut self) -> Result<BencodedValue, String> {
        let colon_pos = self.find_byte(b':')?;
        let len_str = std::str::from_utf8(&self.data[self.position..colon_pos])
            .map_err(|_| "Invalid string length encoding")?;
        let len = len_str.parse::<usize>()
            .map_err(|_| "Invalid string length")?;
        
        self.position = colon_pos + 1;
        if self.position + len > self.data.len() {
            return Err("String length exceeds data bounds".to_string());
        }
        
        let value = self.data[self.position..self.position + len].to_vec();
        self.position += len;
        Ok(BencodedValue::String(value))
    }

    fn parse_list(&mut self) -> Result<BencodedValue, String> {
        self.position += 1; // skip 'l'
        let mut list = Vec::new();
        
        while self.position < self.data.len() && self.data[self.position] != b'e' {
            list.push(self.parse()?);
        }
        
        if self.position >= self.data.len() {
            return Err("Unterminated list".to_string());
        }
        
        self.position += 1; // skip 'e'
        Ok(BencodedValue::List(list))
    }

    fn parse_dict(&mut self) -> Result<BencodedValue, String> {
        self.position += 1; // skip 'd'
        let mut dict = HashMap::new();
        
        while self.position < self.data.len() && self.data[self.position] != b'e' {
            let key = match self.parse()? {
                BencodedValue::String(k) => k,
                _ => return Err("Dictionary keys must be strings".to_string()),
            };
            let value = self.parse()?;
            dict.insert(key, value);
        }
        
        if self.position >= self.data.len() {
            return Err("Unterminated dictionary".to_string());
        }
        
        self.position += 1; // skip 'e'
        Ok(BencodedValue::Dict(dict))
    }

    fn find_byte(&self, byte: u8) -> Result<usize, String> {
        self.data[self.position..]
            .iter()
            .position(|&b| b == byte)
            .map(|pos| self.position + pos)
            .ok_or_else(|| format!("Byte {:?} not found", byte as char))
    }
}

/// Service for managing tracker communications
/// Handles: connect to tracker(s) to get peers
pub struct TrackerService {
    tracker_repository: Arc<dyn TrackerRepository>,
    peer_repository: Arc<dyn PeerRepository>,
    torrent_repository: Arc<dyn TorrentRepository>,
}

impl TrackerService {
    pub fn new(
        tracker_repository: Arc<dyn TrackerRepository>,
        peer_repository: Arc<dyn PeerRepository>,
        torrent_repository: Arc<dyn TorrentRepository>,
    ) -> Self {
        Self {
            tracker_repository,
            peer_repository,
            torrent_repository,
        }
    }

    /// Announce to all trackers for a torrent
    /// This implements: connect to tracker(s) to get peers
    pub async fn announce_to_trackers(
        &self,
        torrent_id: i32,
        info_hash: &str,
    ) -> Result<Vec<Peer>, DomainError> {
        let trackers: Vec<Tracker> = self.tracker_repository.find_active(torrent_id).await?;
        let mut all_peers: Vec<Peer> = Vec::new();

        for mut tracker in trackers {
            match self.announce_to_tracker(&mut tracker, info_hash).await {
                Ok(peers) => {
                    // Save the peers to the repository
                    let saved_peers: Vec<Peer> = self.peer_repository.save_batch(&peers).await?;
                    all_peers.extend(saved_peers);

                    // Update tracker status
                    tracker.mark_announce_success(1800); // 30 minutes default interval
                    self.tracker_repository.update(&tracker).await?;
                }
                Err(e) => {
                    tracker.mark_announce_failed();
                    self.tracker_repository.update(&tracker).await?;
                    eprintln!("Tracker announce failed for {}: {}", tracker.url, e);
                }
            }
        }

        Ok(all_peers)
    }

    /// Announce to a specific tracker
    async fn announce_to_tracker(
        &self,
        tracker: &Tracker,
        info_hash: &str,
    ) -> Result<Vec<Peer>, DomainError> {
        if tracker.url.starts_with("http://") || tracker.url.starts_with("https://") {
            self.http_tracker_announce(tracker, info_hash).await
        } else if tracker.url.starts_with("udp://") {
            // Implement basic UDP tracker protocol
            self.udp_tracker_announce(tracker, info_hash).await
        } else {
            Err(DomainError::TrackerError(format!(
                "Unsupported tracker protocol: {}",
                tracker.url
            )))
        }
    }

    /// HTTP tracker announce implementation
    async fn http_tracker_announce(
        &self,
        tracker: &Tracker,
        info_hash: &str,
    ) -> Result<Vec<Peer>, DomainError> {
        use reqwest;
        use url::Url;

        // Create HTTP client
        let client = reqwest::Client::new();

        // Build announce URL with parameters
        let mut url = Url::parse(&tracker.url)
            .map_err(|e| DomainError::TrackerError(format!("Invalid tracker URL: {}", e)))?;

        // Convert hex info_hash to URL-encoded bytes
        let info_hash_bytes = hex::decode(info_hash)
            .map_err(|e| DomainError::TrackerError(format!("Invalid info_hash: {}", e)))?;
        let info_hash_encoded =
            percent_encoding::percent_encode(&info_hash_bytes, percent_encoding::NON_ALPHANUMERIC);

        // Generate peer_id (20 bytes)
        let peer_id = format!("-RS0001-{:012}", rand::random::<u64>());
        let peer_id_encoded = percent_encoding::percent_encode(
            peer_id.as_bytes(),
            percent_encoding::NON_ALPHANUMERIC,
        );

        // Calculate actual remaining bytes
        let remaining_bytes = match self.torrent_repository.find_by_id(tracker.torrent_id).await {
            Ok(Some(torrent)) => {
                let downloaded_pieces = match self
                    .peer_repository
                    .find_by_torrent_id(tracker.torrent_id)
                    .await
                {
                    Ok(peers) => peers.len() as i64, // Simplified calculation
                    Err(_) => 0,
                };
                (torrent.total_size - (downloaded_pieces * torrent.piece_length as i64)).max(0)
            }
            _ => 1000000, // Default fallback
        };

        url.query_pairs_mut()
            .append_pair("info_hash", &info_hash_encoded.to_string())
            .append_pair("peer_id", &peer_id_encoded.to_string())
            .append_pair("port", "6881")
            .append_pair("uploaded", "0")
            .append_pair("downloaded", "0")
            .append_pair("left", &remaining_bytes.to_string())
            .append_pair("compact", "1")
            .append_pair("event", "started");

        println!("Announcing to tracker: {}", url);

        // Make HTTP request
        let response = client
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| DomainError::NetworkError(format!("Tracker request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(DomainError::TrackerError(format!(
                "Tracker returned status: {}",
                response.status()
            )));
        }

        let response_bytes = response.bytes().await.map_err(|e| {
            DomainError::NetworkError(format!("Failed to read tracker response: {}", e))
        })?;

        // Parse bencoded response
        self.parse_tracker_response(tracker.torrent_id, &response_bytes)
            .await
    }

    /// UDP tracker announce implementation following BEP-15
    async fn udp_tracker_announce(
        &self,
        tracker: &Tracker,
        info_hash: &str,
    ) -> Result<Vec<Peer>, DomainError> {
        use tokio::net::UdpSocket;
        use std::net::SocketAddr;
        
        println!("📡 UDP tracker announce to: {}", tracker.url);

        // Parse the UDP tracker URL
        let url = tracker.url.strip_prefix("udp://")
            .ok_or_else(|| DomainError::TrackerError("Invalid UDP tracker URL".to_string()))?;
        
        let socket_addr: SocketAddr = url.parse()
            .map_err(|e| DomainError::TrackerError(format!("Invalid tracker address: {}", e)))?;

        // Create UDP socket
        let socket = UdpSocket::bind("0.0.0.0:0").await
            .map_err(|e| DomainError::NetworkError(format!("Failed to bind UDP socket: {}", e)))?;

        // Step 1: Connect request
        let connection_id = self.udp_connect(&socket, socket_addr).await?;
        
        // Step 2: Announce request
        let peers = self.udp_announce(&socket, socket_addr, connection_id, info_hash, tracker.torrent_id).await?;

        println!("✅ UDP tracker returned {} peers", peers.len());
        Ok(peers)
    }

    /// UDP tracker connect request (BEP-15)
    async fn udp_connect(
        &self,
        socket: &UdpSocket,
        tracker_addr: SocketAddr,
    ) -> Result<u64, DomainError> {
        use rand::Rng;
        
        // Connect request packet:
        // Offset  Size            Name            Value
        // 0       64-bit integer  protocol_id     0x41727101980 (magic constant)
        // 8       32-bit integer  action          0 (connect)
        // 12      32-bit integer  transaction_id  Random
        
        let mut rng = rand::thread_rng();
        let transaction_id: u32 = rng.gen();
        
        let mut connect_request = Vec::with_capacity(16);
        connect_request.extend_from_slice(&0x41727101980u64.to_be_bytes()); // protocol_id
        connect_request.extend_from_slice(&0u32.to_be_bytes()); // action (connect)
        connect_request.extend_from_slice(&transaction_id.to_be_bytes()); // transaction_id

        // Send connect request
        socket.send_to(&connect_request, tracker_addr).await
            .map_err(|e| DomainError::NetworkError(format!("Failed to send UDP connect: {}", e)))?;

        // Receive connect response
        let mut response_buf = [0u8; 16];
        let (size, _) = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            socket.recv_from(&mut response_buf)
        ).await
            .map_err(|_| DomainError::TrackerError("UDP connect timeout".to_string()))?
            .map_err(|e| DomainError::NetworkError(format!("Failed to receive UDP connect response: {}", e)))?;

        if size < 16 {
            return Err(DomainError::TrackerError("Invalid UDP connect response size".to_string()));
        }

        // Parse connect response:
        // Offset  Size            Name            Value
        // 0       32-bit integer  action          0 (connect)
        // 4       32-bit integer  transaction_id  Must match request
        // 8       64-bit integer  connection_id   For future requests
        
        let response_action = u32::from_be_bytes([response_buf[0], response_buf[1], response_buf[2], response_buf[3]]);
        let response_transaction_id = u32::from_be_bytes([response_buf[4], response_buf[5], response_buf[6], response_buf[7]]);
        let connection_id = u64::from_be_bytes([
            response_buf[8], response_buf[9], response_buf[10], response_buf[11],
            response_buf[12], response_buf[13], response_buf[14], response_buf[15]
        ]);

        if response_action != 0 {
            return Err(DomainError::TrackerError("Invalid UDP connect response action".to_string()));
        }

        if response_transaction_id != transaction_id {
            return Err(DomainError::TrackerError("UDP connect transaction ID mismatch".to_string()));
        }

        Ok(connection_id)
    }

    /// UDP tracker announce request (BEP-15)
    async fn udp_announce(
        &self,
        socket: &UdpSocket,
        tracker_addr: SocketAddr,
        connection_id: u64,
        info_hash: &str,
        torrent_id: i32,
    ) -> Result<Vec<Peer>, DomainError> {
        use rand::Rng;
        
        let mut rng = rand::thread_rng();
        let transaction_id: u32 = rng.gen();
        
        // Convert info_hash from hex string to bytes
        let info_hash_bytes = hex::decode(info_hash)
            .map_err(|e| DomainError::TrackerError(format!("Invalid info hash: {}", e)))?;
        
        if info_hash_bytes.len() != 20 {
            return Err(DomainError::TrackerError("Info hash must be 20 bytes".to_string()));
        }

        // Generate random peer_id (20 bytes)
        let peer_id: [u8; 20] = rng.gen();

        // Announce request packet:
        // Offset  Size            Name            Value
        // 0       64-bit integer  connection_id   From connect response
        // 8       32-bit integer  action          1 (announce)
        // 12      32-bit integer  transaction_id  Random
        // 16      20-byte string  info_hash       Torrent info hash
        // 36      20-byte string  peer_id         Client peer ID
        // 56      64-bit integer  downloaded      Bytes downloaded
        // 64      64-bit integer  left            Bytes left to download
        // 72      64-bit integer  uploaded        Bytes uploaded
        // 80      32-bit integer  event           0=none, 1=completed, 2=started, 3=stopped
        // 84      32-bit integer  IP address      0 (use sender IP)
        // 88      32-bit integer  key             Random key
        // 92      32-bit integer  num_want        Number of peers wanted (-1 = default)
        // 96      16-bit integer  port            Client port
        
        let mut announce_request = Vec::with_capacity(98);
        announce_request.extend_from_slice(&connection_id.to_be_bytes()); // connection_id
        announce_request.extend_from_slice(&1u32.to_be_bytes()); // action (announce)
        announce_request.extend_from_slice(&transaction_id.to_be_bytes()); // transaction_id
        announce_request.extend_from_slice(&info_hash_bytes); // info_hash (20 bytes)
        announce_request.extend_from_slice(&peer_id); // peer_id (20 bytes)
        announce_request.extend_from_slice(&0u64.to_be_bytes()); // downloaded
        announce_request.extend_from_slice(&0u64.to_be_bytes()); // left (0 = seeding)
        announce_request.extend_from_slice(&0u64.to_be_bytes()); // uploaded
        announce_request.extend_from_slice(&2u32.to_be_bytes()); // event (2 = started)
        announce_request.extend_from_slice(&0u32.to_be_bytes()); // IP (0 = use sender)
        announce_request.extend_from_slice(&rng.gen::<u32>().to_be_bytes()); // key
        announce_request.extend_from_slice(&(-1i32 as u32).to_be_bytes()); // num_want (-1 = default)
        announce_request.extend_from_slice(&6881u16.to_be_bytes()); // port

        // Send announce request
        socket.send_to(&announce_request, tracker_addr).await
            .map_err(|e| DomainError::NetworkError(format!("Failed to send UDP announce: {}", e)))?;

        // Receive announce response
        let mut response_buf = [0u8; 1024]; // Large buffer for peer list
        let (size, _) = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            socket.recv_from(&mut response_buf)
        ).await
            .map_err(|_| DomainError::TrackerError("UDP announce timeout".to_string()))?
            .map_err(|e| DomainError::NetworkError(format!("Failed to receive UDP announce response: {}", e)))?;

        if size < 20 {
            return Err(DomainError::TrackerError("Invalid UDP announce response size".to_string()));
        }

        // Parse announce response:
        // Offset  Size            Name            Value
        // 0       32-bit integer  action          1 (announce)
        // 4       32-bit integer  transaction_id  Must match request
        // 8       32-bit integer  interval        Announce interval in seconds
        // 12      32-bit integer  leechers        Number of leechers
        // 16      32-bit integer  seeders         Number of seeders
        // 20      6-byte records  peers           IP (4 bytes) + port (2 bytes)
        
        let response_action = u32::from_be_bytes([response_buf[0], response_buf[1], response_buf[2], response_buf[3]]);
        let response_transaction_id = u32::from_be_bytes([response_buf[4], response_buf[5], response_buf[6], response_buf[7]]);

        if response_action != 1 {
            return Err(DomainError::TrackerError("Invalid UDP announce response action".to_string()));
        }

        if response_transaction_id != transaction_id {
            return Err(DomainError::TrackerError("UDP announce transaction ID mismatch".to_string()));
        }

        // Extract peer list (starting at offset 20)
        let peer_data = &response_buf[20..size];
        if peer_data.len() % 6 != 0 {
            return Err(DomainError::TrackerError("Invalid UDP peer data length".to_string()));
        }

        let mut peers = Vec::new();
        for chunk in peer_data.chunks(6) {
            let ip = format!("{}.{}.{}.{}", chunk[0], chunk[1], chunk[2], chunk[3]);
            let port = ((chunk[4] as u16) << 8) | (chunk[5] as u16);
            
            peers.push(Peer::new(torrent_id, ip, port));
        }

        Ok(peers)
    }

    /// Parse tracker response and extract peers
    async fn parse_tracker_response(
        &self,
        torrent_id: i32,
        response_bytes: &[u8],
    ) -> Result<Vec<Peer>, DomainError> {
        println!(
            "📡 Parsing tracker response ({} bytes)",
            response_bytes.len()
        );

        let mut parser = BencodedParser::new(response_bytes);
        
        // Parse the tracker response as a dictionary
        let response_value = parser.parse_dict()
            .map_err(|e| DomainError::TrackerError(format!("Failed to parse tracker response: {}", e)))?;

        let response_dict = match response_value {
            BencodedValue::Dict(dict) => dict,
            _ => return Err(DomainError::TrackerError("Tracker response is not a dictionary".to_string())),
        };

        // Check for failure message first
        if let Some(BencodedValue::String(failure_reason)) = response_dict.get(&b"failure reason".to_vec()) {
            return Err(DomainError::TrackerError(
                format!("Tracker error: {}", String::from_utf8_lossy(failure_reason))
            ));
        }

        // Extract peer list from the dictionary
        let peers_value = response_dict.get(&b"peers".to_vec())
            .ok_or_else(|| DomainError::TrackerError("No peers found in tracker response".to_string()))?;

        let mut extracted_peers = Vec::new();

        match peers_value {
            BencodedValue::String(compact_peers) => {
                // Compact peer format: 6 bytes per peer (4 bytes IP + 2 bytes port)
                if compact_peers.len() % 6 != 0 {
                    return Err(DomainError::TrackerError("Invalid compact peer format".to_string()));
                }

                for chunk in compact_peers.chunks(6) {
                    let ip = format!("{}.{}.{}.{}", chunk[0], chunk[1], chunk[2], chunk[3]);
                    let port = ((chunk[4] as u16) << 8) | (chunk[5] as u16);
                    
                    extracted_peers.push(Peer::new(torrent_id, ip, port));
                }
            }
            BencodedValue::List(peer_list) => {
                // Dictionary format: list of peer dictionaries
                for peer_value in peer_list {
                    if let BencodedValue::Dict(peer_dict) = peer_value {
                        let ip = peer_dict.get(&b"ip".to_vec())
                            .and_then(|v| match v {
                                BencodedValue::String(s) => Some(String::from_utf8_lossy(s).to_string()),
                                _ => None,
                            })
                            .ok_or_else(|| DomainError::TrackerError("Peer missing IP".to_string()))?;

                        let port = peer_dict.get(&b"port".to_vec())
                            .and_then(|v| match v {
                                BencodedValue::Int(i) => Some(*i as u16),
                                _ => None,
                            })
                            .ok_or_else(|| DomainError::TrackerError("Peer missing port".to_string()))?;

                        extracted_peers.push(Peer::new(torrent_id, ip, port));
                    }
                }
            }
            _ => {
                return Err(DomainError::TrackerError("Invalid peers format in tracker response".to_string()));
            }
        }

        println!("Extracted {} peers from tracker response", extracted_peers.len());
        Ok(extracted_peers)
    }

    /// Add trackers from torrent announce list
    pub async fn add_trackers(
        &self,
        torrent_id: i32,
        tracker_urls: Vec<String>,
    ) -> Result<Vec<Tracker>, DomainError> {
        let trackers: Vec<Tracker> = tracker_urls
            .into_iter()
            .map(|url| Tracker::new(torrent_id, url))
            .collect();

        self.tracker_repository.save_batch(&trackers).await
    }

    /// Get active trackers for a torrent
    pub async fn get_active_trackers(&self, torrent_id: i32) -> Result<Vec<Tracker>, DomainError> {
        self.tracker_repository.find_active(torrent_id).await
    }

    /// Perform periodic announces for all active torrents
    pub async fn perform_periodic_announces(&self) -> Result<(), DomainError> {
        // Find all active torrents
        let torrents = self.torrent_repository.find_all().await?;
        
        for torrent in torrents {
            // Skip completed or stopped torrents
            if matches!(torrent.status, crate::entities::TorrentStatus::Completed | crate::entities::TorrentStatus::Paused) {
                continue;
            }

            // Get trackers for this torrent
            let trackers = self.tracker_repository.find_active(torrent.id.unwrap_or(0)).await?;
            
            for tracker in trackers {
                println!("🔄 Periodic announce to tracker: {} for torrent: {}", tracker.url, torrent.name);
                
                // Perform announce (this will update peer lists)
                match self.announce_to_tracker(&tracker, &torrent.info_hash).await {
                    Ok(peers) => {
                        println!("✅ Periodic announce successful, got {} peers", peers.len());
                        
                        // Save the discovered peers
                        for peer in peers {
                            if let Err(e) = self.peer_repository.save(&peer).await {
                                eprintln!("Failed to save peer: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Periodic announce failed for {}: {}", tracker.url, e);
                        
                        // Mark tracker as failed (could implement retry logic here)
                        let mut failed_tracker = tracker;
                        failed_tracker.last_announce = Some(std::time::SystemTime::now());
                        if let Err(e) = self.tracker_repository.update(&failed_tracker).await {
                            eprintln!("Failed to update tracker status: {}", e);
                        }
                    }
                }
                
                // Add delay between announces to avoid overwhelming trackers
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }

        Ok(())
    }
}
