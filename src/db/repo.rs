// Lightweight repo stubs — replace with real DB access (rusqlite/sqlx) later.
use crate::services::{job::Job, user::User};

pub async fn save_user(user: User) -> anyhow::Result<User> {
    // persist to DB
    Ok(user)
}

pub async fn save_job(job: Job) -> anyhow::Result<Job> {
    Ok(job)
}
