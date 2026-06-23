use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Job {
    pub id: i64,
    pub title: String,
    pub budget: i32,
}

pub async fn create_job(_j: Job) -> anyhow::Result<Job> {
    Ok(_j)
}
