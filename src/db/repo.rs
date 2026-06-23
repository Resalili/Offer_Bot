use crate::services::{job::Job, user::User};
use sqlx::SqlitePool;

pub async fn save_user(pool: &SqlitePool, user: User) -> anyhow::Result<User> {
    // clone fields so we don't partially move `user` (we return it)
    let u = user.clone();
    sqlx::query("INSERT INTO users (id, name, nickname, avatar, description, skills, stage, job_stage, job_draft_title, job_draft_budget, job_draft_skills, job_draft_description) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET name=excluded.name, nickname=excluded.nickname, avatar=excluded.avatar, description=excluded.description, skills=excluded.skills, stage=excluded.stage, job_stage=excluded.job_stage, job_draft_title=excluded.job_draft_title, job_draft_budget=excluded.job_draft_budget, job_draft_skills=excluded.job_draft_skills, job_draft_description=excluded.job_draft_description")
        .bind(u.id)
        .bind(&u.name)
        .bind(u.nickname)
        .bind(u.avatar)
        .bind(u.description)
        .bind(u.skills)
        .bind(u.stage)
        .bind(u.job_stage)
        .bind(u.job_draft_title)
        .bind(u.job_draft_budget)
        .bind(u.job_draft_skills)
        .bind(u.job_draft_description)
        .execute(pool)
        .await?;
    Ok(user)
}

pub async fn get_user(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<User>> {
    let row = sqlx::query_as::<_, User>("SELECT id, name, nickname, avatar, description, skills, stage, job_stage, job_draft_title, job_draft_budget, job_draft_skills, job_draft_description FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn update_user_stage(pool: &SqlitePool, id: i64, stage: i32) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET stage = ? WHERE id = ?")
        .bind(stage)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_user_job_stage(pool: &SqlitePool, id: i64, job_stage: i32) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET job_stage = ? WHERE id = ?")
        .bind(job_stage)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_user_job_draft(pool: &SqlitePool, id: i64, title: Option<String>, budget: Option<i32>, skills: Option<String>, description: Option<String>) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET job_draft_title = COALESCE(?, job_draft_title), job_draft_budget = COALESCE(?, job_draft_budget), job_draft_skills = COALESCE(?, job_draft_skills), job_draft_description = COALESCE(?, job_draft_description) WHERE id = ?")
        .bind(title)
        .bind(budget)
        .bind(skills)
        .bind(description)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn clear_user_job_draft(pool: &SqlitePool, id: i64) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET job_draft_title = NULL, job_draft_budget = NULL, job_draft_skills = NULL WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_user_fields(pool: &SqlitePool, id: i64, nickname: Option<String>, avatar: Option<String>, description: Option<String>, skills: Option<String>) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET nickname = COALESCE(?, nickname), avatar = COALESCE(?, avatar), description = COALESCE(?, description), skills = COALESCE(?, skills), stage = 0 WHERE id = ?")
        .bind(nickname)
        .bind(avatar)
        .bind(description)
        .bind(skills)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn save_job(pool: &SqlitePool, mut job: Job) -> anyhow::Result<Job> {
    // clone fields to avoid partial move of `job`
    let mut j = job.clone();
    let _ = sqlx::query("INSERT INTO jobs (title, budget, skills, description, creator_id) VALUES (?, ?, ?, ?, ?)")
        .bind(&j.title)
        .bind(j.budget)
        .bind(j.skills.clone())
        .bind(j.description.clone())
        .bind(j.creator_id)
        .execute(pool)
        .await?;
    tracing::info!("repo: inserted job title='{}' creator={} budget={} skills={:?} description={:?}", job.title, job.creator_id, job.budget, job.skills, job.description);

    let id: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
        .fetch_one(pool)
        .await?;
    j.id = id;
    Ok(j)
}

pub async fn get_latest_job_by_creator(pool: &SqlitePool, creator_id: i64) -> anyhow::Result<Option<Job>> {
    let row = sqlx::query_as::<_, Job>(
        "SELECT id, title, budget, skills, description, creator_id FROM jobs WHERE creator_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn get_all_jobs(pool: &SqlitePool) -> anyhow::Result<Vec<Job>> {
    let rows = sqlx::query_as::<_, Job>("SELECT id, title, budget, skills, description, creator_id FROM jobs ORDER BY id DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}
