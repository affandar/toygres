use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::sync::OnceCell;

static POOL: OnceCell<PgPool> = OnceCell::const_new();

pub(crate) async fn get_pool() -> Result<PgPool, String> {
    // Use get_or_try_init to safely handle concurrent initialization
    let pool = POOL
        .get_or_try_init(|| async {
            let db_url = std::env::var("DATABASE_URL")
                .map_err(|_| "DATABASE_URL not set".to_string())?;

            PgPoolOptions::new()
                .max_connections(10)
                .connect(&db_url)
                .await
                .map_err(|e| format!("Failed to connect to database: {}", e))
        })
        .await?;

    Ok(pool.clone())
}

