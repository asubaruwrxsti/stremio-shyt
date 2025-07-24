use crate::database::{peers, SqlitePool};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use domain::{DomainError, Peer, PeerRepository, PeerStatus};

// Database model
#[derive(Queryable, Selectable, AsChangeset, Debug)]
#[diesel(table_name = peers)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct PeerModel {
    id: i32,
    torrent_id: i32,
    ip: String,
    port: i32,
    peer_id: Option<String>,
    last_seen: NaiveDateTime,
    status: String,
}

#[derive(Insertable)]
#[diesel(table_name = peers)]
struct NewPeerModel {
    torrent_id: i32,
    ip: String,
    port: i32,
    peer_id: Option<String>,
    last_seen: NaiveDateTime,
    status: String,
}

impl From<PeerModel> for Peer {
    fn from(model: PeerModel) -> Self {
        let status = match model.status.as_str() {
            "disconnected" => PeerStatus::Disconnected,
            "connecting" => PeerStatus::Connecting,
            "connected" => PeerStatus::Connected,
            "banned" => PeerStatus::Banned,
            _ => PeerStatus::Disconnected,
        };

        let last_seen = std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(model.last_seen.and_utc().timestamp() as u64);

        Peer {
            id: Some(model.id),
            torrent_id: model.torrent_id,
            ip: model.ip,
            port: model.port as u16,
            peer_id: model.peer_id,
            last_seen,
            status,
        }
    }
}

impl From<&Peer> for NewPeerModel {
    fn from(peer: &Peer) -> Self {
        let status_str = match &peer.status {
            PeerStatus::Disconnected => "disconnected",
            PeerStatus::Connecting => "connecting",
            PeerStatus::Connected => "connected",
            PeerStatus::Banned => "banned",
        };

        let last_seen = chrono::DateTime::<chrono::Utc>::from(peer.last_seen).naive_utc();

        NewPeerModel {
            torrent_id: peer.torrent_id,
            ip: peer.ip.clone(),
            port: peer.port as i32,
            peer_id: peer.peer_id.clone(),
            last_seen,
            status: status_str.to_string(),
        }
    }
}

pub struct SqlitePeerRepository {
    pool: SqlitePool,
}

impl SqlitePeerRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PeerRepository for SqlitePeerRepository {
    async fn find_by_torrent_id(&self, torrent_id: i32) -> Result<Vec<Peer>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let result = tokio::task::spawn_blocking(move || {
            peers::table
                .filter(peers::torrent_id.eq(torrent_id))
                .select(PeerModel::as_select())
                .load::<PeerModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }

    async fn find_connected(&self, torrent_id: i32) -> Result<Vec<Peer>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let result = tokio::task::spawn_blocking(move || {
            peers::table
                .filter(peers::torrent_id.eq(torrent_id))
                .filter(peers::status.eq("connected"))
                .select(PeerModel::as_select())
                .load::<PeerModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }

    async fn save(&self, peer: &Peer) -> Result<Peer, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let new_peer = NewPeerModel::from(peer);

        let result = tokio::task::spawn_blocking(move || {
            diesel::insert_into(peers::table)
                .values(&new_peer)
                .execute(&mut conn)?;

            // Get the last inserted row
            peers::table
                .order(peers::id.desc())
                .select(PeerModel::as_select())
                .first::<PeerModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into())
    }

    async fn update(&self, peer: &Peer) -> Result<Peer, DomainError> {
        let peer_id = peer.id.ok_or_else(|| {
            DomainError::ValidationError("Peer ID is required for updates".to_string())
        })?;

        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let status_str = match &peer.status {
            PeerStatus::Disconnected => "disconnected",
            PeerStatus::Connecting => "connecting",
            PeerStatus::Connected => "connected",
            PeerStatus::Banned => "banned",
        }
        .to_string();

        let last_seen = chrono::DateTime::<chrono::Utc>::from(peer.last_seen).naive_utc();
        let peer_id_opt = peer.peer_id.clone();

        let result = tokio::task::spawn_blocking(move || {
            diesel::update(peers::table.filter(peers::id.eq(peer_id)))
                .set((
                    peers::status.eq(status_str),
                    peers::last_seen.eq(last_seen),
                    peers::peer_id.eq(peer_id_opt),
                ))
                .execute(&mut conn)?;

            // Fetch the updated peer
            peers::table
                .filter(peers::id.eq(peer_id))
                .select(PeerModel::as_select())
                .first::<PeerModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into())
    }

    async fn save_batch(&self, peers: &[Peer]) -> Result<Vec<Peer>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let new_peers: Vec<NewPeerModel> = peers.iter().map(|p| NewPeerModel::from(p)).collect();

        let result = tokio::task::spawn_blocking(move || {
            diesel::insert_into(peers::table)
                .values(&new_peers)
                .execute(&mut conn)?;

            // Return the peers that were just inserted
            peers::table
                .order(peers::id.desc())
                .limit(new_peers.len() as i64)
                .select(PeerModel::as_select())
                .load::<PeerModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }

    async fn delete_old(&self, torrent_id: i32, hours: u32) -> Result<(), DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let cutoff_time = chrono::Utc::now().naive_utc() - chrono::Duration::hours(hours as i64);

        tokio::task::spawn_blocking(move || {
            diesel::delete(
                peers::table
                    .filter(peers::torrent_id.eq(torrent_id))
                    .filter(peers::last_seen.lt(cutoff_time)),
            )
            .execute(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(())
    }
}
