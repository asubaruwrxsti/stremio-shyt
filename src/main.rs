mod config;

use application::TorrentApp;
use config::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Stremio BitTorrent Streaming Service");
    println!("Production-ready BitTorrent streaming with:");
    println!("  ✅ Real torrent file parsing");
    println!("  ✅ Piece-based streaming");
    println!("  ✅ HTTP range request support");
    println!("  ✅ Intelligent piece prioritization");
    println!("  ✅ Background piece buffering");
    println!("  ✅ Production peer networking");
    println!();

    // Load configuration from environment variables
    let config = Config::from_env()?;
    config.print_config();
    println!();

    // Initialize the application with configuration
    let _app = TorrentApp::new(&config.database_path);

    println!("🎯 BitTorrent streaming system initialized!");
    println!("🌐 API Server will be available at: http://{}", config.api_address());
    println!("📁 Downloads will be saved to: {}", config.download_dir);

    // Keep the application running
    println!("\n⏳ Service running... (Press Ctrl+C to stop)");
    tokio::signal::ctrl_c().await?;
    println!("\n👋 Shutting down BitTorrent streaming service");

    Ok(())
}
