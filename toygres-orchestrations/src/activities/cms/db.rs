use once_cell::sync::OnceCell;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

static POOL: OnceCell<PgPool> = OnceCell::new();

pub(crate) async fn get_pool() -> Result<PgPool, String> {
    if let Some(pool) = POOL.get() {
        return Ok(pool.clone());
    }

    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL not set".to_string())?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&db_url)
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;

    POOL.set(pool.clone())
        .map_err(|_| "Failed to initialize global database pool".to_string())?;

    Ok(pool)
}

