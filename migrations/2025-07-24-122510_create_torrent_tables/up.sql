-- Create torrents table
CREATE TABLE torrents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    info_hash TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    total_size BIGINT NOT NULL,
    piece_length INTEGER NOT NULL,
    piece_count INTEGER NOT NULL,
    file_path TEXT,
    status TEXT NOT NULL DEFAULT 'parsing',
    progress REAL NOT NULL DEFAULT 0.0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create pieces table
CREATE TABLE pieces (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    torrent_id INTEGER NOT NULL,
    piece_index INTEGER NOT NULL,
    hash TEXT NOT NULL,
    downloaded BOOLEAN NOT NULL DEFAULT FALSE,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    FOREIGN KEY (torrent_id) REFERENCES torrents(id) ON DELETE CASCADE,
    UNIQUE(torrent_id, piece_index)
);

-- Create trackers table
CREATE TABLE trackers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    torrent_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    last_announce TIMESTAMP,
    next_announce TIMESTAMP,
    seeders INTEGER,
    leechers INTEGER,
    completed INTEGER,
    FOREIGN KEY (torrent_id) REFERENCES torrents(id) ON DELETE CASCADE
);

-- Create peers table
CREATE TABLE peers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    torrent_id INTEGER NOT NULL,
    ip TEXT NOT NULL,
    port INTEGER NOT NULL,
    peer_id TEXT,
    last_seen TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status TEXT NOT NULL DEFAULT 'disconnected',
    FOREIGN KEY (torrent_id) REFERENCES torrents(id) ON DELETE CASCADE,
    UNIQUE(torrent_id, ip, port)
);

-- Create torrent_files table
CREATE TABLE torrent_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    torrent_id INTEGER NOT NULL,
    path TEXT NOT NULL,
    length BIGINT NOT NULL,
    FOREIGN KEY (torrent_id) REFERENCES torrents(id) ON DELETE CASCADE
);

-- Create indexes for performance
CREATE INDEX idx_torrents_info_hash ON torrents(info_hash);
CREATE INDEX idx_torrents_status ON torrents(status);
CREATE INDEX idx_pieces_torrent_id ON pieces(torrent_id);
CREATE INDEX idx_pieces_downloaded ON pieces(downloaded);
CREATE INDEX idx_trackers_torrent_id ON trackers(torrent_id);
CREATE INDEX idx_peers_torrent_id ON peers(torrent_id);
CREATE INDEX idx_peers_status ON peers(status);
CREATE INDEX idx_torrent_files_torrent_id ON torrent_files(torrent_id);
