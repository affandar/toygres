use anyhow::Result;
use reqwest::StatusCode;

use crate::commands::server::ensure_server_running;

pub async fn list(status: Option<String>, instance: Option<String>, limit: usize) -> Result<()> {
    // Ensure server is running (auto-start if needed)
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let response = reqwest::get(format!("{}/api/server/orchestrations", api_url))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to API: {}", e))?;
    
    if !response.status().is_success() {
        anyhow::bail!("API error: {}", response.status());
    }
    
    let mut orchestrations: Vec<serde_json::Value> = response.json().await?;
    
    // Filter by status if provided
    if let Some(status_filter) = &status {
        orchestrations.retain(|o| {
            o["status"].as_str().map(|s| s.contains(status_filter)).unwrap_or(false)
        });
    }
    
    // Filter by instance name if provided
    if let Some(instance_filter) = &instance {
        orchestrations.retain(|o| {
            // Check if the instance_id contains the instance name
            // Orchestration IDs follow patterns like: create-<name>-<guid>, delete-<name>-<guid>
            if let Some(id) = o["instance_id"].as_str() {
                id.contains(instance_filter)
            } else {
                false
            }
        });
    }
    
    // Limit results
    orchestrations.truncate(limit);
    
    // Show filter info if applied
    if let Some(ref inst) = instance {
        println!("Orchestrations for instance: {}", inst);
    } else {
        println!("Orchestrations (Advanced Diagnostics)");
    }
    
    if let Some(ref stat) = status {
        println!("Filtered by status: {}", stat);
    }
    
    println!("{}", "=".repeat(110));
    println!();
    println!("{:<35} {:<25} {:<10} {:<10} {:<20}", 
             "ID", "TYPE", "VERSION", "STATUS", "STARTED");
    println!("{}", "-".repeat(110));
    
    for orch in &orchestrations {
        let id = orch["instance_id"].as_str().unwrap_or("-");
        let name = orch["orchestration_name"].as_str().unwrap_or("-");
        let version = orch["orchestration_version"].as_str().unwrap_or("-");
        let status = orch["status"].as_str().unwrap_or("-");
        let created = orch["created_at"].as_str().unwrap_or("-");
        
        // Shorten the name for display
        let short_name = name.split("::").last().unwrap_or(name);
        
        println!("{:<35} {:<25} {:<10} {:<10} {}", 
                 id, short_name, version, status, created);
    }
    
    println!();
    
    if orchestrations.is_empty() {
        if let Some(ref inst) = instance {
            println!("No orchestrations found for instance: {}", inst);
            println!();
            println!("Tips:");
            println!("  - Check if the instance name is correct");
            println!("  - Instance names are part of orchestration IDs (e.g., create-mydb-12345678)");
            println!("  - Try listing all orchestrations without the filter");
        } else {
            println!("No orchestrations found");
        }
    } else {
        println!("{} orchestration(s) found", orchestrations.len());
        println!();
        println!("Use './toygres server orchestration <ID>' for details");
    }
    
    Ok(())
}

pub async fn get(id: &str, history: bool) -> Result<()> {
    // Ensure server is running (auto-start if needed)
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let response = reqwest::get(format!("{}/api/server/orchestrations/{}", api_url, id))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to API: {}", e))?;
    
    if response.status() == StatusCode::NOT_FOUND {
        anyhow::bail!("Orchestration '{}' not found", id);
    }
    
    if !response.status().is_success() {
        anyhow::bail!("API error: {}", response.status());
    }
    
    let orch: serde_json::Value = response.json().await?;
    
    println!("Orchestration: {}", id);
    println!("{}", "=".repeat(80));
    println!();
    
    let status = orch["status"].as_str().unwrap_or("-");
    println!("Status:          {}", status);
    println!("Type:            {}", orch["orchestration_name"].as_str().unwrap_or("-"));
    println!("Version:         {}", orch["orchestration_version"].as_str().unwrap_or("-"));
    println!("Execution:       #{}", orch["current_execution_id"].as_i64().unwrap_or(0));
    println!();
    println!("Timeline:");
    println!("  Created:       {}", orch["created_at"].as_str().unwrap_or("-"));
    println!("  Updated:       {}", orch["updated_at"].as_str().unwrap_or("-"));
    println!();
    
    // Show output if available
    if let Some(output_val) = orch.get("output") {
        if !output_val.is_null() {
            println!("Output:");
            if let Some(output_str) = output_val.as_str() {
                // Try to parse as JSON for prettier display
                if let Ok(output_json) = serde_json::from_str::<serde_json::Value>(output_str) {
                    println!("{}", serde_json::to_string_pretty(&output_json).unwrap_or(output_str.to_string()));
                } else {
                    println!("{}", output_str);
                }
            }
            println!();
        }
    }
    
    // Show execution history if --history flag is set
    if history {
        if let Some(history_arr) = orch["history"].as_array() {
            if !history_arr.is_empty() {
                println!("Execution History ({} events):", history_arr.len());
                println!("{}", "-".repeat(80));
                println!();
                
                // Pretty-print the history JSON
                if let Ok(pretty) = serde_json::to_string_pretty(history_arr) {
                    println!("{}", pretty);
                } else {
                    println!("{:?}", history_arr);
                }
                println!();
            } else {
                println!("No execution history available");
                println!();
            }
        }
    } else {
        // Show hint about --history flag
        if let Some(history_arr) = orch["history"].as_array() {
            if !history_arr.is_empty() {
                println!("Use '--history' to see {} execution events", history_arr.len());
                println!();
            }
        }
    }
    
    println!("Use './toygres get <instance>' to check instance status");
    
    Ok(())
}

pub async fn cancel(id: &str, force: bool) -> Result<()> {
    // Ensure server is running (auto-start if needed)
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    // First, get orchestration info to show confirmation details
    let response = reqwest::get(format!("{}/api/server/orchestrations/{}", api_url, id))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to API: {}", e))?;
    
    if response.status() == StatusCode::NOT_FOUND {
        anyhow::bail!("Orchestration '{}' not found", id);
    }
    
    if !response.status().is_success() {
        anyhow::bail!("API error: {}", response.status());
    }
    
    let orch: serde_json::Value = response.json().await?;
    let status = orch["status"].as_str().unwrap_or("unknown");
    let orch_type = orch["orchestration_name"].as_str()
        .and_then(|s| s.split("::").last())
        .unwrap_or("-");
    
    // Check if already completed or failed
    if status == "Completed" || status == "Failed" {
        println!("⚠️  Orchestration is already {}", status.to_lowercase());
        println!();
        println!("  ID:     {}", id);
        println!("  Type:   {}", orch_type);
        println!("  Status: {}", status);
        println!();
        println!("Cannot cancel a completed or failed orchestration.");
        return Ok(());
    }
    
    // Show confirmation unless --force
    if !force {
        println!("⚠️  Cancel Orchestration");
        println!();
        println!("  ID:       {}", id);
        println!("  Type:     {}", orch_type);
        println!("  Status:   {}", status);
        println!();
        println!("This will stop the orchestration immediately.");
        println!("The instance may be in an incomplete state.");
        println!();
        print!("Are you sure you want to cancel? (y/N) ");
        
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let answer = input.trim().to_lowercase();
        if answer != "y" && answer != "yes" {
            println!();
            println!("Cancelled.");
            return Ok(());
        }
        println!();
    }
    
    // Make the cancel request
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/server/orchestrations/{}/cancel", api_url, id))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to cancel orchestration: {}", e))?;
    
    if !response.status().is_success() {
        let error_msg = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        anyhow::bail!("Failed to cancel orchestration: {}", error_msg);
    }
    
    println!("✓ Orchestration cancelled");
    println!();
    println!("Check instance state with: ./toygres get <instance>");
    
    Ok(())
}

