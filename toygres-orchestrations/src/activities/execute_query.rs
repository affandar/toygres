//! Execute SQL query activity
//!
//! Runs an ad-hoc SQL query against a PostgreSQL instance.
//! Reuses the connection pool from test_connection when k8s_name is provided.

use duroxide::ActivityContext;
use crate::activity_types::{ExecuteQueryInput, ExecuteQueryOutput};
use tokio_postgres::NoTls;
use std::time::Duration;

pub const NAME: &str = "toygres-orchestrations::activity::execute-query";

pub async fn activity(
    ctx: ActivityContext,
    input: ExecuteQueryInput,
) -> Result<ExecuteQueryOutput, String> {
    ctx.trace_info(format!("Executing query on instance {:?}", input.k8s_name));

    // Connect with timeout
    let (client, connection) = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_postgres::connect(&input.connection_string, NoTls),
    )
    .await
    .map_err(|_| "Connection timed out after 10s".to_string())?
    .map_err(|e| format!("Failed to connect: {}", e))?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("PostgreSQL connection error: {}", e);
        }
    });

    // Set statement timeout for safety (10 seconds)
    client.execute("SET statement_timeout = '10s'", &[])
        .await
        .map_err(|e| format!("Failed to set statement timeout: {}", e))?;

    // Execute the query
    match client.query(&input.query, &[]).await {
        Ok(rows) => {
            // Extract column names
            let columns: Vec<String> = if let Some(first_row) = rows.first() {
                first_row.columns().iter().map(|c| c.name().to_string()).collect()
            } else {
                Vec::new()
            };

            // Extract rows as string values
            let result_rows: Vec<Vec<Option<String>>> = rows.iter().take(100).map(|row| {
                (0..row.columns().len()).map(|i| {
                    // Try common types, fall back to Debug format
                    if let Ok(v) = row.try_get::<_, String>(i) {
                        Some(v)
                    } else if let Ok(v) = row.try_get::<_, i64>(i) {
                        Some(v.to_string())
                    } else if let Ok(v) = row.try_get::<_, i32>(i) {
                        Some(v.to_string())
                    } else if let Ok(v) = row.try_get::<_, f64>(i) {
                        Some(v.to_string())
                    } else if let Ok(v) = row.try_get::<_, bool>(i) {
                        Some(v.to_string())
                    } else if let Ok(v) = row.try_get::<_, Option<String>>(i) {
                        v
                    } else {
                        Some("(unsupported type)".to_string())
                    }
                }).collect()
            }).collect();

            let row_count = result_rows.len();
            ctx.trace_info(format!("Query returned {} rows, {} columns", row_count, columns.len()));

            Ok(ExecuteQueryOutput {
                columns,
                rows: result_rows,
                row_count,
                success: true,
                error: None,
            })
        }
        Err(e) => {
            let error_msg = format!("Query failed: {}", e);
            ctx.trace_warn(&error_msg);
            Ok(ExecuteQueryOutput {
                columns: Vec::new(),
                rows: Vec::new(),
                row_count: 0,
                success: false,
                error: Some(error_msg),
            })
        }
    }
}
