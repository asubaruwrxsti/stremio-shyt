-- Drop indexes
DROP INDEX IF EXISTS idx_torrent_files_torrent_id;
DROP INDEX IF EXISTS idx_peers_status;
DROP INDEX IF EXISTS idx_peers_torrent_id;
DROP INDEX IF EXISTS idx_trackers_torrent_id;
DROP INDEX IF EXISTS idx_pieces_downloaded;
DROP INDEX IF EXISTS idx_pieces_torrent_id;
DROP INDEX IF EXISTS idx_torrents_status;
DROP INDEX IF EXISTS idx_torrents_info_hash;

-- Drop tables in reverse order (due to foreign keys)
DROP TABLE IF EXISTS torrent_files;
DROP TABLE IF EXISTS peers;
DROP TABLE IF EXISTS trackers;
DROP TABLE IF EXISTS pieces;
DROP TABLE IF EXISTS torrents;
