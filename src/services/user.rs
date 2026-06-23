use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub nickname: Option<String>,
    pub avatar: Option<String>,
    pub description: Option<String>,
    pub skills: Option<String>,
    pub stage: Option<i32>,
    pub job_stage: Option<i32>,
    pub job_draft_title: Option<String>,
    pub job_draft_budget: Option<i32>,
    pub job_draft_skills: Option<String>,
    pub job_draft_description: Option<String>,
}

pub async fn create_user(_u: User) -> anyhow::Result<User> {
    Ok(_u)
}
