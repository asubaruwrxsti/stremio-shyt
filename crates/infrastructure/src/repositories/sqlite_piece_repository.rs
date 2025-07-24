use crate::database::{pieces, SqlitePool};
use async_trait::async_trait;
use diesel::prelude::*;
use domain::{DomainError, Piece, PieceRepository};

// Database model
#[derive(Queryable, Selectable, AsChangeset, Debug)]
#[diesel(table_name = pieces)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct PieceModel {
    id: i32,
    torrent_id: i32,
    piece_index: i32,
    hash: String,
    downloaded: bool,
    verified: bool,
}

#[derive(Insertable)]
#[diesel(table_name = pieces)]
struct NewPieceModel {
    torrent_id: i32,
    piece_index: i32,
    hash: String,
    downloaded: bool,
    verified: bool,
}

impl From<PieceModel> for Piece {
    fn from(model: PieceModel) -> Self {
        Piece {
            id: Some(model.id),
            torrent_id: model.torrent_id,
            piece_index: model.piece_index,
            hash: model.hash,
            downloaded: model.downloaded,
            verified: model.verified,
        }
    }
}

impl From<&Piece> for NewPieceModel {
    fn from(piece: &Piece) -> Self {
        NewPieceModel {
            torrent_id: piece.torrent_id,
            piece_index: piece.piece_index,
            hash: piece.hash.clone(),
            downloaded: piece.downloaded,
            verified: piece.verified,
        }
    }
}

pub struct SqlitePieceRepository {
    pool: SqlitePool,
}

impl SqlitePieceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PieceRepository for SqlitePieceRepository {
    async fn find_by_torrent_id(&self, torrent_id: i32) -> Result<Vec<Piece>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let result = tokio::task::spawn_blocking(move || {
            pieces::table
                .filter(pieces::torrent_id.eq(torrent_id))
                .select(PieceModel::as_select())
                .load::<PieceModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }

    async fn find_by_torrent_and_index(
        &self,
        torrent_id: i32,
        piece_index: i32,
    ) -> Result<Option<Piece>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let result = tokio::task::spawn_blocking(move || {
            pieces::table
                .filter(pieces::torrent_id.eq(torrent_id))
                .filter(pieces::piece_index.eq(piece_index))
                .select(PieceModel::as_select())
                .first::<PieceModel>(&mut conn)
                .optional()
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.map(|model| model.into()))
    }

    async fn save(&self, piece: &Piece) -> Result<Piece, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let new_piece = NewPieceModel::from(piece);

        let result = tokio::task::spawn_blocking(move || {
            diesel::insert_into(pieces::table)
                .values(&new_piece)
                .execute(&mut conn)?;

            // Get the last inserted row
            pieces::table
                .order(pieces::id.desc())
                .select(PieceModel::as_select())
                .first::<PieceModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into())
    }

    async fn update(&self, piece: &Piece) -> Result<Piece, DomainError> {
        let piece_id = piece.id.ok_or_else(|| {
            DomainError::ValidationError("Piece ID is required for updates".to_string())
        })?;

        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let downloaded = piece.downloaded;
        let verified = piece.verified;

        let result = tokio::task::spawn_blocking(move || {
            diesel::update(pieces::table.filter(pieces::id.eq(piece_id)))
                .set((
                    pieces::downloaded.eq(downloaded),
                    pieces::verified.eq(verified),
                ))
                .execute(&mut conn)?;

            // Fetch the updated piece
            pieces::table
                .filter(pieces::id.eq(piece_id))
                .select(PieceModel::as_select())
                .first::<PieceModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into())
    }

    async fn save_batch(&self, pieces: &[Piece]) -> Result<Vec<Piece>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let new_pieces: Vec<NewPieceModel> =
            pieces.iter().map(|p| NewPieceModel::from(p)).collect();

        let result = tokio::task::spawn_blocking(move || {
            diesel::insert_into(pieces::table)
                .values(&new_pieces)
                .execute(&mut conn)?;

            // Return the pieces that were just inserted
            // This is a simplified approach - in production you might want to be more precise
            pieces::table
                .order(pieces::id.desc())
                .limit(new_pieces.len() as i64)
                .select(PieceModel::as_select())
                .load::<PieceModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }

    async fn count_downloaded(&self, torrent_id: i32) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let count = tokio::task::spawn_blocking(move || {
            pieces::table
                .filter(pieces::torrent_id.eq(torrent_id))
                .filter(pieces::downloaded.eq(true))
                .filter(pieces::verified.eq(true))
                .count()
                .get_result::<i64>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(count as i32)
    }

    async fn find_next_needed(
        &self,
        torrent_id: i32,
        limit: i32,
    ) -> Result<Vec<Piece>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        let result = tokio::task::spawn_blocking(move || {
            pieces::table
                .filter(pieces::torrent_id.eq(torrent_id))
                .filter(pieces::downloaded.eq(false))
                .order(pieces::piece_index.asc())
                .limit(limit as i64)
                .select(PieceModel::as_select())
                .load::<PieceModel>(&mut conn)
        })
        .await
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?
        .map_err(|e| DomainError::RepositoryError(e.to_string()))?;

        Ok(result.into_iter().map(|model| model.into()).collect())
    }
}
