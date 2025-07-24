use domain::*;
use infrastructure::*;
use std::sync::Arc;

/// Torrent Application - orchestrates the complete flow
pub struct TorrentApp {
    pub torrent_service: TorrentService,
    pub download_service: DownloadService,
    pub tracker_service: TrackerService,
    pub peer_service: PeerService,
    pub streaming_service: StreamingServiceImpl,
}

impl TorrentApp {
    /// Creates a new TorrentApp with default configuration
    pub fn new(database_path: &str) -> Self {
        Self::new_with_config(database_path, "downloads", 64)
    }

    /// Creates a new TorrentApp with custom configuration parameters
    pub fn new_with_config(database_path: &str, download_dir: &str, buffer_size_mb: usize) -> Self {
        // Infrastructure layer - database setup
        let database = Database::new(database_path);
        let pool = database.get_pool().clone();

        // Create repository implementations
        let torrent_repository: Arc<dyn TorrentRepository> =
            Arc::new(SqliteTorrentRepository::new(pool.clone()));
        let piece_repository: Arc<dyn PieceRepository> =
            Arc::new(SqlitePieceRepository::new(pool.clone()));
        let tracker_repository: Arc<dyn TrackerRepository> =
            Arc::new(SqliteTrackerRepository::new(pool.clone()));
        let peer_repository: Arc<dyn PeerRepository> =
            Arc::new(SqlitePeerRepository::new(pool.clone()));

        // Domain services
        let torrent_service = TorrentService::new(
            torrent_repository.clone(),
            piece_repository.clone(),
            tracker_repository.clone(),
        );

        let download_service =
            DownloadService::new(piece_repository.clone(), torrent_repository.clone());

        let tracker_service = TrackerService::new(
            tracker_repository,
            peer_repository.clone(),
            torrent_repository.clone(),
        );

        let peer_service = PeerService::new(peer_repository.clone(), torrent_repository.clone());
        
        // Create piece manager
        let piece_manager = Arc::new(PieceManager::new(
            piece_repository.clone(),
            torrent_repository.clone(),
            download_dir.to_string(),
        ));

        // Create piece downloader for production downloading
        let piece_downloader = Arc::new(PieceDownloader::new(
            piece_repository.clone(),
            peer_repository.clone(),
            torrent_repository.clone(),
            piece_manager.clone(),
            download_dir.to_string(),
        ));

        // Create streaming buffer for production streaming
        let streaming_buffer = Arc::new(StreamingBuffer::new(
            piece_manager.clone(),
            piece_downloader.clone(),
            torrent_repository.clone(),
            buffer_size_mb,
        ));
        
        let streaming_service = StreamingServiceImpl::new(
            torrent_repository.clone(),
            piece_manager,
            streaming_buffer,
            download_dir.to_string(),
        );

        Self {
            torrent_service,
            download_service,
            tracker_service,
            peer_service,
            streaming_service,
        }
    }

    /// Complete torrent download flow as per your requirements
    pub async fn download_torrent(
        &self,
        torrent_file_data: Vec<u8>,
    ) -> Result<String, DomainError> {
        // Step 1: Parse .torrent file with bip_metainfo
        let torrent = self
            .torrent_service
            .parse_torrent_file(torrent_file_data)
            .await?;
        println!(
            "📄 Parsed torrent: {} ({} pieces)",
            torrent.name, torrent.piece_count
        );

        // Step 2: Get info_hash and piece layout (already extracted during parsing)
        let info_hash = torrent.info_hash.clone();

        // Step 3: Add torrent to system
        let saved_torrent = self.torrent_service.add_torrent(torrent).await?;
        let torrent_id = saved_torrent.id.unwrap();
        println!("💾 Saved torrent with ID: {}", torrent_id);

        // Step 4: Connect to tracker(s) to get peers
        let peers = self
            .tracker_service
            .announce_to_trackers(torrent_id, &info_hash)
            .await?;
        println!("📡 Found {} peers from trackers", peers.len());

        // Step 5: Initiate peer connections
        let connected_peers = self.peer_service.connect_to_peers(torrent_id).await?;
        println!("🤝 Connected to {} peers", connected_peers.len());

        // Step 6: Start downloading first N pieces
        let pieces_to_download = self
            .download_service
            .get_next_pieces_to_download(torrent_id, 10)
            .await?;
        println!(
            "⬇️  Starting download of {} pieces",
            pieces_to_download.len()
        );

        for piece in pieces_to_download {
            // Request piece from peers
            self.peer_service.request_piece(torrent_id, &piece).await?;
        }

        // Step 7: Verify SHA1 hash of each piece (handled in complete_piece)
        // Step 8: Write to local file or stream buffer (handled in complete_piece)

        // Step 9: Prepare for streaming and check readiness
        let is_ready = self
            .download_service
            .prepare_for_streaming(torrent_id)
            .await?;

        // Start the torrent download process
        self.torrent_service.start_download(torrent_id).await?;

        // Return info about streaming readiness
        let status_message = if is_ready {
            format!("Torrent ready for streaming. Use API endpoints: /api/torrents/{}/files and /api/torrents/{}/stream/{{file_index}}", torrent_id, torrent_id)
        } else {
            format!("Torrent download started. Check /api/torrents/{} for progress.", torrent_id)
        };

        Ok(status_message)
    }

    /// Handle piece completion - verifies hash and writes data
    pub async fn handle_piece_data(
        &self,
        torrent_id: i32,
        piece_index: i32,
        data: Vec<u8>,
    ) -> Result<(), DomainError> {
        // Step 7 & 8: Verify SHA1 hash and write to local file/stream
        self.download_service
            .complete_piece(torrent_id, piece_index, data)
            .await?;

        // Update overall torrent progress
        self.torrent_service.update_progress(torrent_id).await?;

        Ok(())
    }
}
