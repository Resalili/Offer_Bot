// Redis stub for starter-repo. Integrate an async redis client later.
use redis::Client;
use sqlx::encode::IsNull::No;


/*pub async fn connect(_url: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::open(_url)?;
    let mut con = client.get_async_connection().await?;
    Ok(None)
}*/

