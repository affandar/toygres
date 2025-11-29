//! Test PostgreSQL connection activity

use duroxide::ActivityContext;
use crate::activity_types::{TestConnectionInput, TestConnectionOutput};
use tokio_postgres::NoTls;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::test-connection";

pub async fn activity(
    ctx: ActivityContext,
    input: TestConnectionInput,
) -> Result<TestConnectionOutput, String> {
    ctx.trace_info("Testing PostgreSQL connection");
    
    // Inject failure for testing (via environment variable)
    if std::env::var("TOYGRES_INJECT_TEST_CONNECTION_FAILURE").is_ok() {
        ctx.trace_error("INJECTED FAILURE: Test connection forced to fail for rollback testing");
        return Err("INJECTED FAILURE: Connection test failed (for testing rollback)".to_string());
    }
    
    // 2. Connect and query version
    let version = connect_and_query_version(&input.connection_string, &ctx).await
        .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;
    
    ctx.trace_info(format!("Connected successfully, version: {}", version));
    
    // 3. Return output
    Ok(TestConnectionOutput {
        version,
        connected: true,
    })
}

async fn connect_and_query_version(
    connection_string: &str,
    ctx: &ActivityContext,
) -> anyhow::Result<String> {
    // Parse connection string and connect
    let (client, connection) = tokio_postgres::connect(connection_string, NoTls)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect: {}", e))?;
    
    // Spawn connection handler
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("PostgreSQL connection error: {}", e);
        }
    });
    
    ctx.trace_info("Connected to PostgreSQL, querying version");
    
    // Query version
    let row = client
        .query_one("SELECT version()", &[])
        .await
        .map_err(|e| anyhow::anyhow!("Failed to query version: {}", e))?;
    
    let version: String = row.get(0);
    
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_test_connection_input_serialization() {
        let input = TestConnectionInput {
            connection_string: "postgresql://postgres:pass@host:5432/db".to_string(),
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: TestConnectionInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
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

