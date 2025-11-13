//! Wait for PostgreSQL pod to be ready activity

use duroxide::ActivityContext;
use crate::activity_types::{WaitForReadyInput, WaitForReadyOutput};
use crate::k8s_client::get_k8s_client;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, ListParams};

pub async fn wait_for_ready_activity(
    ctx: ActivityContext,
    input: WaitForReadyInput,
) -> Result<WaitForReadyOutput, String> {
    ctx.trace_info(format!("Checking pod readiness: {}", input.instance_name));
    
    // 2. Get K8s client
    let client = get_k8s_client().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;
    
    // 3. Check current pod status (no polling, orchestration handles that)
    let (phase, is_ready) = check_pod_ready(&client, &input.namespace, &input.instance_name, &ctx).await
        .map_err(|e| format!("Failed to check pod status: {}", e))?;
    
    ctx.trace_info(format!("Pod phase: {}, ready: {}", phase, is_ready));
    
    // 4. Return output
    Ok(WaitForReadyOutput {
        pod_phase: phase,
        is_ready,
    })
}

async fn check_pod_ready(
    client: &kube::Client,
    namespace: &str,
    instance_name: &str,
    _ctx: &ActivityContext,
) -> anyhow::Result<(String, bool)> {
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let label_selector = format!("instance={}", instance_name);
    
    let pod_list = pods
        .list(&ListParams::default().labels(&label_selector))
        .await?;

    if let Some(pod) = pod_list.items.first() {
        // Check if pod is ready
        if let Some(status) = &pod.status {
            let phase = status.phase.as_ref()
                .map(|p| p.as_str())
                .unwrap_or("Unknown")
                .to_string();
            
            // Check Ready condition
            if let Some(conditions) = &status.conditions {
                for condition in conditions {
                    if condition.type_ == "Ready" && condition.status == "True" {
                        return Ok((phase, true));
                    }
                }
            }
            
            return Ok((phase, false));
        }
    }
    
    // No pod found
    Ok(("NotFound".to_string(), false))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wait_for_ready_input_serialization() {
        let input = WaitForReadyInput {
            namespace: "test".to_string(),
            instance_name: "test-pg".to_string(),
            timeout_seconds: 300,
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: WaitForReadyInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
    }
    
    #[test]
    fn test_wait_for_ready_output_serialization() {
        let output = WaitForReadyOutput {
            pod_phase: "Running".to_string(),
            is_ready: true,
        };
        
        let json = serde_json::to_string(&output).unwrap();
        let parsed: WaitForReadyOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }
}

