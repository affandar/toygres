//! Get connection strings activity

use duroxide::ActivityContext;
use crate::activity_types::{GetConnectionStringsInput, GetConnectionStringsOutput};
use crate::k8s_client::{get_k8s_client, get_azure_region};
use k8s_openapi::api::core::v1::Service;
use kube::api::Api;
use std::time::Duration;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::get-connection-strings";

/// URL encode a password for use in connection strings
fn url_encode_password(password: &str) -> String {
    let mut encoded = String::with_capacity(password.len() * 3);
    for c in password.chars() {
        match c {
            // Safe characters (alphanumeric and a few others)
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                encoded.push(c);
            }
            // Everything else needs encoding
            _ => {
                for byte in c.to_string().as_bytes() {
                    encoded.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    encoded
}

pub async fn activity(
    ctx: ActivityContext,
    input: GetConnectionStringsInput,
) -> Result<GetConnectionStringsOutput, String> {
    ctx.trace_info(format!("Getting connection strings for: {}", input.instance_name));
    
    // 2. Get K8s client
    let client = get_k8s_client().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;
    
    // 3. Build connection strings
    let (ip_conn, dns_conn, internal_conn, external_ip, dns_name) = build_connection_strings(&client, &input, &ctx).await
        .map_err(|e| format!("Failed to build connection strings: {}", e))?;
    
    ctx.trace_info("Connection strings generated");
    
    // 4. Return output
    Ok(GetConnectionStringsOutput {
        ip_connection_string: ip_conn,
        dns_connection_string: dns_conn,
        internal_connection_string: internal_conn,
        external_ip,
        dns_name,
    })
}

async fn build_connection_strings(
    client: &kube::Client,
    input: &GetConnectionStringsInput,
    ctx: &ActivityContext,
) -> anyhow::Result<(String, Option<String>, String, Option<String>, Option<String>)> {
    let service_name = format!("{}-svc", input.instance_name);
    let username = "postgres";
    // pg_durable is an extension in the postgres database, not a separate DB
    let database = "postgres";
    let port = 5432;
    
    // URL encode the password for safe use in connection strings
    let encoded_password = url_encode_password(&input.password);
    
    // Always build internal connection string (ClusterIP-based)
    let internal_host = format!("{}.{}.svc.cluster.local", service_name, input.namespace);
    let internal_connection_string = format!(
        "postgresql://{}:{}@{}:{}/{}",
        username, encoded_password, internal_host, port, database
    );
    
    if input.use_load_balancer {
        // Wait for LoadBalancer to get an external IP
        ctx.trace_info("Waiting for LoadBalancer external IP");
        let services: Api<Service> = Api::namespaced(client.clone(), &input.namespace);
        
        let mut external_ip: Option<String> = None;
        
        for attempt in 1..=24 {
            let svc = services.get(&service_name).await?;
            
            if let Some(status) = &svc.status {
                if let Some(load_balancer) = &status.load_balancer {
                    if let Some(ingresses) = &load_balancer.ingress {
                        if let Some(ingress) = ingresses.first() {
                            if let Some(ip) = &ingress.ip {
                                ctx.trace_info(format!("External IP: {}", ip));
                                external_ip = Some(ip.clone());
                                break;
                            }
                        }
                    }
                }
            }
            
            if attempt < 24 {
                ctx.trace_info(format!("Waiting for LoadBalancer IP (attempt {}/24)...", attempt));
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
        
        let ip = external_ip.ok_or_else(|| anyhow::anyhow!("Timeout waiting for LoadBalancer external IP"))?;
        
        // Build IP connection string
        let ip_connection_string = format!(
            "postgresql://{}:{}@{}:{}/{}",
            username, encoded_password, ip, port, database
        );
        
        // Build DNS connection string if DNS label provided
        let (dns_connection_string, dns_name) = if let Some(label) = &input.dns_label {
            match get_azure_region(client).await {
                Ok(region) => {
                    let dns = format!("{}.{}.cloudapp.azure.com", label, region);
                    ctx.trace_info(format!("Azure DNS name: {}", dns));
                    let conn = format!(
                        "postgresql://{}:{}@{}:{}/{}",
                        username, encoded_password, dns, port, database
                    );
                    (Some(conn), Some(dns))
                }
                Err(_) => {
                    ctx.trace_warn("Could not determine Azure region, DNS name not available");
                    (None, None)
                }
            }
        } else {
            (None, None)
        };
        
        Ok((ip_connection_string, dns_connection_string, internal_connection_string, Some(ip), dns_name))
    } else {
        // Use cluster-internal DNS name (no LoadBalancer)
        // In this case, internal and external are the same
        Ok((internal_connection_string.clone(), None, internal_connection_string, None, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity_types::ImageType;
    
    #[test]
    fn test_get_connection_strings_input_serialization() {
        let input = GetConnectionStringsInput {
            namespace: "test".to_string(),
            instance_name: "test-pg".to_string(),
            password: "password123".to_string(),
            use_load_balancer: true,
            dns_label: Some("testlabel".to_string()),
            image_type: ImageType::Stock,
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: GetConnectionStringsInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
    }
    
    #[test]
    fn test_get_connection_strings_output_serialization() {
        let output = GetConnectionStringsOutput {
            ip_connection_string: "postgresql://postgres:pass@1.2.3.4:5432/postgres".to_string(),
            dns_connection_string: Some("postgresql://postgres:pass@test.eastus.cloudapp.azure.com:5432/postgres".to_string()),
            internal_connection_string: "postgresql://postgres:pass@test-svc.toygres.svc.cluster.local:5432/postgres".to_string(),
            external_ip: Some("1.2.3.4".to_string()),
            dns_name: Some("test.eastus.cloudapp.azure.com".to_string()),
        };
        
        let json = serde_json::to_string(&output).unwrap();
        let parsed: GetConnectionStringsOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }
}

