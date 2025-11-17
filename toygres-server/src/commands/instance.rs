use anyhow::Result;
use duroxide::Client;
use reqwest::StatusCode;
use toygres_orchestrations::names::orchestrations;
use toygres_orchestrations::types::*;
use uuid::Uuid;

use crate::commands::server::ensure_server_running;
use crate::db;

pub async fn run_list(output: String) -> Result<()> {
    // Ensure server is running (auto-start if needed)
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let response = reqwest::get(format!("{}/api/instances", api_url))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to API: {}", e))?;
    
    if !response.status().is_success() {
        anyhow::bail!("API error: {}", response.status());
    }
    
    let instances: Vec<serde_json::Value> = response.json().await?;
    
    if output == "json" {
        println!("{}", serde_json::to_string_pretty(&instances)?);
    } else {
        // Table format
        println!("{:<15} {:<20} {:<10} {:<10} {:<8} {:<10}", 
                 "NAME", "DNS NAME", "STATE", "HEALTH", "VERSION", "STORAGE");
        println!("{}", "-".repeat(85));
        
        for inst in &instances {
            let name = inst["user_name"].as_str().unwrap_or("-");
            let dns = inst["dns_name"].as_str().unwrap_or("-");
            let state = inst["state"].as_str().unwrap_or("-");
            let health = inst["health_status"].as_str().unwrap_or("-");
            let version = inst["postgres_version"].as_str().unwrap_or("-");
            let storage = inst["storage_size_gb"].as_i64().unwrap_or(0);
            
            println!("{:<15} {:<20} {:<10} {:<10} {:<8} {}GB", 
                     name, dns, state, health, version, storage);
        }
        
        println!();
        println!("{} instance(s) found", instances.len());
    }
    
    Ok(())
}

pub async fn run_get(name: String, output: String) -> Result<()> {
    // Ensure server is running (auto-start if needed)
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let response = reqwest::get(format!("{}/api/instances/{}", api_url, name))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to API: {}", e))?;
    
    if response.status() == StatusCode::NOT_FOUND {
        anyhow::bail!("Instance '{}' not found", name);
    }
    
    if !response.status().is_success() {
        anyhow::bail!("API error: {}", response.status());
    }
    
    let instance: serde_json::Value = response.json().await?;
    
    if output == "json" {
        println!("{}", serde_json::to_string_pretty(&instance)?);
    } else {
        // Table format
        println!("Instance: {}", name);
        println!("{}", "=".repeat(60));
        println!();
        println!("Status:");
        println!("  State:              {}", instance["state"].as_str().unwrap_or("-"));
        println!("  Health:             {}", instance["health_status"].as_str().unwrap_or("-"));
        println!("  PostgreSQL Version: {}", instance["postgres_version"].as_str().unwrap_or("-"));
        println!();
        println!("Identity:");
        println!("  User Name:          {}", instance["user_name"].as_str().unwrap_or("-"));
        println!("  K8s Name:           {}", instance["k8s_name"].as_str().unwrap_or("-"));
        println!("  DNS Name:           {}", instance["dns_name"].as_str().unwrap_or("-"));
        println!();
        println!("Configuration:");
        println!("  Storage:            {} GB", instance["storage_size_gb"].as_i64().unwrap_or(0));
        println!("  Load Balancer:      {}", instance["use_load_balancer"].as_bool().unwrap_or(false));
        println!();
        println!("Network:");
        if let Some(dns_conn) = instance["dns_connection_string"].as_str() {
            println!("  DNS Connection:     {}", dns_conn);
        }
        if let Some(ip_conn) = instance["ip_connection_string"].as_str() {
            println!("  IP Connection:      {}", ip_conn);
        }
        if let Some(external_ip) = instance["external_ip"].as_str() {
            println!("  External IP:        {}", external_ip);
        }
        println!();
        println!("Timestamps:");
        println!("  Created:            {}", instance["created_at"].as_str().unwrap_or("-"));
        println!("  Updated:            {}", instance["updated_at"].as_str().unwrap_or("-"));
    }
    
    Ok(())
}

pub async fn run_create(
    name: String,
    password: String,
    version: Option<String>,
    storage: Option<i32>,
    internal: bool,
    namespace: Option<String>,
) -> Result<()> {
    tracing::info!("Toygres Control Plane CLI");
    
    // Initialize Duroxide
    let (runtime, store) = crate::duroxide::initialize().await?;
    
    // Create Duroxide client
    let client = Client::new(store);
    
    // Execute create command
    handle_create(client, name, password, version, storage, !internal, namespace).await?;
    
    // Shutdown runtime
    tracing::info!("Shutting down Duroxide runtime");
    runtime.shutdown(None).await;

    Ok(())
}

pub async fn run_delete(
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    tracing::info!("Toygres Control Plane CLI");
    
    // Initialize Duroxide
    let (runtime, store) = crate::duroxide::initialize().await?;
    
    // Create Duroxide client
    let client = Client::new(store);
    
    // Execute delete command
    handle_delete(client, name, namespace).await?;
    
    // Shutdown runtime
    tracing::info!("Shutting down Duroxide runtime");
    runtime.shutdown(None).await;

    Ok(())
}

async fn handle_create(
    client: Client,
    name: String,
    password: String,
    version: Option<String>,
    storage: Option<i32>,
    use_load_balancer: bool,
    namespace: Option<String>,
) -> Result<()> {
    // Generate unique instance name with 8-character GUID suffix
    let guid = Uuid::new_v4().to_string();
    let guid_suffix = &guid[..8];
    let unique_instance_name = format!("{}-{}", name, guid_suffix);
    
    tracing::info!("Creating PostgreSQL instance: {} (K8s name: {})", name, unique_instance_name);
    
    // Use the user-provided name directly as the DNS label
    // This creates DNS names like: <name>.<region>.cloudapp.azure.com
    let dns_label = Some(name.clone());
    
    let instance_id = format!("create-{}", unique_instance_name);
    
    // Build input (use unique instance name for K8s resources)
    let input = CreateInstanceInput {
        user_name: name.clone(),
        name: unique_instance_name.clone(),
        password,
        postgres_version: version,
        storage_size_gb: storage,
        use_load_balancer: Some(use_load_balancer),
        dns_label,
        namespace,
        orchestration_id: instance_id.clone(),
    };
    
    let input_json = serde_json::to_string(&input)?;
    
    // Start orchestration (non-blocking)
    client
        .start_orchestration(&instance_id, orchestrations::CREATE_INSTANCE, input_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start orchestration: {}", e))?;
    
    // Return immediately - user can check status with 'get' command
    println!("✓ Instance creation started");
    println!();
    println!("  Name:           {}", name);
    println!("  K8s Name:       {}", unique_instance_name);
    println!("  DNS (expected): {}.westus3.cloudapp.azure.com", name);
    println!();
    println!("The instance is being created in the background.");
    println!();
    println!("Check status with:");
    println!("  ./toygres get {}", name);
    println!();
    println!("For advanced diagnostics:");
    println!("  ./toygres server orchestration {}", instance_id);
    
    Ok(())
}

async fn handle_delete(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    tracing::info!("Deleting PostgreSQL instance: {}", name);
    
    // Look up the K8s name by user_name in the CMS database
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite::memory:".to_string());
    
    let k8s_name = if !db_url.starts_with("sqlite") {
        db::lookup_k8s_name_by_user_name(&db_url, &name).await?
    } else {
        // For SQLite testing, assume name is the k8s_name
        name.clone()
    };
    
    tracing::info!("Resolved to K8s instance: {}", k8s_name);
    
    let instance_id = format!("delete-{}", k8s_name);
    
    // Build input (use k8s_name for deletion)
    let input = DeleteInstanceInput {
        name: k8s_name.clone(),
        namespace,
        orchestration_id: instance_id.clone(),
    };
    
    let input_json = serde_json::to_string(&input)?;
    
    // Start orchestration (non-blocking)
    client
        .start_orchestration(&instance_id, orchestrations::DELETE_INSTANCE, input_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start orchestration: {}", e))?;
    
    // Return immediately - user can check status with 'get' command
    println!("✓ Instance deletion started");
    println!();
    println!("  Name:     {}", name);
    println!("  K8s Name: {}", k8s_name);
    println!();
    println!("The instance is being deleted in the background.");
    println!();
    println!("Check status with:");
    println!("  ./toygres get {}", name);
    println!();
    println!("For advanced diagnostics:");
    println!("  ./toygres server orchestration {}", instance_id);
    
    Ok(())
}

