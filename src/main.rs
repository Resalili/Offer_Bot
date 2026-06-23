mod bot;
mod services;
mod db;
mod cache;
mod utils;

use axum::{routing::post, Router};
use teloxide::prelude::*;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    // tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let bot = Bot::from_env();

    let app = Router::new()
        .route("/webhook", post(bot::handlers::webhook_handler))
        .with_state(Arc::new(bot));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}