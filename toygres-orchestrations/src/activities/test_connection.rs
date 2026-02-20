//! Test PostgreSQL connection activity
//!
//! v2: Supports worker-local connection pooling via session affinity.
//! When k8s_name is provided, the activity maintains a persistent connection
//! in a worker-global pool. Subsequent calls with the same k8s_name reuse
//! the existing connection instead of opening a new one.

use duroxide::ActivityContext;
use crate::activity_types::{TestConnectionInput, TestConnectionOutput};
use tokio_postgres::{Client, NoTls};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::test-connection";

/// Worker-global connection pool: k8s_name → (Client, connection_string)
/// The connection_string is stored to detect when it changes (e.g., DNS update).
static CONN_POOL: std::sync::LazyLock<Mutex<HashMap<String, (Client, String)>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn activity(
    ctx: ActivityContext,
    input: TestConnectionInput,
) -> Result<TestConnectionOutput, String> {
    // Inject failure for testing (via environment variable)
    if std::env::var("TOYGRES_INJECT_TEST_CONNECTION_FAILURE").is_ok() {
        ctx.trace_error("INJECTED FAILURE: Test connection forced to fail for rollback testing");
        return Err("INJECTED FAILURE: Connection test failed (for testing rollback)".to_string());
    }

    let pool_key = input.k8s_name.clone();

    // Try cached connection if we have a pool key
    if let Some(key) = &pool_key {
        if let Some((client, cached_conn_str)) = take_from_pool(key) {
            // Check connection string hasn't changed
            if cached_conn_str == input.connection_string {
                match client.query_one("SELECT 1", &[]).await {
                    Ok(_) => {
                        let version = query_version(&client, &ctx).await?;
                        put_to_pool(key, client, cached_conn_str);
                        return Ok(TestConnectionOutput { version, connected: true });
                    }
                    Err(_) => {
                        ctx.trace_info("Pooled connection broken, reconnecting");
                    }
                }
            } else {
                ctx.trace_info("Connection string changed, dropping cached connection");
            }
        }
    }

    // Cold path: connect with retries (3 attempts, 5s timeout each)
    let mut last_err = String::new();
    for attempt in 0..3u32 {
        match connect_with_timeout(&input.connection_string, Duration::from_secs(5)).await {
            Ok(client) => {
                ctx.trace_info("Connected to PostgreSQL, querying version");
                let version = query_version(&client, &ctx).await?;
                if let Some(key) = &pool_key {
                    put_to_pool(key, client, input.connection_string.clone());
                }
                return Ok(TestConnectionOutput { version, connected: true });
            }
            Err(e) => {
                last_err = e.to_string();
                if attempt < 2 {
                    let delay = Duration::from_secs(2u64.pow(attempt));
                    ctx.trace_info(format!("Connection attempt {} failed ({}), retrying in {:?}", attempt + 1, last_err, delay));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(format!("Failed to connect to PostgreSQL after 3 attempts: {}", last_err))
}

fn take_from_pool(key: &str) -> Option<(Client, String)> {
    CONN_POOL.lock().ok()?.remove(key)
}

fn put_to_pool(key: &str, client: Client, conn_str: String) {
    if let Ok(mut pool) = CONN_POOL.lock() {
        pool.insert(key.to_string(), (client, conn_str));
    }
}

async fn connect_with_timeout(
    connection_string: &str,
    timeout: Duration,
) -> Result<Client, String> {
    let result = tokio::time::timeout(
        timeout,
        tokio_postgres::connect(connection_string, NoTls),
    )
    .await
    .map_err(|_| format!("Connection timed out after {}s", timeout.as_secs()))?
    .map_err(|e| format!("Failed to connect: {}", e))?;

    let (client, connection) = result;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("PostgreSQL connection error: {}", e);
        }
    });

    Ok(client)
}

async fn query_version(client: &Client, ctx: &ActivityContext) -> Result<String, String> {
    let row = client
        .query_one("SELECT version()", &[])
        .await
        .map_err(|e| format!("Failed to query version: {}", e))?;
    let version: String = row.get(0);
    ctx.trace_info(format!("Connected successfully, version: {}", version));
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_test_connection_input_serialization() {
        let input = TestConnectionInput {
            connection_string: "postgresql://postgres:pass@host:5432/db".to_string(),
            k8s_name: Some("myinstance-abc123".to_string()),
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: TestConnectionInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
    }

    #[test]
    fn test_test_connection_input_without_k8s_name() {
        // Backward compatibility: old inputs without k8s_name should deserialize
        let json = r#"{"connection_string":"postgresql://postgres:pass@host:5432/db"}"#;
        let parsed: TestConnectionInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.k8s_name, None);
    }
    
    #[test]
    fn test_test_connection_output_serialization() {
        let output = TestConnectionOutput {
            version: "PostgreSQL 18.0".to_string(),
            connected: true,
        };
        
        let json = serde_json::to_string(&output).unwrap();
        let parsed: TestConnectionOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }
}

