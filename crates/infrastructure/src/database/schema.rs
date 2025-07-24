// Database schema for torrent application
diesel::table! {
    torrents (id) {
        id -> Integer,
        info_hash -> Text,         // SHA1 hash of the torrent info
        name -> Text,              // Name of the torrent
        total_size -> BigInt,      // Total size in bytes
        piece_length -> Integer,   // Length of each piece
        piece_count -> Integer,    // Total number of pieces
        file_path -> Nullable<Text>, // Local file path if downloaded
        status -> Text,            // downloading, completed, paused, error
        progress -> Float,         // Download progress (0.0 - 1.0)
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    torrent_files (id) {
        id -> Integer,
        torrent_id -> Integer,
        path -> Text,              // File path within torrent
        length -> BigInt,          // File size in bytes
        offset -> BigInt,          // Byte offset in the torrent
    }
}

diesel::table! {
    pieces (id) {
        id -> Integer,
        torrent_id -> Integer,
        piece_index -> Integer,    // Index of the piece
        hash -> Text,              // SHA1 hash of the piece
        downloaded -> Bool,        // Whether piece is downloaded
        verified -> Bool,          // Whether piece hash is verified
    }
}

diesel::table! {
    peers (id) {
        id -> Integer,
        torrent_id -> Integer,
        ip -> Text,
        port -> Integer,
        peer_id -> Nullable<Text>,
        last_seen -> Timestamp,
        status -> Text,            // connected, disconnected, banned
    }
}

diesel::table! {
    trackers (id) {
        id -> Integer,
        torrent_id -> Integer,
        url -> Text,
        status -> Text,            // active, failed, disabled
        last_announce -> Nullable<Timestamp>,
        next_announce -> Nullable<Timestamp>,
        seeders -> Nullable<Integer>,
        leechers -> Nullable<Integer>,
        completed -> Nullable<Integer>,
    }
}

diesel::joinable!(torrent_files -> torrents (torrent_id));
diesel::joinable!(pieces -> torrents (torrent_id));
diesel::joinable!(peers -> torrents (torrent_id));
diesel::joinable!(trackers -> torrents (torrent_id));

diesel::allow_tables_to_appear_in_same_query!(torrents, torrent_files, pieces, peers, trackers,);
