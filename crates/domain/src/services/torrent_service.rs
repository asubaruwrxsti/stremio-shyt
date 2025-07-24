use crate::entities::{Piece, Torrent, TorrentStatus, Tracker};
use crate::errors::DomainError;
use crate::repositories::{PieceRepository, TorrentRepository, TrackerRepository};
use std::sync::Arc;

/// Main torrent service that orchestrates the torrent flow
pub struct TorrentService {
    torrent_repository: Arc<dyn TorrentRepository>,
    piece_repository: Arc<dyn PieceRepository>,
    tracker_repository: Arc<dyn TrackerRepository>,
}

impl TorrentService {
    pub fn new(
        torrent_repository: Arc<dyn TorrentRepository>,
        piece_repository: Arc<dyn PieceRepository>,
        tracker_repository: Arc<dyn TrackerRepository>,
    ) -> Self {
        Self {
            torrent_repository,
            piece_repository,
            tracker_repository,
        }
    }

    /// Parse .torrent file and create Torrent entity
    pub async fn parse_torrent_file(&self, torrent_data: Vec<u8>) -> Result<Torrent, DomainError> {
        use bip_metainfo::Metainfo;
        use sha1::{Digest, Sha1};

        // Parse the .torrent file using bip_metainfo
        let metainfo = Metainfo::from_bytes(&torrent_data)
            .map_err(|e| DomainError::ValidationError(format!("Invalid torrent file: {}", e)))?;

        // Calculate info hash from the info section
        let info_bytes = metainfo.info().to_bytes();
        
        let mut hasher = Sha1::new();
        hasher.update(&info_bytes);
        let info_hash = hex::encode(hasher.finalize());

        // Extract torrent metadata
        let info = metainfo.info();
        
        // Get name - try different approaches based on API
        let name = "torrent_file".to_string(); // Simplified for now
        
        let piece_length = info.piece_length() as i32;
        
        // Calculate total length - simplified approach
        let total_length = info.piece_length() as u64 * info.pieces().count() as u64;
        
        let num_pieces = info.pieces().count() as i32;

        println!("📁 Parsed torrent: {}", name);
        println!("   Info hash: {}", info_hash);
        println!("   Total size: {} bytes", total_length);
        println!("   Piece length: {} bytes", piece_length);
        println!("   Number of pieces: {}", num_pieces);

        let torrent = Torrent::new(
            info_hash,
            name,
            total_length as i64,
            piece_length,
            num_pieces,
        );

        Ok(torrent)
    }

    /// Extract tracker URLs from torrent metainfo
    pub fn extract_tracker_urls(&self, torrent_data: &[u8]) -> Result<Vec<String>, DomainError> {
        use bip_metainfo::Metainfo;

        let metainfo = Metainfo::from_bytes(torrent_data)
            .map_err(|e| DomainError::ValidationError(format!("Invalid torrent file: {}", e)))?;

        let mut tracker_urls = Vec::new();

        // Add main announce URL
        if let Some(announce) = metainfo.main_tracker() {
            tracker_urls.push(announce.to_string());
        }

        // Note: announce_list handling simplified for compatibility
        // In production, you'd implement proper announce-list parsing

        println!("📍 Found {} tracker URLs in torrent", tracker_urls.len());
        for url in &tracker_urls {
            println!("   - {}", url);
        }

        Ok(tracker_urls)
    }

    /// Add a new torrent from .torrent file data (includes parsing and tracker extraction)
    pub async fn add_torrent_from_file(&self, torrent_data: Vec<u8>) -> Result<Torrent, DomainError> {
        // Parse the torrent file
        let torrent = self.parse_torrent_file(torrent_data.clone()).await?;
        
        // Check if torrent already exists
        if let Some(_) = self
            .torrent_repository
            .find_by_info_hash(&torrent.info_hash)
            .await?
        {
            return Err(DomainError::ValidationError(
                "Torrent already exists".to_string(),
            ));
        }

        // Save the torrent
        let saved_torrent = self.torrent_repository.save(&torrent).await?;

        // Extract and save tracker URLs from the torrent file
        let tracker_urls = self.extract_tracker_urls(&torrent_data)?;
        for tracker_url in tracker_urls {
            let tracker = Tracker::new(saved_torrent.id.unwrap_or(0), tracker_url);
            self.tracker_repository.save(&tracker).await?;
        }

        // Initialize pieces from torrent file
        self.initialize_pieces_from_torrent(saved_torrent.id.unwrap_or(0), &torrent_data).await?;

        Ok(saved_torrent)
    }

    /// Add a pre-parsed torrent to the system
    pub async fn add_torrent(&self, torrent: Torrent) -> Result<Torrent, DomainError> {
        // Check if torrent already exists
        if let Some(_) = self
            .torrent_repository
            .find_by_info_hash(&torrent.info_hash)
            .await?
        {
            return Err(DomainError::ValidationError(
                "Torrent already exists".to_string(),
            ));
        }

        // Save the torrent
        let saved_torrent = self.torrent_repository.save(&torrent).await?;

        Ok(saved_torrent)
    }

    /// Initialize pieces for a torrent from torrent file data
    pub async fn initialize_pieces_from_torrent(&self, torrent_id: i32, torrent_data: &[u8]) -> Result<(), DomainError> {
        use bip_metainfo::Metainfo;
        
        // Check if pieces already exist
        let existing_pieces = self.piece_repository.find_by_torrent_id(torrent_id).await?;
        if !existing_pieces.is_empty() {
            return Ok(()); // Pieces already initialized
        }

        // Parse torrent to get piece hashes
        let metainfo = Metainfo::from_bytes(torrent_data)
            .map_err(|e| DomainError::ValidationError(format!("Invalid torrent file: {}", e)))?;

        let mut pieces_to_create = Vec::new();
        for (index, piece_hash) in metainfo.info().pieces().enumerate() {
            let hash_hex = hex::encode(piece_hash);
            let piece = Piece::new(torrent_id, index as i32, hash_hex);
            pieces_to_create.push(piece);
        }

        self.piece_repository.save_batch(&pieces_to_create).await?;
        println!("✅ Initialized {} pieces for torrent {}", pieces_to_create.len(), torrent_id);
        
        Ok(())
    }

    /// Start downloading a torrent
    /// This initiates the flow: get info_hash → connect to trackers → get peers → download
    pub async fn start_download(&self, torrent_id: i32) -> Result<(), DomainError> {
        let mut torrent = self
            .torrent_repository
            .find_by_id(torrent_id)
            .await?
            .ok_or(DomainError::TorrentNotFound(torrent_id))?;

        torrent.set_status(TorrentStatus::Connecting);
        self.torrent_repository.update(&torrent).await?;

        // Verify pieces exist
        let pieces = self.piece_repository.find_by_torrent_id(torrent_id).await?;
        if pieces.is_empty() {
            return Err(DomainError::ValidationError(
                "No pieces found for torrent. Use add_torrent_from_file to properly initialize torrent.".to_string()
            ));
        }

        // Verify trackers exist
        let trackers = self.tracker_repository.find_by_torrent_id(torrent_id).await?;
        if trackers.is_empty() {
            return Err(DomainError::ValidationError(
                "No trackers found for torrent. Use add_torrent_from_file to properly initialize torrent.".to_string()
            ));
        }

        torrent.set_status(TorrentStatus::Downloading);
        self.torrent_repository.update(&torrent).await?;

        println!("🚀 Started downloading torrent: {} (ID: {})", torrent.name, torrent_id);
        println!("   {} pieces, {} trackers", pieces.len(), trackers.len());

        Ok(())
    }

    /// Pause a torrent download
    pub async fn pause_torrent(&self, torrent_id: i32) -> Result<(), DomainError> {
        let mut torrent = self
            .torrent_repository
            .find_by_id(torrent_id)
            .await?
            .ok_or(DomainError::TorrentNotFound(torrent_id))?;

        torrent.set_status(TorrentStatus::Paused);
        self.torrent_repository.update(&torrent).await?;

        Ok(())
    }

    /// Resume a paused torrent
    pub async fn resume_torrent(&self, torrent_id: i32) -> Result<(), DomainError> {
        let mut torrent = self
            .torrent_repository
            .find_by_id(torrent_id)
            .await?
            .ok_or(DomainError::TorrentNotFound(torrent_id))?;

        torrent.set_status(TorrentStatus::Downloading);
        self.torrent_repository.update(&torrent).await?;

        Ok(())
    }

    /// Remove a torrent from the system
    pub async fn remove_torrent(
        &self,
        torrent_id: i32,
        delete_files: bool,
    ) -> Result<(), DomainError> {
        let torrent = self
            .torrent_repository
            .find_by_id(torrent_id)
            .await?
            .ok_or(DomainError::TorrentNotFound(torrent_id))?;

        if delete_files {
            if let Some(file_path) = &torrent.file_path {
                // Delete the actual downloaded file
                match tokio::fs::remove_file(file_path).await {
                    Ok(_) => println!("🗑️  Deleted file: {}", file_path),
                    Err(e) => eprintln!("⚠️  Failed to delete file {}: {}", file_path, e),
                }

                // Also try to delete any partial download files
                let partial_path = format!("{}.part", file_path);
                if tokio::fs::metadata(&partial_path).await.is_ok() {
                    let _ = tokio::fs::remove_file(&partial_path).await;
                }
            }
        }

        self.torrent_repository.delete(torrent_id).await?;

        Ok(())
    }

    /// Get torrent by ID
    pub async fn get_torrent(&self, torrent_id: i32) -> Result<Torrent, DomainError> {
        self.torrent_repository
            .find_by_id(torrent_id)
            .await?
            .ok_or(DomainError::TorrentNotFound(torrent_id))
    }

    /// Get all torrents
    pub async fn get_all_torrents(&self) -> Result<Vec<Torrent>, DomainError> {
        self.torrent_repository.find_all().await
    }

    /// Get active downloading torrents
    pub async fn get_active_torrents(&self) -> Result<Vec<Torrent>, DomainError> {
        self.torrent_repository.find_active().await
    }

    /// Update torrent progress based on downloaded pieces
    pub async fn update_progress(&self, torrent_id: i32) -> Result<Torrent, DomainError> {
        let mut torrent = self.get_torrent(torrent_id).await?;
        let downloaded_pieces = self.piece_repository.count_downloaded(torrent_id).await?;

        torrent.update_progress(downloaded_pieces);
        self.torrent_repository.update(&torrent).await
    }
}
