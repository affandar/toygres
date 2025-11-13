//! Delete PostgreSQL activity

use duroxide::ActivityContext;
use crate::activity_types::{DeletePostgresInput, DeletePostgresOutput};
use crate::k8s_client::{get_k8s_client, check_resources_exist};
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Service};
use kube::api::{Api, DeleteParams};

pub async fn delete_postgres_activity(
    ctx: ActivityContext,
    input: DeletePostgresInput,
) -> Result<DeletePostgresOutput, String> {
    ctx.trace_info(format!("Deleting PostgreSQL: {}", input.instance_name));
    
    // 2. Get K8s client
    let client = get_k8s_client().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;
    
    // 3. Check idempotency - do resources exist?
    let exists = check_resources_exist(&client, &input.namespace, &input.instance_name).await
        .map_err(|e| format!("Failed to check if resources exist: {}", e))?;
    
    if !exists {
        ctx.trace_info("Resources don't exist, nothing to delete");
        return Ok(DeletePostgresOutput { deleted: false });
    }
    
    // 4. Delete resources in order: Service -> StatefulSet -> PVC
    delete_k8s_resources(&client, &input, &ctx).await
        .map_err(|e| format!("Failed to delete K8s resources: {}", e))?;
    
    ctx.trace_info("PostgreSQL deletion complete");
    
    // 5. Return output
    Ok(DeletePostgresOutput { deleted: true })
}

async fn delete_k8s_resources(
    client: &kube::Client,
    input: &DeletePostgresInput,
    ctx: &ActivityContext,
) -> anyhow::Result<()> {
    let delete_params = DeleteParams::default();
    
    // Delete Service
    ctx.trace_info("Deleting Service");
    let services: Api<Service> = Api::namespaced(client.clone(), &input.namespace);
    let service_name = format!("{}-svc", input.instance_name);
    match services.delete(&service_name, &delete_params).await {
        Ok(_) => ctx.trace_info("Service deleted"),
        Err(kube::Error::Api(response)) if response.code == 404 => {
            ctx.trace_info("Service not found, skipping");
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to delete Service: {}", e)),
    }
    
    // Delete StatefulSet
    ctx.trace_info("Deleting StatefulSet");
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), &input.namespace);
    match statefulsets.delete(&input.instance_name, &delete_params).await {
        Ok(_) => ctx.trace_info("StatefulSet deleted"),
        Err(kube::Error::Api(response)) if response.code == 404 => {
            ctx.trace_info("StatefulSet not found, skipping");
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to delete StatefulSet: {}", e)),
    }
    
    // Wait a bit for pods to terminate
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    
    // Delete PVC
    ctx.trace_info("Deleting PersistentVolumeClaim");
    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), &input.namespace);
    let pvc_name = format!("{}-pvc", input.instance_name);
    match pvcs.delete(&pvc_name, &delete_params).await {
        Ok(_) => ctx.trace_info("PersistentVolumeClaim deleted"),
        Err(kube::Error::Api(response)) if response.code == 404 => {
            ctx.trace_info("PersistentVolumeClaim not found, skipping");
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to delete PVC: {}", e)),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_delete_postgres_input_serialization() {
        let input = DeletePostgresInput {
            namespace: "test".to_string(),
            instance_name: "test-pg".to_string(),
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: DeletePostgresInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
    }
    
    #[test]
    fn test_delete_postgres_output_serialization() {
        let output = DeletePostgresOutput {
            deleted: true,
        };
        
        let json = serde_json::to_string(&output).unwrap();
        let parsed: DeletePostgresOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }
}

