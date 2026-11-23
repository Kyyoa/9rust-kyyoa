use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{
    routing::{get, post},
    Router,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod rtk;
mod handlers;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.json".to_string());
    let cfg = match config::AppConfig::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to load {}: {}. Using default config.", config_path, e);
            config::AppConfig::default()
        }
    };

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(cfg.port);

    let state = Arc::new(config::AppState {
        config: Arc::new(RwLock::new(cfg)),
        client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?,
        rtk: rtk::RtkEngine::new(),
    });

    let app = Router::new()
        .route("/health", get(handlers::health_handler))
        .route("/v1/models", get(handlers::models_handler))
        .route("/v1/chat/completions", post(handlers::chat_completions_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("🦀 9rust-kyyoa AI Gateway running on port {}", port);
    tracing::info!("⚡ Token Saver RTK: active");

    axum::serve(listener, app).await?;

    Ok(())
}
