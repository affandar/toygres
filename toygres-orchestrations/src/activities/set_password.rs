//! Set PostgreSQL password activity
//!
//! This activity explicitly sets the postgres user password after the container
//! is running. This is necessary because:
//! 1. POSTGRES_PASSWORD env var only works on initial database creation
//! 2. If PVC already has data, the password is not changed
//! 3. Some images (like pg_durable) may have their own init that affects password

use duroxide::ActivityContext;
use crate::activity_types::{SetPasswordInput, SetPasswordOutput};
use tokio_postgres::NoTls;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::set-password";

pub async fn activity(
    ctx: ActivityContext,
    input: SetPasswordInput,
) -> Result<SetPasswordOutput, String> {
    ctx.trace_info(format!(
        "Setting password for instance {} in namespace {}",
        input.instance_name, input.namespace
    ));

    // Connect to PostgreSQL using the connection string
    // Mask password in logs for security
    let masked_conn_string = mask_password(&input.internal_connection_string);
    ctx.trace_info(format!("Connecting to PostgreSQL: {}", masked_conn_string));
    
    let (client, connection) = tokio_postgres::connect(&input.internal_connection_string, NoTls)
        .await
        .map_err(|e| {
            // Get detailed error info
            let db_error = e.as_db_error().map(|dbe| {
                format!(
                    "severity={}, code={}, message={}, detail={:?}, hint={:?}",
                    dbe.severity(),
                    dbe.code().code(),
                    dbe.message(),
                    dbe.detail(),
                    dbe.hint()
                )
            });
            format!(
                "Failed to connect to PostgreSQL ({}): {} [db_error: {:?}]",
                masked_conn_string, e, db_error
            )
        })?;
    
    // Spawn connection handler
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("PostgreSQL connection error: {}", e);
        }
    });
    
    ctx.trace_info("Connected to PostgreSQL, executing ALTER USER");
    
    // Build the ALTER USER command
    // We use single quotes for the password and escape any single quotes in it
    let escaped_password = input.new_password.replace('\'', "''");
    let sql_command = format!("ALTER USER postgres WITH PASSWORD '{}'", escaped_password);
    
    // Execute the ALTER USER command
    client
        .execute(&sql_command, &[])
        .await
        .map_err(|e| format!("Failed to execute ALTER USER: {}", e))?;
    
    ctx.trace_info("Password set successfully");
    
    Ok(SetPasswordOutput { success: true })
}

/// Mask password in connection string for safe logging
fn mask_password(conn_str: &str) -> String {
    // Connection string format: postgresql://user:password@host:port/db
    if let Some(at_pos) = conn_str.find('@') {
        if let Some(colon_pos) = conn_str[..at_pos].rfind(':') {
            let prefix = &conn_str[..colon_pos + 1];
            let suffix = &conn_str[at_pos..];
            return format!("{}****{}", prefix, suffix);
        }
    }
    conn_str.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_password_input_serialization() {
        let input = SetPasswordInput {
            namespace: "toygres".to_string(),
            instance_name: "test-instance".to_string(),
            new_password: "secure_password123!".to_string(),
            internal_connection_string: "postgresql://postgres:postgres@test-instance-svc.toygres.svc.cluster.local:5432/postgres".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: SetPasswordInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
    }

    #[test]
    fn test_set_password_output_serialization() {
        let output = SetPasswordOutput { success: true };

        let json = serde_json::to_string(&output).unwrap();
        let parsed: SetPasswordOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }

    #[test]
    fn test_password_escaping() {
        // Test that passwords with single quotes are properly escaped
        let password = "test'password";
        let escaped = password.replace('\'', "''");
        assert_eq!(escaped, "test''password");
    }
}
