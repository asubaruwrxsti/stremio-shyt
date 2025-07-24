use crate::database::{torrents, SqlitePool};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use domain::{DomainError, Torrent, TorrentRepository, TorrentStatus};

// Database model - separate from domain entity
#[derive(Queryable, Selectable, AsChangeset, Debug)]
#[diesel(table_name = torrents)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct TorrentModel {
    id: i32,
    info_hash: String,
    name: String,
    total_size: i64,
    piece_length: i32,
    piece_count: i32,
    file_path: Option<String>,
    status: String,
    progress: f32,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = torrents)]
struct NewTorrentModel {
    info_hash: String,
    name: String,
    total_size: i64,
    piece_length: i32,
    piece_count: i32,
    file_path: Option<String>,
    status: String,
    progress: f32,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

// Convert between domain and database models
impl From<TorrentModel> for Torrent {
    fn from(model: TorrentModel) -> Self {
        let status = match model.status.as_str() {
            "parsing" => TorrentStatus::Parsing,
            "connecting" => TorrentStatus::Connecting,
            "downloading" => TorrentStatus::Downloading,
            "seeding" => TorrentStatus::Seeding,
            "paused" => TorrentStatus::Paused,
            "completed" => TorrentStatus::Completed,
            error_msg => TorrentStatus::Error(error_msg.to_string()),
        };

        Torrent::with_id(
            model.id,
            model.info_hash,
            model.name,
            model.total_size,
            model.piece_length,
            model.piece_count,
            model.file_path,
            status,
            model.progress,
            std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(model.created_at.and_utc().timestamp() as u64),
            std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(model.updated_at.and_utc().timestamp() as u64),
        )
    }
}

impl From<&Torrent> for NewTorrentModel {
    fn from(torrent: &Torrent) -> Self {
        let status_str = match &torrent.status {
            TorrentStatus::Parsing => "parsing",
            TorrentStatus::Connecting => "connecting",
            TorrentStatus::Downloading => "downloading",
            TorrentStatus::Seeding => "seeding",
            TorrentStatus::Paused => "paused",
            TorrentStatus::Completed => "completed",
            TorrentStatus::Error(msg) => msg,
        };

        let now = chrono::Utc::now().naive_utc();

        NewTorrentModel {
            info_hash: torrent.info_hash.clone(),
            name: torrent.name.clone(),
            total_size: torrent.total_size,
            piece_length: torrent.piece_length,
            piece_count: torrent.piece_count,
            file_path: torrent.file_path.clone(),
            status: status_str.to_string(),
            progress: torrent.progress,
            created_at: now,
            updated_at: now,
        }
    }
}

pub struct SqliteTorrentRepository {
    pool: SqlitePool,
}

impl SqliteTorrentRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TorrentRepository for SqliteTorrentRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<Torrent>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let result = tokio::task::spawn_blocking(move || {
            torrents::table
                .filter(torrents::id.eq(id))
                .select(TorrentModel::as_select())
                .first::<TorrentModel>(&mut conn)
                .optional()
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.map(|model| model.into()))
    }

    async fn find_by_info_hash(&self, info_hash: &str) -> Result<Option<Torrent>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let info_hash = info_hash.to_string();
        let result = tokio::task::spawn_blocking(move || {
            torrents::table
                .filter(torrents::info_hash.eq(info_hash))
                .select(TorrentModel::as_select())
                .first::<TorrentModel>(&mut conn)
                .optional()
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.map(|model| model.into()))
    }

    async fn save(&self, torrent: &Torrent) -> Result<Torrent, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let new_torrent = NewTorrentModel::from(torrent);

        let result = tokio::task::spawn_blocking(move || {
            diesel::insert_into(torrents::table)
                .values(&new_torrent)
                .execute(&mut conn)?;

            // Get the last inserted row
            torrents::table
                .order(torrents::id.desc())
                .select(TorrentModel::as_select())
                .first::<TorrentModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into())
    }

    async fn update(&self, torrent: &Torrent) -> Result<Torrent, DomainError> {
        let torrent_id = torrent.id.ok_or_else(|| {
            DomainError::ValidationError("Torrent ID is required for updates".to_string())
        })?;

        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let status_str = match &torrent.status {
            TorrentStatus::Parsing => "parsing",
            TorrentStatus::Connecting => "connecting",
            TorrentStatus::Downloading => "downloading",
            TorrentStatus::Seeding => "seeding",
            TorrentStatus::Paused => "paused",
            TorrentStatus::Completed => "completed",
            TorrentStatus::Error(msg) => msg,
        }
        .to_string();

        let now = chrono::Utc::now().naive_utc();
        let progress = torrent.progress;
        let file_path = torrent.file_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            diesel::update(torrents::table.filter(torrents::id.eq(torrent_id)))
                .set((
                    torrents::status.eq(status_str),
                    torrents::progress.eq(progress),
                    torrents::file_path.eq(file_path),
                    torrents::updated_at.eq(now),
                ))
                .execute(&mut conn)?;

            // Fetch the updated torrent
            torrents::table
                .filter(torrents::id.eq(torrent_id))
                .select(TorrentModel::as_select())
                .first::<TorrentModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into())
    }

    async fn delete(&self, id: i32) -> Result<(), DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        tokio::task::spawn_blocking(move || {
            diesel::delete(torrents::table.filter(torrents::id.eq(id))).execute(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(())
    }

    async fn find_all(&self) -> Result<Vec<Torrent>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let result = tokio::task::spawn_blocking(move || {
            torrents::table
                .select(TorrentModel::as_select())
                .load::<TorrentModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }

    async fn find_active(&self) -> Result<Vec<Torrent>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let result = tokio::task::spawn_blocking(move || {
            torrents::table
                .filter(torrents::status.ne("completed"))
                .filter(torrents::status.ne("paused"))
                .select(TorrentModel::as_select())
                .load::<TorrentModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }
}
