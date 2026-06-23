use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct Job {
    pub id: i64,
    pub title: String,
    pub budget: i32,
    pub skills: Option<String>,
    pub description: Option<String>,
    pub creator_id: i64,
}

pub async fn create_job(_j: Job) -> anyhow::Result<Job> {
    Ok(_j)
}
