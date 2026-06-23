use axum::{extract::State, http::StatusCode, Json};
use teloxide::prelude::*;
use std::sync::Arc;
use tracing::info;

pub async fn webhook_handler(
    State(_bot): State<Arc<Bot>>,
    Json(update): Json<teloxide::types::Update>,
) -> StatusCode {
    info!("received update: {:?}", update);
    StatusCode::OK
}
