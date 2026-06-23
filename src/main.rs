mod bot;
mod services;
mod db;
mod cache;
mod utils;

use axum::{routing::{get, post}, Router};
use axum::http::StatusCode;
use teloxide::prelude::*;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use sqlx::SqlitePool;

pub struct AppState {
    pub bot: Bot,
    pub db: SqlitePool,
}
#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    // tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let bot = Bot::from_env();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://offerbot.db".to_string());

    // Ensure parent dir and DB file exist for sqlite URLs to avoid "unable to open database file".
    let database_url = {
        if let Some(path_str) = database_url.strip_prefix("sqlite://") {
            use std::path::PathBuf;
            let mut db_path = PathBuf::from(path_str);
            if !db_path.is_absolute() {
                db_path = std::env::current_dir().expect("cwd").join(db_path);
            }
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).expect("create db parent dirs");
            }
            if !db_path.exists() {
                // create empty file so sqlite can open it with current permissions
                std::fs::File::create(&db_path).expect("create db file");
            }
            format!("sqlite://{}", db_path.to_string_lossy())
        } else {
            database_url
        }
    };

    let db_pool = db::sqlite::init_db(&database_url).await.expect("DB init");

    let state = Arc::new(AppState { bot: bot.clone(), db: db_pool });

    let app = Router::new()
        .route(
            "/webhook",
            get(|| async { StatusCode::OK }).post(bot::handlers::webhook_handler),
        )
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}