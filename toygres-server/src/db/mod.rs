use anyhow::{Context, Result};

/// Initialize the CMS schema in the database
pub async fn initialize_cms_schema(db_url: &str) -> Result<()> {
    use sqlx::postgres::PgPoolOptions;
    
    // Connect to database
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(db_url)
        .await
        .context("Failed to connect to database for CMS schema initialization")?;
    
    // Create schema if it doesn't exist
    sqlx::query("CREATE SCHEMA IF NOT EXISTS toygres_cms")
        .execute(&pool)
        .await
        .context("Failed to create toygres_cms schema")?;
    
    tracing::info!("✓ CMS schema ready");
    
    Ok(())
}

/// Verify that CMS tables exist
pub async fn verify_cms_tables(db_url: &str) -> Result<()> {
    use sqlx::postgres::PgPoolOptions;
    
    // Connect to database
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(db_url)
        .await
        .context("Failed to connect to database for CMS table verification")?;
    
    // Check if the instances table exists
    let result: Option<(bool,)> = sqlx::query_as(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_schema = 'toygres_cms' 
            AND table_name = 'instances'
        )"
    )
    .fetch_optional(&pool)
    .await
    .context("Failed to check if CMS tables exist")?;
    
    match result {
        Some((true,)) => {
            tracing::info!("✓ CMS tables verified");
            Ok(())
        }
        _ => {
            anyhow::bail!(
                "CMS tables not found. Please run: ./scripts/db-init.sh\n\
                 This will create the required tables in the toygres_cms schema."
            )
        }
    }
}

/// Look up Kubernetes name by user-provided DNS name
pub async fn lookup_k8s_name_by_user_name(db_url: &str, dns_name: &str) -> Result<String> {
    use sqlx::postgres::PgPoolOptions;
    
    // Connect to database
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(db_url)
        .await
        .context("Failed to connect to database for instance lookup")?;
    
    // Look up the k8s_name by dns_name
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT k8s_name FROM toygres_cms.instances 
         WHERE dns_name = $1 
         AND state != 'deleted'
         ORDER BY created_at DESC 
         LIMIT 1"
    )
    .bind(dns_name)
    .fetch_optional(&pool)
    .await
    .context("Failed to look up instance by DNS name")?;
    
    match result {
        Some((k8s_name,)) => Ok(k8s_name),
        None => anyhow::bail!(
            "Instance with DNS name '{}' not found in database. \n\
             Note: Use the DNS name you provided during creation (e.g., 'adardb5'), not the K8s name with GUID suffix.",
            dns_name
        ),
    }
}

