use crate::database::{trackers, SqlitePool};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use domain::{DomainError, Tracker, TrackerRepository, TrackerStatus};

// Database model
#[derive(Queryable, Selectable, AsChangeset, Debug)]
#[diesel(table_name = trackers)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct TrackerModel {
    id: i32,
    torrent_id: i32,
    url: String,
    status: String,
    last_announce: Option<NaiveDateTime>,
    next_announce: Option<NaiveDateTime>,
    seeders: Option<i32>,
    leechers: Option<i32>,
    completed: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = trackers)]
struct NewTrackerModel {
    torrent_id: i32,
    url: String,
    status: String,
    last_announce: Option<NaiveDateTime>,
    next_announce: Option<NaiveDateTime>,
    seeders: Option<i32>,
    leechers: Option<i32>,
    completed: Option<i32>,
}

impl From<TrackerModel> for Tracker {
    fn from(model: TrackerModel) -> Self {
        let status = match model.status.as_str() {
            "active" => TrackerStatus::Active,
            "failed" => TrackerStatus::Failed,
            "disabled" => TrackerStatus::Disabled,
            _ => TrackerStatus::Failed,
        };

        let last_announce = model.last_announce.map(|dt| {
            std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(dt.and_utc().timestamp() as u64)
        });

        let next_announce = model.next_announce.map(|dt| {
            std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(dt.and_utc().timestamp() as u64)
        });

        Tracker {
            id: Some(model.id),
            torrent_id: model.torrent_id,
            url: model.url,
            status,
            last_announce,
            next_announce,
            seeders: model.seeders,
            leechers: model.leechers,
            completed: model.completed,
        }
    }
}

impl From<&Tracker> for NewTrackerModel {
    fn from(tracker: &Tracker) -> Self {
        let status_str = match &tracker.status {
            TrackerStatus::Active => "active",
            TrackerStatus::Failed => "failed",
            TrackerStatus::Disabled => "disabled",
        };

        let last_announce = tracker
            .last_announce
            .map(|st| chrono::DateTime::<chrono::Utc>::from(st).naive_utc());

        let next_announce = tracker
            .next_announce
            .map(|st| chrono::DateTime::<chrono::Utc>::from(st).naive_utc());

        NewTrackerModel {
            torrent_id: tracker.torrent_id,
            url: tracker.url.clone(),
            status: status_str.to_string(),
            last_announce,
            next_announce,
            seeders: tracker.seeders,
            leechers: tracker.leechers,
            completed: tracker.completed,
        }
    }
}

pub struct SqliteTrackerRepository {
    pool: SqlitePool,
}

impl SqliteTrackerRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TrackerRepository for SqliteTrackerRepository {
    async fn find_by_torrent_id(&self, torrent_id: i32) -> Result<Vec<Tracker>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let result = tokio::task::spawn_blocking(move || {
            trackers::table
                .filter(trackers::torrent_id.eq(torrent_id))
                .select(TrackerModel::as_select())
                .load::<TrackerModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }

    async fn find_active(&self, torrent_id: i32) -> Result<Vec<Tracker>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let result = tokio::task::spawn_blocking(move || {
            trackers::table
                .filter(trackers::torrent_id.eq(torrent_id))
                .filter(trackers::status.eq("active"))
                .select(TrackerModel::as_select())
                .load::<TrackerModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }

    async fn save(&self, tracker: &Tracker) -> Result<Tracker, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let new_tracker = NewTrackerModel::from(tracker);

        let result = tokio::task::spawn_blocking(move || {
            diesel::insert_into(trackers::table)
                .values(&new_tracker)
                .execute(&mut conn)?;

            // Get the last inserted row
            trackers::table
                .order(trackers::id.desc())
                .select(TrackerModel::as_select())
                .first::<TrackerModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into())
    }

    async fn update(&self, tracker: &Tracker) -> Result<Tracker, DomainError> {
        let tracker_id = tracker.id.ok_or_else(|| {
            DomainError::ValidationError("Tracker ID is required for updates".to_string())
        })?;

        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let status_str = match &tracker.status {
            TrackerStatus::Active => "active",
            TrackerStatus::Failed => "failed",
            TrackerStatus::Disabled => "disabled",
        }
        .to_string();

        let last_announce = tracker
            .last_announce
            .map(|st| chrono::DateTime::<chrono::Utc>::from(st).naive_utc());

        let next_announce = tracker
            .next_announce
            .map(|st| chrono::DateTime::<chrono::Utc>::from(st).naive_utc());

        let seeders = tracker.seeders;
        let leechers = tracker.leechers;
        let completed = tracker.completed;

        let result = tokio::task::spawn_blocking(move || {
            diesel::update(trackers::table.filter(trackers::id.eq(tracker_id)))
                .set((
                    trackers::status.eq(status_str),
                    trackers::last_announce.eq(last_announce),
                    trackers::next_announce.eq(next_announce),
                    trackers::seeders.eq(seeders),
                    trackers::leechers.eq(leechers),
                    trackers::completed.eq(completed),
                ))
                .execute(&mut conn)?;

            // Fetch the updated tracker
            trackers::table
                .filter(trackers::id.eq(tracker_id))
                .select(TrackerModel::as_select())
                .first::<TrackerModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into())
    }

    async fn save_batch(&self, trackers: &[Tracker]) -> Result<Vec<Tracker>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let new_trackers: Vec<NewTrackerModel> =
            trackers.iter().map(|t| NewTrackerModel::from(t)).collect();

        let result = tokio::task::spawn_blocking(move || {
            diesel::insert_into(trackers::table)
                .values(&new_trackers)
                .execute(&mut conn)?;

            // Return the trackers that were just inserted
            trackers::table
                .order(trackers::id.desc())
                .limit(new_trackers.len() as i64)
                .select(TrackerModel::as_select())
                .load::<TrackerModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }
}
