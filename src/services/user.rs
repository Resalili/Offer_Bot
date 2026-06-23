use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: i64,
    pub name: String,
}

pub async fn create_user(_u: User) -> anyhow::Result<User> {
    // placeholder: persist to DB
    Ok(_u)
}
