use anyhow::Result;
use std::collections::HashMap;

use crate::commands::server::ensure_server_running;

pub async fn stats(watch: bool) -> Result<()> {
    // Ensure server is running
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    if watch {
        println!("Watch mode - press Ctrl+C to stop");
        println!();
        
        loop {
            // Clear screen (Unix)
            #[cfg(unix)]
            {
                print!("\x1B[2J\x1B[1;1H");
            }
            
            display_stats(&api_url).await?;
            
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    } else {
        display_stats(&api_url).await
    }
}

async fn display_stats(api_url: &str) -> Result<()> {
    // Fetch orchestrations
    let orchestrations_response = reqwest::get(format!("{}/api/server/orchestrations", api_url))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch orchestrations: {}", e))?;
    
    let orchestrations: Vec<serde_json::Value> = if orchestrations_response.status().is_success() {
        orchestrations_response.json().await.unwrap_or_default()
    } else {
        Vec::new()
    };
    
    // Fetch instances
    let instances_response = reqwest::get(format!("{}/api/instances", api_url))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch instances: {}", e))?;
    
    let instances: Vec<serde_json::Value> = if instances_response.status().is_success() {
        instances_response.json().await.unwrap_or_default()
    } else {
        Vec::new()
    };
    
    println!("Toygres System Statistics");
    println!("{}", "=".repeat(80));
    println!();
    
    // Instance statistics
    let total_instances = instances.len();
    let running = instances.iter().filter(|i| i["state"].as_str() == Some("running")).count();
    let creating = instances.iter().filter(|i| i["state"].as_str() == Some("creating")).count();
    let deleting = instances.iter().filter(|i| i["state"].as_str() == Some("deleting")).count();
    let failed = instances.iter().filter(|i| i["state"].as_str() == Some("failed")).count();
    
    println!("Instances:");
    println!("  Total:             {}", total_instances);
    println!("  Running:           {}  {}", running, format_percentage(running, total_instances));
    println!("  Creating:          {}  {}", creating, format_percentage(creating, total_instances));
    println!("  Deleting:          {}  {}", deleting, format_percentage(deleting, total_instances));
    println!("  Failed:            {}  {}", failed, format_percentage(failed, total_instances));
    println!();
    
    // Health status
    let healthy = instances.iter().filter(|i| i["health_status"].as_str() == Some("healthy")).count();
    let unhealthy = instances.iter().filter(|i| i["health_status"].as_str() == Some("unhealthy")).count();
    let unknown = instances.iter().filter(|i| {
        let health = i["health_status"].as_str().unwrap_or("unknown");
        health != "healthy" && health != "unhealthy"
    }).count();
    
    println!("Health Status:");
    println!("  Healthy:           {}  {}", healthy, format_percentage(healthy, total_instances));
    println!("  Unhealthy:         {}  {}", unhealthy, format_percentage(unhealthy, total_instances));
    println!("  Unknown:           {}  {}", unknown, format_percentage(unknown, total_instances));
    println!();
    
    // Orchestration statistics
    let total_orches = orchestrations.len();
    let running_orches = orchestrations.iter().filter(|o| o["status"].as_str() == Some("Running")).count();
    let completed_orches = orchestrations.iter().filter(|o| o["status"].as_str() == Some("Completed")).count();
    let failed_orches = orchestrations.iter().filter(|o| o["status"].as_str() == Some("Failed")).count();
    
    println!("Orchestrations (All Time):");
    println!("  Total:             {}", total_orches);
    println!("  Running:           {}  {}", running_orches, format_percentage(running_orches, total_orches));
    println!("  Completed:         {}  {}", completed_orches, format_percentage(completed_orches, total_orches));
    println!("  Failed:            {}  {}", failed_orches, format_percentage(failed_orches, total_orches));
    println!();
    
    // By type
    let mut type_counts: HashMap<String, (usize, usize, usize)> = HashMap::new();
    for orch in &orchestrations {
        if let Some(name) = orch["orchestration_name"].as_str() {
            let short_name = name.split("::").last().unwrap_or(name).to_string();
            let status = orch["status"].as_str().unwrap_or("unknown");
            
            let entry = type_counts.entry(short_name).or_insert((0, 0, 0));
            entry.0 += 1; // total
            if status == "Completed" {
                entry.1 += 1; // completed
            } else if status == "Running" {
                entry.2 += 1; // running
            }
        }
    }
    
    if !type_counts.is_empty() {
        println!("By Type:");
        for (name, (total, completed, running)) in type_counts.iter() {
            println!("  {:<25} {} total, {} completed, {} running", 
                     name, total, completed, running);
        }
        println!();
    }
    
    // Resource usage
    let total_storage: i64 = instances.iter()
        .filter_map(|i| i["storage_size_gb"].as_i64())
        .sum();
    
    if total_instances > 0 {
        println!("Resource Usage:");
        println!("  Storage (provisioned):  {} GB across {} instances", total_storage, total_instances);
        println!("  Average per instance:   {} GB", if total_instances > 0 { total_storage / total_instances as i64 } else { 0 });
        println!();
    }
    
    // Timestamp
    let now = chrono::Utc::now();
    println!("Last Updated: {} (just now)", now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    
    Ok(())
}

fn format_percentage(count: usize, total: usize) -> String {
    if total == 0 {
        return "  0%".to_string();
    }
    let pct = (count as f64 / total as f64) * 100.0;
    format!("{:3.0}%", pct)
}

pub async fn config() -> Result<()> {
    println!("Server Configuration");
    println!("{}", "=".repeat(80));
    println!();
    
    // Server info
    println!("Server:");
    println!("  Mode:              standalone (API + Workers)");
    println!("  API Port:          8080");
    println!("  Workers:           1");
    println!();
    
    // Database
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite::memory:".to_string());
    
    let db_display = if db_url.contains('@') {
        // Hide password in URL
        let parts: Vec<&str> = db_url.split('@').collect();
        if parts.len() == 2 {
            format!("postgresql://***@{}", parts[1])
        } else {
            "***hidden***".to_string()
        }
    } else {
        db_url.clone()
    };
    
    println!("Database:");
    println!("  URL:               {}", db_display);
    println!("  Schema (CMS):      toygres_cms");
    println!("  Schema (Duroxide): toygres_duroxide");
    println!();
    
    // Kubernetes
    let cluster = std::env::var("AKS_CLUSTER_NAME").unwrap_or_else(|_| "not set".to_string());
    let resource_group = std::env::var("AKS_RESOURCE_GROUP").unwrap_or_else(|_| "not set".to_string());
    let namespace = std::env::var("AKS_NAMESPACE").unwrap_or_else(|_| "toygres".to_string());
    let kubeconfig = std::env::var("HOME")
        .map(|h| format!("{}/.kube/config", h))
        .unwrap_or_else(|_| "~/.kube/config".to_string());
    
    println!("Kubernetes:");
    println!("  Cluster:           {}", cluster);
    println!("  Resource Group:    {}", resource_group);
    println!("  Namespace:         {}", namespace);
    println!("  Kubeconfig:        {}", kubeconfig);
    println!();
    
    // Paths
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    println!("Paths:");
    println!("  PID File:          {}/.toygres/server.pid", home);
    println!("  Log File:          {}/.toygres/server.log", home);
    println!();
    
    // Environment variables status
    println!("Environment Variables:");
    println!("  DATABASE_URL:        {}", if std::env::var("DATABASE_URL").is_ok() { "✓ Set" } else { "✗ Not set" });
    println!("  AKS_CLUSTER_NAME:    {}", if std::env::var("AKS_CLUSTER_NAME").is_ok() { "✓ Set" } else { "✗ Not set" });
    println!("  AKS_RESOURCE_GROUP:  {}", if std::env::var("AKS_RESOURCE_GROUP").is_ok() { "✓ Set" } else { "✗ Not set" });
    println!("  AKS_NAMESPACE:       {}", if std::env::var("AKS_NAMESPACE").is_ok() { "✓ Set" } else { "✗ Not set" });
    println!("  RUST_LOG:            {}", std::env::var("RUST_LOG").unwrap_or_else(|_| "not set".to_string()));
    
    Ok(())
}

pub async fn env(show_secrets: bool) -> Result<()> {
    println!("Environment Variables");
    println!("{}", "=".repeat(80));
    println!();
    
    // Required variables
    println!("Required:");
    
    let db_url = std::env::var("DATABASE_URL");
    match &db_url {
        Ok(url) => {
            if show_secrets {
                println!("  DATABASE_URL           ✓ Set ({})", url);
            } else {
                println!("  DATABASE_URL           ✓ Set (***hidden***)");
            }
        }
        Err(_) => {
            println!("  DATABASE_URL           ✗ Not set");
        }
    }
    
    let cluster = std::env::var("AKS_CLUSTER_NAME");
    match &cluster {
        Ok(val) => println!("  AKS_CLUSTER_NAME       ✓ Set ({})", val),
        Err(_) => println!("  AKS_CLUSTER_NAME       ✗ Not set"),
    }
    
    let rg = std::env::var("AKS_RESOURCE_GROUP");
    match &rg {
        Ok(val) => println!("  AKS_RESOURCE_GROUP     ✓ Set ({})", val),
        Err(_) => println!("  AKS_RESOURCE_GROUP     ✗ Not set"),
    }
    
    println!();
    
    // Optional variables
    println!("Optional:");
    
    let namespace = std::env::var("AKS_NAMESPACE");
    match &namespace {
        Ok(val) => println!("  AKS_NAMESPACE          ✓ Set ({})", val),
        Err(_) => println!("  AKS_NAMESPACE          ✗ Not set (using default: toygres)"),
    }
    
    let rust_log = std::env::var("RUST_LOG");
    match &rust_log {
        Ok(val) => println!("  RUST_LOG               ✓ Set ({})", val),
        Err(_) => println!("  RUST_LOG               ✗ Not set (using default: info)"),
    }
    
    let api_url = std::env::var("TOYGRES_API_URL");
    match &api_url {
        Ok(val) => println!("  TOYGRES_API_URL        ✓ Set ({})", val),
        Err(_) => println!("  TOYGRES_API_URL        ✗ Not set (using default: http://localhost:8080)"),
    }
    
    println!();
    
    // Computed values
    println!("Computed:");
    
    let home = std::env::var("HOME").unwrap_or_else(|_| "unknown".to_string());
    println!("  HOME                   {}", home);
    
    let kubeconfig = format!("{}/.kube/config", home);
    let exists = std::path::Path::new(&kubeconfig).exists();
    println!("  KUBECONFIG             {} (exists: {})", kubeconfig, if exists { "yes" } else { "no" });
    
    if !show_secrets {
        println!();
        println!("Use --show-secrets to reveal hidden values (not recommended)");
    }
    
    Ok(())
}

pub async fn workers(_watch: bool) -> Result<()> {
    // Ensure server is running
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    // Fetch orchestrations to see what's running
    let response = reqwest::get(format!("{}/api/server/orchestrations", api_url))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch orchestrations: {}", e))?;
    
    if !response.status().is_success() {
        anyhow::bail!("API error: {}", response.status());
    }
    
    let orchestrations: Vec<serde_json::Value> = response.json().await?;
    
    println!("Duroxide Workers");
    println!("{}", "=".repeat(80));
    println!();
    
    // Filter running orchestrations
    let running: Vec<&serde_json::Value> = orchestrations.iter()
        .filter(|o| o["status"].as_str() == Some("Running"))
        .collect();
    
    if running.is_empty() {
        println!("No active workers (no running orchestrations)");
        println!();
        println!("Workers are idle, waiting for work.");
    } else {
        println!("Active Orchestrations:");
        println!();
        println!("{:<35} {:<25} {:<20}", "ID", "TYPE", "STARTED");
        println!("{}", "-".repeat(80));
        
        for orch in &running {
            let id = orch["instance_id"].as_str().unwrap_or("-");
            let name = orch["orchestration_name"].as_str()
                .and_then(|s| s.split("::").last())
                .unwrap_or("-");
            let created = orch["created_at"].as_str().unwrap_or("-");
            
            println!("{:<35} {:<25} {}", id, name, created);
        }
        
        println!();
        println!("{} orchestration(s) running", running.len());
    }
    
    println!();
    
    // Queue info
    let total = orchestrations.len();
    let completed = orchestrations.iter().filter(|o| o["status"].as_str() == Some("Completed")).count();
    let failed = orchestrations.iter().filter(|o| o["status"].as_str() == Some("Failed")).count();
    
    println!("Statistics:");
    println!("  Total Orchestrations:  {}", total);
    println!("  Running:               {}", running.len());
    println!("  Completed:             {}", completed);
    println!("  Failed:                {}", failed);
    
    Ok(())
}

