use anyhow::{Context, Result};
use clap::Parser;
use dotenvy;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::{Namespace, Node, Pod, PersistentVolumeClaim, Service};
use k8s_openapi::api::storage::v1::StorageClass;
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams},
    Client,
};
use std::time::Duration;
use tera::{Tera, Context as TeraContext};
use tokio::time::sleep;
use tokio_postgres::NoTls;
use tracing::{info, warn};
use uuid::Uuid;

/// PostgreSQL Deployment Tool for Toygres
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Clean up resources after testing
    #[arg(long)]
    clean: bool,
    
    /// DNS name for the instance (used with DNS_LABEL to create <name>-<DNS_LABEL>)
    #[arg(long)]
    dns_name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();
    
    // Parse command line arguments
    let args = Args::parse();
    
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "manual_deploy=info".to_string()),
        )
        .init();

    info!("=== PostgreSQL Deployment Test ===");
    info!("");

    // Configuration
    let namespace = std::env::var("AKS_NAMESPACE").unwrap_or_else(|_| "toygres".to_string());
    let instance_name = std::env::var("INSTANCE_NAME").unwrap_or_else(|_| "test-pg-1".to_string());
    let postgres_password = std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "testpass123".to_string());
    // Default to LoadBalancer (public IP) unless explicitly disabled
    let use_load_balancer = std::env::var("USE_LOAD_BALANCER").unwrap_or_else(|_| "true".to_string()) == "true";
    
    // DNS label construction
    let dns_label_base = std::env::var("DNS_LABEL").ok();
    let final_dns_label = if let Some(base) = &dns_label_base {
        // DNS_LABEL is set, construct the full label
        let user_part = if let Some(user_name) = &args.dns_name {
            // User provided a name via --dns-name
            user_name.clone()
        } else {
            // Generate a GUID-based name
            let guid = uuid::Uuid::new_v4().to_string();
            let short_guid = &guid[..8]; // First 8 chars
            format!("toygres-instance-{}", short_guid)
        };
        Some(format!("{}-{}", user_part, base))
    } else {
        None
    };

    info!("Configuration:");
    info!("  Namespace: {}", namespace);
    info!("  Instance name: {}", instance_name);
    info!("  Password: ****");
    info!("  Service type: {}", if use_load_balancer { "LoadBalancer (public IP)" } else { "ClusterIP (cluster-internal)" });
    if let Some(label) = &final_dns_label {
        info!("  DNS label: {} (will create {}.{}.cloudapp.azure.com)", label, label, "<region>");
    }
    info!("  Cleanup after test: {}", if args.clean { "yes" } else { "no" });
    info!("");

    // Connect to Kubernetes cluster
    info!("Connecting to Kubernetes cluster...");
    let client = Client::try_default()
        .await
        .context("Failed to create Kubernetes client. Make sure kubectl is configured.")?;
    
    info!("✓ Connected to Kubernetes cluster");
    info!("");

    // Deploy PostgreSQL
    let (ip_connection_string, dns_connection_string) = match deploy_and_validate(&client, &namespace, &instance_name, &postgres_password, use_load_balancer, final_dns_label.as_deref()).await {
        Ok((ip_conn, dns_conn)) => {
            info!("");
            info!("=== Deployment Complete ===");
            info!("✓ PostgreSQL instance is running in AKS");
            info!("✓ Ready to accept connections");
            (ip_conn, dns_conn)
        }
        Err(e) => {
            warn!("Deployment test failed: {}", e);
            info!("Attempting cleanup...");
            let _ = cleanup_postgres_instance(&client, &namespace, &instance_name).await;
            return Err(e);
        }
    };
    
    // Test actual PostgreSQL connection
    info!("");
    info!("=== Testing PostgreSQL Connection ===");
    // Try DNS first if available, fallback to IP
    let test_connection_string = dns_connection_string.as_ref().unwrap_or(&ip_connection_string);
    match test_postgres_connection(test_connection_string).await {
        Ok(version) => {
            info!("✓ Successfully connected to PostgreSQL");
            info!("✓ PostgreSQL version: {}", version);
        }
        Err(e) => {
            warn!("Failed to connect to PostgreSQL: {}", e);
            if args.clean {
                info!("Cleaning up...");
                let _ = cleanup_postgres_instance(&client, &namespace, &instance_name).await;
            }
            return Err(e);
        }
    }
    
    // Cleanup if requested
    if args.clean {
        info!("");
        info!("=== Cleaning Up (--clean flag specified) ===");
        cleanup_postgres_instance(&client, &namespace, &instance_name).await?;
        info!("✓ All resources cleaned up");
        info!("");
        info!("=== Test Complete ===");
        info!("✓ Deployment successful");
        info!("✓ Connection verified");
        info!("✓ Cleanup complete");
    } else {
        info!("");
        info!("=== Keeping Resources (use --clean to auto-cleanup) ===");
    }

    Ok(())
}

async fn test_cluster_info(client: &Client) -> Result<()> {
    info!("--- Cluster Information ---");
    
    // List nodes
    info!("Querying cluster nodes...");
    let nodes: Api<Node> = Api::all(client.clone());
    match nodes.list(&ListParams::default()).await {
        Ok(node_list) => {
            info!("  ✓ Found {} nodes", node_list.items.len());
            for node in node_list.items.iter().take(3) {
                if let Some(name) = &node.metadata.name {
                    let status = node.status.as_ref()
                        .and_then(|s| s.node_info.as_ref())
                        .map(|ni| format!("{} ({})", ni.kubelet_version, ni.os_image))
                        .unwrap_or_else(|| "Unknown".to_string());
                    info!("    - {}: {}", name, status);
                }
            }
        }
        Err(e) => {
            info!("  ✗ Cannot list nodes: {}", e);
            info!("    (This is OK if not cluster-admin)");
        }
    }

    info!("");
    Ok(())
}

async fn test_namespaces(client: &Client) -> Result<()> {
    info!("--- Namespaces ---");
    
    info!("Listing namespaces...");
    let namespaces: Api<Namespace> = Api::all(client.clone());
    match namespaces.list(&ListParams::default()).await {
        Ok(ns_list) => {
            info!("  ✓ Found {} namespaces", ns_list.items.len());
            let ns_names: Vec<String> = ns_list.items.iter()
                .filter_map(|ns| ns.metadata.name.clone())
                .collect();
            
            // Show first few namespaces
            for name in ns_names.iter().take(5) {
                info!("    - {}", name);
            }
            if ns_names.len() > 5 {
                info!("    ... and {} more", ns_names.len() - 5);
            }
            
            // Check if toygres namespace exists
            if ns_names.contains(&"toygres".to_string()) {
                info!("  ✓ 'toygres' namespace exists");
            } else {
                info!("  ℹ 'toygres' namespace does not exist (will need to create it)");
            }
        }
        Err(e) => {
            info!("  ✗ Cannot list namespaces: {}", e);
        }
    }

    info!("");
    Ok(())
}

async fn test_storage_classes(client: &Client) -> Result<()> {
    info!("--- Storage Classes ---");
    
    info!("Querying available storage classes...");
    let storage_classes: Api<StorageClass> = Api::all(client.clone());
    match storage_classes.list(&ListParams::default()).await {
        Ok(sc_list) => {
            info!("  ✓ Found {} storage classes", sc_list.items.len());
            for sc in sc_list.items.iter() {
                if let Some(name) = &sc.metadata.name {
                    let provisioner = &sc.provisioner;
                    let is_default = sc.metadata.annotations.as_ref()
                        .and_then(|a| a.get("storageclass.kubernetes.io/is-default-class"))
                        .map(|v| v == "true")
                        .unwrap_or(false);
                    
                    let default_marker = if is_default { " (default)" } else { "" };
                    info!("    - {}: {}{}", name, provisioner, default_marker);
                }
            }
        }
        Err(e) => {
            info!("  ✗ Cannot list storage classes: {}", e);
        }
    }

    info!("");
    Ok(())
}

async fn test_namespace_resources(client: &Client, namespace: &str) -> Result<()> {
    info!("--- Resources in namespace '{}' ---", namespace);
    
    // Test if we can access this namespace
    let namespaces: Api<Namespace> = Api::all(client.clone());
    match namespaces.get(namespace).await {
        Ok(_) => {
            info!("  ✓ Namespace '{}' exists and is accessible", namespace);
        }
        Err(e) => {
            info!("  ✗ Cannot access namespace '{}': {}", namespace, e);
            info!("    (Namespace might not exist or insufficient permissions)");
            return Ok(());
        }
    }
    
    // List Pods
    info!("Querying pods...");
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    match pods.list(&ListParams::default()).await {
        Ok(pod_list) => {
            info!("  ✓ Found {} pods", pod_list.items.len());
            for pod in pod_list.items.iter().take(3) {
                if let Some(name) = &pod.metadata.name {
                    let phase = pod.status.as_ref()
                        .and_then(|s| s.phase.as_ref())
                        .map(|p| p.as_str())
                        .unwrap_or("Unknown");
                    info!("    - {}: {}", name, phase);
                }
            }
        }
        Err(e) => {
            info!("  ✗ Cannot list pods: {}", e);
        }
    }
    
    // List Services
    info!("Querying services...");
    let services: Api<Service> = Api::namespaced(client.clone(), namespace);
    match services.list(&ListParams::default()).await {
        Ok(svc_list) => {
            info!("  ✓ Found {} services", svc_list.items.len());
            for svc in svc_list.items.iter().take(3) {
                if let Some(name) = &svc.metadata.name {
                    let svc_type = svc.spec.as_ref()
                        .and_then(|s| s.type_.as_ref())
                        .map(|t| t.as_str())
                        .unwrap_or("Unknown");
                    info!("    - {}: {}", name, svc_type);
                }
            }
        }
        Err(e) => {
            info!("  ✗ Cannot list services: {}", e);
        }
    }
    
    // List StatefulSets
    info!("Querying statefulsets...");
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    match statefulsets.list(&ListParams::default()).await {
        Ok(sts_list) => {
            info!("  ✓ Found {} statefulsets", sts_list.items.len());
            for sts in sts_list.items.iter().take(3) {
                if let Some(name) = &sts.metadata.name {
                    let replicas = sts.spec.as_ref()
                        .and_then(|s| s.replicas)
                        .unwrap_or(0);
                    info!("    - {}: {} replicas", name, replicas);
                }
            }
        }
        Err(e) => {
            info!("  ✗ Cannot list statefulsets: {}", e);
        }
    }
    
    // List PVCs
    info!("Querying persistent volume claims...");
    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), namespace);
    match pvcs.list(&ListParams::default()).await {
        Ok(pvc_list) => {
            info!("  ✓ Found {} PVCs", pvc_list.items.len());
            for pvc in pvc_list.items.iter().take(3) {
                if let Some(name) = &pvc.metadata.name {
                    let phase = pvc.status.as_ref()
                        .and_then(|s| s.phase.as_ref())
                        .map(|p| p.as_str())
                        .unwrap_or("Unknown");
                    info!("    - {}: {}", name, phase);
                }
            }
        }
        Err(e) => {
            info!("  ✗ Cannot list PVCs: {}", e);
        }
    }

    info!("");
    Ok(())
}

async fn deploy_and_validate(
    client: &Client,
    namespace: &str,
    name: &str,
    password: &str,
    use_load_balancer: bool,
    dns_label: Option<&str>,
) -> Result<(String, Option<String>)> {
    info!("--- Deploying PostgreSQL ---");
    
    // Create resources
    create_postgres_instance(client, namespace, name, password, use_load_balancer, dns_label).await?;
    info!("✓ Kubernetes resources created");
    info!("");
    
    // Wait for pod to be ready
    info!("--- Waiting for Pod ---");
    wait_for_pod_ready(client, namespace, name).await?;
    info!("✓ Pod is ready");
    info!("");
    
    // Get connection strings
    info!("--- Generating Connection Strings ---");
    let (ip_connection_string, dns_connection_string) = get_connection_strings(client, namespace, name, password, use_load_balancer, dns_label).await?;
    info!("✓ IP Connection string: {}", ip_connection_string);
    if let Some(dns_conn) = &dns_connection_string {
        info!("✓ DNS Connection string: {}", dns_conn);
    }
    info!("");
    
    // Validate deployment
    info!("--- Validating Deployment ---");
    validate_deployment(client, namespace, name).await?;
    info!("✓ Deployment validated");
    
    Ok((ip_connection_string, dns_connection_string))
}

async fn test_postgres_connection(connection_string: &str) -> Result<String> {
    // Parse connection string
    let (client, connection) = tokio_postgres::connect(connection_string, NoTls)
        .await
        .context("Failed to connect to PostgreSQL")?;
    
    // Spawn connection handler
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("PostgreSQL connection error: {}", e);
        }
    });
    
    // Query version
    let row = client
        .query_one("SELECT version()", &[])
        .await
        .context("Failed to query PostgreSQL version")?;
    
    let version: String = row.get(0);
    
    Ok(version)
}

async fn get_azure_region(client: &Client) -> Result<String> {
    use k8s_openapi::api::core::v1::Node;
    
    let nodes: Api<Node> = Api::all(client.clone());
    let node_list = nodes.list(&ListParams::default().limit(1)).await?;
    
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

async fn create_postgres_instance(
    client: &Client,
    namespace: &str,
    name: &str,
    password: &str,
    use_load_balancer: bool,
    dns_label: Option<&str>,
) -> Result<()> {
    // Initialize template engine
    let mut tera = Tera::default();
    
    // Load templates
    let pvc_template = include_str!("../templates/postgres-pvc.yaml");
    let statefulset_template = include_str!("../templates/postgres-statefulset.yaml");
    let service_template = include_str!("../templates/postgres-service.yaml");
    
    tera.add_raw_template("pvc", pvc_template)?;
    tera.add_raw_template("statefulset", statefulset_template)?;
    tera.add_raw_template("service", service_template)?;
    
    // Prepare template context
    let mut context = TeraContext::new();
    context.insert("name", name);
    context.insert("namespace", namespace);
    context.insert("password", password);
    context.insert("storage_size", &10);
    context.insert("postgres_version", "18");
    context.insert("service_type", if use_load_balancer { "LoadBalancer" } else { "ClusterIP" });
    context.insert("dns_label", &dns_label.unwrap_or(""));
    
    // 1. Create PersistentVolumeClaim
    info!("  Creating PersistentVolumeClaim...");
    let pvc_yaml = tera.render("pvc", &context)?;
    let pvc: PersistentVolumeClaim = serde_yaml::from_str(&pvc_yaml)
        .context("Failed to parse PVC YAML")?;
    
    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), namespace);
    pvcs.create(&PostParams::default(), &pvc)
        .await
        .context("Failed to create PersistentVolumeClaim")?;
    info!("    ✓ PersistentVolumeClaim created");
    
    // 2. Create StatefulSet
    info!("  Creating StatefulSet...");
    let statefulset_yaml = tera.render("statefulset", &context)?;
    let statefulset: StatefulSet = serde_yaml::from_str(&statefulset_yaml)
        .context("Failed to parse StatefulSet YAML")?;
    
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    statefulsets
        .create(&PostParams::default(), &statefulset)
        .await
        .context("Failed to create StatefulSet")?;
    info!("    ✓ StatefulSet created");
    
    // 3. Create Service
    info!("  Creating Service...");
    let service_yaml = tera.render("service", &context)?;
    let service: Service = serde_yaml::from_str(&service_yaml)
        .context("Failed to parse Service YAML")?;
    
    let services: Api<Service> = Api::namespaced(client.clone(), namespace);
    services
        .create(&PostParams::default(), &service)
        .await
        .context("Failed to create Service")?;
    info!("    ✓ Service created");

    Ok(())
}

async fn wait_for_pod_ready(client: &Client, namespace: &str, name: &str) -> Result<()> {
    use k8s_openapi::api::core::v1::Pod;
    use kube::api::ListParams;

    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let label_selector = format!("instance={}", name);

    for attempt in 1..=60 {
        info!("  Checking pod status (attempt {}/60)...", attempt);
        
        let pod_list = pods
            .list(&ListParams::default().labels(&label_selector))
            .await
            .context("Failed to list pods")?;

        if let Some(pod) = pod_list.items.first() {
            if let Some(status) = &pod.status {
                if let Some(conditions) = &status.conditions {
                    for condition in conditions {
                        if condition.type_ == "Ready" && condition.status == "True" {
                            info!("  ✓ Pod is ready!");
                            return Ok(());
                        }
                    }
                }
                
                // Show pod status for debugging
                if let Some(phase) = &status.phase {
                    info!("    Pod phase: {}", phase);
                }
            }
        }

        if attempt < 60 {
            sleep(Duration::from_secs(5)).await;
        }
    }

    anyhow::bail!("Timeout waiting for pod to be ready")
}

async fn get_connection_strings(
    client: &Client,
    namespace: &str,
    name: &str,
    password: &str,
    use_load_balancer: bool,
    dns_label: Option<&str>,
) -> Result<(String, Option<String>)> {
    let service_name = format!("{}-svc", name);
    let username = "postgres";
    let database = "postgres";
    let port = 5432;

    if use_load_balancer {
        // Wait for LoadBalancer to get an external IP
        info!("  Waiting for LoadBalancer to get external IP...");
        let services: Api<Service> = Api::namespaced(client.clone(), namespace);
        
        let mut external_ip: Option<String> = None;
        
        for attempt in 1..=30 {
            let svc = services.get(&service_name).await?;
            
            if let Some(status) = &svc.status {
                if let Some(load_balancer) = &status.load_balancer {
                    if let Some(ingresses) = &load_balancer.ingress {
                        if let Some(ingress) = ingresses.first() {
                            if let Some(ip) = &ingress.ip {
                                info!("  ✓ External IP: {}", ip);
                                external_ip = Some(ip.clone());
                                break;
                            }
                        }
                    }
                }
            }
            
            if attempt < 30 {
                info!("    Waiting for external IP (attempt {}/30)...", attempt);
                sleep(Duration::from_secs(5)).await;
            }
        }
        
        let ip = external_ip.context("Timeout waiting for LoadBalancer external IP")?;
        
        // Build IP connection string
        let ip_connection_string = format!(
            "postgresql://{}:{}@{}:{}/{}",
            username, password, ip, port, database
        );
        
        // Build DNS connection string if DNS label provided
        let dns_connection_string = if let Some(label) = dns_label {
            match get_azure_region(client).await {
                Ok(region) => {
                    let dns_name = format!("{}.{}.cloudapp.azure.com", label, region);
                    info!("  ✓ Azure DNS name: {}", dns_name);
                    info!("    (DNS may take a few minutes to propagate)");
                    Some(format!(
                        "postgresql://{}:{}@{}:{}/{}",
                        username, password, dns_name, port, database
                    ))
                }
                Err(_) => {
                    info!("  ⚠ Could not determine Azure region, DNS name not available");
                    None
                }
            }
        } else {
            None
        };
        
        Ok((ip_connection_string, dns_connection_string))
    } else {
        // Use cluster-internal DNS name
        let internal_host = format!("{}.{}.svc.cluster.local", service_name, namespace);
        let connection_string = format!(
            "postgresql://{}:{}@{}:{}/{}",
            username, password, internal_host, port, database
        );
        Ok((connection_string, None))
    }
}

async fn validate_deployment(client: &Client, namespace: &str, name: &str) -> Result<()> {
    // Verify StatefulSet exists and has correct replica count
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    let sts = statefulsets.get(name).await
        .context("StatefulSet not found")?;
    
    let ready_replicas = sts.status
        .as_ref()
        .and_then(|s| s.ready_replicas)
        .unwrap_or(0);
    
    if ready_replicas < 1 {
        anyhow::bail!("StatefulSet has no ready replicas");
    }
    info!("  StatefulSet: {} ready replicas", ready_replicas);
    
    // Verify Service exists
    let services: Api<Service> = Api::namespaced(client.clone(), namespace);
    let _svc = services.get(&format!("{}-svc", name)).await
        .context("Service not found")?;
    info!("  Service: exists and accessible");
    
    // Verify PVC exists and is bound
    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), namespace);
    let pvc = pvcs.get(&format!("{}-pvc", name)).await
        .context("PVC not found")?;
    
    let pvc_phase = pvc.status
        .as_ref()
        .and_then(|s| s.phase.as_ref())
        .map(|p| p.as_str())
        .unwrap_or("Unknown");
    
    if pvc_phase != "Bound" {
        anyhow::bail!("PVC is not bound (phase: {})", pvc_phase);
    }
    info!("  PVC: bound and ready");
    
    // Verify Pod exists and is running
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pod_list = pods.list(&ListParams::default().labels(&format!("instance={}", name))).await
        .context("Failed to list pods")?;
    
    if pod_list.items.is_empty() {
        anyhow::bail!("No pods found for instance");
    }
    
    let pod = &pod_list.items[0];
    let pod_phase = pod.status
        .as_ref()
        .and_then(|s| s.phase.as_ref())
        .map(|p| p.as_str())
        .unwrap_or("Unknown");
    
    if pod_phase != "Running" {
        anyhow::bail!("Pod is not running (phase: {})", pod_phase);
    }
    info!("  Pod: running");
    
    Ok(())
}

async fn cleanup_postgres_instance(client: &Client, namespace: &str, name: &str) -> Result<()> {
    let delete_params = DeleteParams::default();

    // Delete Service
    info!("  Deleting Service...");
    let services: Api<Service> = Api::namespaced(client.clone(), namespace);
    match services.delete(&format!("{}-svc", name), &delete_params).await {
        Ok(_) => info!("    ✓ Service deleted"),
        Err(e) => warn!("    Failed to delete Service: {}", e),
    }

    // Delete StatefulSet
    info!("  Deleting StatefulSet...");
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    match statefulsets.delete(name, &delete_params).await {
        Ok(_) => info!("    ✓ StatefulSet deleted"),
        Err(e) => warn!("    Failed to delete StatefulSet: {}", e),
    }

    // Wait a bit for pod to terminate
    sleep(Duration::from_secs(5)).await;

    // Delete PVC
    info!("  Deleting PersistentVolumeClaim...");
    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), namespace);
    match pvcs.delete(&format!("{}-pvc", name), &delete_params).await {
        Ok(_) => info!("    ✓ PersistentVolumeClaim deleted"),
        Err(e) => warn!("    Failed to delete PersistentVolumeClaim: {}", e),
    }

    Ok(())
}

