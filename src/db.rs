use eyre::Result;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::env;

pub async fn connect_db() -> Result<PgPool> {
    let url = env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;
    Ok(pool)
}
