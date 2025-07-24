use application::TorrentApp;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Stremio BitTorrent Client");

    // Initialize the torrent application
    let app = TorrentApp::new("stremio.db");

    // Simulate torrent file data (normally you'd read from a .torrent file)
    let mock_torrent_data = vec![
        // This would be actual .torrent file bytes
        // For demo purposes, we're using mock data
        0x64, 0x38, 0x3a, 0x61, 0x6e, 0x6e, 0x6f, 0x75, 0x6e, 0x63,
        0x65,
        // ... rest would be actual torrent file content
    ];

    println!("📄 Parsing .torrent file...");

    // Execute the complete BitTorrent download flow
    match app.download_torrent(mock_torrent_data).await {
        Ok(stream_url) => {
            println!("✅ Torrent ready for streaming!");
            println!("🎬 Stream URL: {}", stream_url);
            println!();
            println!("🎯 Next steps:");
            println!("   • Launch mpv: `mpv {}`", stream_url);
            println!("   • Or serve via HTTP for browser playback");
            println!("   • Download continues in background");

            // Demo: Show torrent status
            println!();
            println!("📊 Current torrents:");
            let torrents = app.torrent_service.get_all_torrents().await?;
            for torrent in torrents {
                println!(
                    "   • {} - {:.1}% complete",
                    torrent.name,
                    torrent.progress * 100.0
                );
                println!(
                    "     Size: {:.1} MB | Pieces: {}/{}",
                    torrent.total_size as f64 / (1024.0 * 1024.0),
                    (torrent.progress * torrent.piece_count as f32) as i32,
                    torrent.piece_count
                );
            }
        }
        Err(e) => {
            eprintln!("❌ Error downloading torrent: {}", e);
            return Err(e.into());
        }
    }

    println!();
    println!("🏁 Demo complete!");
    Ok(())
}
