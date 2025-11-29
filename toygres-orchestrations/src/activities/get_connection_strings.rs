//! Get connection strings activity

use duroxide::ActivityContext;
use crate::activity_types::{GetConnectionStringsInput, GetConnectionStringsOutput};
use crate::k8s_client::{get_k8s_client, get_azure_region};
use k8s_openapi::api::core::v1::Service;
use kube::api::Api;
use std::time::Duration;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::get-connection-strings";

pub async fn activity(
    ctx: ActivityContext,
    input: GetConnectionStringsInput,
) -> Result<GetConnectionStringsOutput, String> {
    ctx.trace_info(format!("Getting connection strings for: {}", input.instance_name));
    
    // 2. Get K8s client
    let client = get_k8s_client().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;
    
    // 3. Build connection strings
    let (ip_conn, dns_conn, external_ip, dns_name) = build_connection_strings(&client, &input, &ctx).await
        .map_err(|e| format!("Failed to build connection strings: {}", e))?;
    
    ctx.trace_info("Connection strings generated");
    
    // 4. Return output
    Ok(GetConnectionStringsOutput {
        ip_connection_string: ip_conn,
        dns_connection_string: dns_conn,
        external_ip,
        dns_name,
    })
}

async fn build_connection_strings(
    client: &kube::Client,
    input: &GetConnectionStringsInput,
    ctx: &ActivityContext,
) -> anyhow::Result<(String, Option<String>, Option<String>, Option<String>)> {
    let service_name = format!("{}-svc", input.instance_name);
    let username = "postgres";
    let database = "postgres";
    let port = 5432;
    
    if input.use_load_balancer {
        // Wait for LoadBalancer to get an external IP
        ctx.trace_info("Waiting for LoadBalancer external IP");
        let services: Api<Service> = Api::namespaced(client.clone(), &input.namespace);
        
        let mut external_ip: Option<String> = None;
        
        for attempt in 1..=10 {
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
            
            if attempt < 30 {
                ctx.trace_info(format!("Waiting for LoadBalancer IP (attempt {}/60)...", attempt));
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
        
        let ip = external_ip.ok_or_else(|| anyhow::anyhow!("Timeout waiting for LoadBalancer external IP"))?;
        
        // Build IP connection string
        let ip_connection_string = format!(
            "postgresql://{}:{}@{}:{}/{}",
            username, input.password, ip, port, database
        );
        
        // Build DNS connection string if DNS label provided
        let (dns_connection_string, dns_name) = if let Some(label) = &input.dns_label {
            match get_azure_region(client).await {
                Ok(region) => {
                    let dns = format!("{}.{}.cloudapp.azure.com", label, region);
                    ctx.trace_info(format!("Azure DNS name: {}", dns));
                    let conn = format!(
                        "postgresql://{}:{}@{}:{}/{}",
                        username, input.password, dns, port, database
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
        
        Ok((ip_connection_string, dns_connection_string, Some(ip), dns_name))
    } else {
        // Use cluster-internal DNS name
        let internal_host = format!("{}.{}.svc.cluster.local", service_name, input.namespace);
        let connection_string = format!(
            "postgresql://{}:{}@{}:{}/{}",
            username, input.password, internal_host, port, database
        );
        Ok((connection_string, None, None, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_connection_strings_input_serialization() {
        let input = GetConnectionStringsInput {
            namespace: "test".to_string(),
            instance_name: "test-pg".to_string(),
            password: "password123".to_string(),
            use_load_balancer: true,
            dns_label: Some("testlabel".to_string()),
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
            external_ip: Some("1.2.3.4".to_string()),
            dns_name: Some("test.eastus.cloudapp.azure.com".to_string()),
        };
        
        let json = serde_json::to_string(&output).unwrap();
        let parsed: GetConnectionStringsOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }
}

