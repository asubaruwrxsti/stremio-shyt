use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Torrent not found with id: {0}")]
    TorrentNotFound(i32),

    #[error("Torrent not found with info hash: {0}")]
    TorrentNotFoundByHash(String),

    #[error("Invalid torrent file: {0}")]
    InvalidTorrent(String),

    #[error("Piece verification failed: piece {0}")]
    PieceVerificationFailed(i32),

    #[error("Tracker error: {0}")]
    TrackerError(String),

    #[error("Peer connection error: {0}")]
    PeerConnectionError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Repository error: {0}")]
    RepositoryError(String),

    #[error("Parsing error: {0}")]
    ParsingError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}
