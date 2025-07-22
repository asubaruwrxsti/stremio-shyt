   use axum::{
       routing::{get, post},
       Router,
   };
   use std::net::SocketAddr;
   use tracing_subscriber;

   mod config;
   use config::Config;

   #[tokio::main]
   async fn main() {
       // Initialize tracing for logging
       tracing_subscriber::fmt::init();

       // Define routes
       let app = Router::new()
           .route("/", get(root))
           .route("/api/health", get(health_check));

       // Define the address to run the server on
       let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
       tracing::info!("Server running on {}", addr);

       // Start the server
       axum::Server::bind(&addr)
           .serve(app.into_make_service())
           .await
           .unwrap();
   }

   // Handler for the root route
   async fn root() -> &'static str {
       "Welcome to your Stremio Alternative Backend!"
   }

   // Health check endpoint
   async fn health_check() -> &'static str {
       "OK"
   }
