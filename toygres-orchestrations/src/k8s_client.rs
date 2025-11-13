//! Shared Kubernetes client utilities

use anyhow::{Context, Result};
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::{Node, PersistentVolumeClaim, Service};
use kube::{api::Api, Client};

/// Get a Kubernetes client
pub async fn get_k8s_client() -> Result<Client> {
    Client::try_default()
        .await
        .context("Failed to create Kubernetes client")
}

/// Check if PostgreSQL resources exist for an instance
pub async fn check_resources_exist(
    client: &Client,
    namespace: &str,
    instance_name: &str,
) -> Result<bool> {
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    
    match statefulsets.get(instance_name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(response)) if response.code == 404 => Ok(false),
        Err(e) => Err(anyhow::anyhow!("Failed to check StatefulSet: {}", e)),
    }
}

/// Get Azure region from node labels
pub async fn get_azure_region(client: &Client) -> Result<String> {
    let nodes: Api<Node> = Api::all(client.clone());
    let node_list = nodes.list(&kube::api::ListParams::default().limit(1)).await?;
    
    if let Some(node) = node_list.items.first() {
        if let Some(labels) = &node.metadata.labels {
            // Azure AKS nodes have region in labels
            if let Some(region) = labels.get("topology.kubernetes.io/region") {
                return Ok(region.clone());
            }
            // Fallback to older label
            if let Some(region) = labels.get("failure-domain.beta.kubernetes.io/region") {
                return Ok(region.clone());
            }
        }
    }
    
    anyhow::bail!("Could not determine Azure region from node labels")
}

/// Check if a service exists
pub async fn service_exists(
    client: &Client,
    namespace: &str,
    service_name: &str,
) -> Result<bool> {
    let services: Api<Service> = Api::namespaced(client.clone(), namespace);
    
    match services.get(service_name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(response)) if response.code == 404 => Ok(false),
        Err(e) => Err(anyhow::anyhow!("Failed to check Service: {}", e)),
    }
}

/// Check if a PVC exists
pub async fn pvc_exists(
    client: &Client,
    namespace: &str,
    pvc_name: &str,
) -> Result<bool> {
    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), namespace);
    
    match pvcs.get(pvc_name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(response)) if response.code == 404 => Ok(false),
        Err(e) => Err(anyhow::anyhow!("Failed to check PVC: {}", e)),
    }
}

