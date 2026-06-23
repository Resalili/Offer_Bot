use sqlx::SqlitePool;

pub async fn init_db(database_url: &str) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePool::connect(database_url).await?;

    // Create tables if not exist
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            nickname TEXT,
            avatar TEXT,
            description TEXT,
            skills TEXT,
            stage INTEGER DEFAULT 0,
            job_stage INTEGER DEFAULT 0,
            job_draft_title TEXT,
            job_draft_budget INTEGER,
            job_draft_skills TEXT,
            job_draft_description TEXT
        );"#,
    )
    .execute(&pool)
    .await?;

    // Ensure added columns exist for existing DB files (idempotent)
    use sqlx::Row;
    let rows = sqlx::query("PRAGMA table_info(users)").fetch_all(&pool).await?;
    let mut cols = Vec::new();
    for r in rows.into_iter() {
        if let Ok(name) = r.try_get::<String, _>("name") {
            cols.push(name);
        }
    }

    if !cols.contains(&"job_stage".to_string()) {
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN job_stage INTEGER DEFAULT 0").execute(&pool).await;
    }
    if !cols.contains(&"job_draft_title".to_string()) {
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN job_draft_title TEXT").execute(&pool).await;
    }
    if !cols.contains(&"job_draft_budget".to_string()) {
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN job_draft_budget INTEGER").execute(&pool).await;
    }
    if !cols.contains(&"job_draft_skills".to_string()) {
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN job_draft_skills TEXT").execute(&pool).await;
    }
    if !cols.contains(&"job_draft_description".to_string()) {
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN job_draft_description TEXT").execute(&pool).await;
    }
    if !cols.contains(&"description".to_string()) {
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN description TEXT").execute(&pool).await;
    }

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS jobs (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            budget INTEGER,
            skills TEXT,
            description TEXT,
            creator_id INTEGER
        );"#,
    )
    .execute(&pool)
    .await?;

    // Ensure existing DBs have a `skills` column in `jobs` table
    let job_rows = sqlx::query("PRAGMA table_info(jobs)").fetch_all(&pool).await?;
    let mut job_cols = Vec::new();
    for r in job_rows.into_iter() {
        if let Ok(name) = r.try_get::<String, _>("name") {
            job_cols.push(name);
        }
    }

    if !job_cols.contains(&"skills".to_string()) {
        // Add the column if it's missing (idempotent for older DBs)
        let _ = sqlx::query("ALTER TABLE jobs ADD COLUMN skills TEXT").execute(&pool).await;
    }
    if !job_cols.contains(&"description".to_string()) {
        let _ = sqlx::query("ALTER TABLE jobs ADD COLUMN description TEXT").execute(&pool).await;
    }

    Ok(pool)
}
