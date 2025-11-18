use anyhow::Result;
use std::path::Path;

use crate::cli::ServerCommand;

pub async fn handle_command(command: ServerCommand) -> Result<()> {
    use std::path::PathBuf;
    
    // Get paths for PID and log files
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let toygres_dir = PathBuf::from(home).join(".toygres");
    let pid_file = toygres_dir.join("server.pid");
    let log_file = toygres_dir.join("server.log");
    
    match command {
        ServerCommand::Start { port, foreground } => {
            start(port, foreground, &pid_file, &log_file).await
        }
        ServerCommand::Stop => {
            stop(&pid_file).await
        }
        ServerCommand::Status => {
            status(&pid_file).await
        }
        ServerCommand::Logs { follow, tail, orchestration } => {
            logs(&log_file, follow, tail, orchestration).await
        }
        ServerCommand::Orchestrations { status, instance, limit } => {
            crate::commands::orchestration::list(status, instance, limit).await
        }
        ServerCommand::Orchestration { id, history } => {
            crate::commands::orchestration::get(&id, history).await
        }
        ServerCommand::Cancel { id, force } => {
            crate::commands::orchestration::cancel(&id, force).await
        }
        ServerCommand::Stats { watch } => {
            crate::commands::system::stats(watch).await
        }
        ServerCommand::Config => {
            crate::commands::system::config().await
        }
        ServerCommand::Env { show_secrets } => {
            crate::commands::system::env(show_secrets).await
        }
        ServerCommand::Workers { watch } => {
            crate::commands::system::workers(watch).await
        }
    }
}

pub async fn run_standalone_mode(port: u16, _workers: usize) -> Result<()> {
    tracing::info!("Starting Toygres in standalone mode (API + Workers)");
    tracing::info!("API port: {}", port);
    
    // Initialize Duroxide
    let (runtime, store) = crate::duroxide::initialize().await?;
    
    // Create API state
    let client = std::sync::Arc::new(duroxide::Client::new(store.clone()));
    let state = crate::api::AppState {
        duroxide_client: client,
        store: store.clone(),
    };
    
    // Start API server
    tracing::info!("Starting API server on 0.0.0.0:{}", port);
    
    // Spawn API server task
    let api_handle = tokio::spawn(async move {
        if let Err(e) = crate::api::start_server(port, state).await {
            tracing::error!("API server error: {}", e);
        }
    });
    
    tracing::info!("✓ Toygres server ready");
    tracing::info!("  API: http://0.0.0.0:{}", port);
    tracing::info!("  Press Ctrl+C to stop");
    
    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    
    tracing::info!("Shutting down...");
    api_handle.abort();
    
    tracing::info!("Shutting down Duroxide runtime");
    runtime.shutdown(None).await;
    
    Ok(())
}

async fn start(
    port: u16,
    foreground: bool,
    pid_file: &Path,
    log_file: &Path,
) -> Result<()> {
    use std::process::{Command, Stdio};
    
    // Check if server is already running
    if is_running(pid_file)? {
        let pid = read_pid(pid_file)?;
        tracing::warn!("Server is already running (PID: {})", pid);
        println!("✓ Server is already running");
        println!("  PID: {}", pid);
        println!("  Use 'toygres-server server status' for details");
        return Ok(());
    }
    
    if foreground {
        // Run in foreground (blocks)
        println!("Starting Toygres server in foreground mode...");
        println!("Press Ctrl+C to stop");
        return run_standalone_mode(port, 1).await;
    }
    
    // Ensure .toygres directory exists
    if let Some(parent) = pid_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    // Get path to current executable
    let exe = std::env::current_exe()?;
    
    println!("Starting Toygres server in background...");
    
    // Start server as background process
    let log_file_handle = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;
    
    let child = Command::new(exe)
        .args(["standalone", "--port", &port.to_string()])
        .stdout(Stdio::from(log_file_handle.try_clone()?))
        .stderr(Stdio::from(log_file_handle))
        .spawn()?;
    
    // Save PID
    std::fs::write(pid_file, child.id().to_string())?;
    
    // Wait a moment for server to start
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Check if it's still running
    if is_running(pid_file)? {
        println!("✓ Server started successfully");
        println!("  PID: {}", child.id());
        println!("  API: http://localhost:{}", port);
        println!("  Logs: {}", log_file.display());
        println!();
        println!("Use './toygres server stop' to stop the server");
        println!("Use './toygres server logs -f' to view logs");
        Ok(())
    } else {
        std::fs::remove_file(pid_file).ok();
        anyhow::bail!("Server failed to start. Check logs at: {}", log_file.display())
    }
}

async fn stop(pid_file: &Path) -> Result<()> {
    if !pid_file.exists() {
        println!("✗ Server is not running (no PID file found)");
        return Ok(());
    }
    
    let pid = read_pid(pid_file)?;
    
    println!("Stopping Toygres server (PID: {})...", pid);
    
    // Send SIGTERM for graceful shutdown
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        
        if let Err(e) = kill(Pid::from_raw(pid), Signal::SIGTERM) {
            println!("✗ Failed to send stop signal: {}", e);
            std::fs::remove_file(pid_file).ok();
            return Err(anyhow::anyhow!("Process may not be running"));
        }
    }
    
    #[cfg(not(unix))]
    {
        println!("⚠️  Graceful shutdown not supported on this platform");
        return Err(anyhow::anyhow!("Platform not supported for server management"));
    }
    
    // Wait for process to stop (up to 30 seconds)
    for i in 0..30 {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        if !is_running(pid_file)? {
            std::fs::remove_file(pid_file).ok();
            println!("✓ Server stopped successfully");
            return Ok(());
        }
        
        if i == 29 {
            println!("⚠️  Server did not stop gracefully, force killing...");
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                kill(Pid::from_raw(pid), Signal::SIGKILL).ok();
            }
            std::fs::remove_file(pid_file).ok();
        }
    }
    
    Ok(())
}

async fn status(pid_file: &Path) -> Result<()> {
    if !pid_file.exists() {
        println!("✗ Server is not running");
        return Ok(());
    }
    
    let pid = read_pid(pid_file)?;
    
    if is_running(pid_file)? {
        println!("✓ Server is running");
        println!("  PID: {}", pid);
        println!("  API: http://localhost:8080");
        
        // Try to get health info
        if let Ok(response) = reqwest::get("http://localhost:8080/health").await {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                println!("  Status: {}", json.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"));
                println!("  Version: {}", json.get("version").and_then(|v| v.as_str()).unwrap_or("unknown"));
            }
        }
    } else {
        println!("✗ Server is not running (stale PID file)");
        std::fs::remove_file(pid_file).ok();
    }
    
    Ok(())
}

async fn logs(log_file: &Path, follow: bool, tail: usize, orchestration: Option<String>) -> Result<()> {
    if !log_file.exists() {
        println!("✗ No log file found at: {}", log_file.display());
        println!("  Server may not have been started yet");
        return Ok(());
    }
    
    if follow {
        // Follow logs (like tail -f)
        if let Some(ref orch_id) = orchestration {
            println!("Following logs from: {} (filtered by orchestration: {})", log_file.display(), orch_id);
        } else {
            println!("Following logs from: {}", log_file.display());
        }
        println!("Press Ctrl+C to stop");
        println!();
        
        // Use tail command with grep on Unix
        #[cfg(unix)]
        {
            if let Some(orch_id) = orchestration {
                // Use tail -f piped through grep for filtering
                let mut child = std::process::Command::new("sh")
                    .args([
                        "-c",
                        &format!(
                            "tail -f -n {} {} | grep --line-buffered '{}'",
                            tail,
                            log_file.to_str().unwrap(),
                            orch_id
                        )
                    ])
                    .spawn()?;
                
                let status = child.wait()?;
                if !status.success() {
                    anyhow::bail!("Failed to tail and filter logs");
                }
            } else {
                let status = std::process::Command::new("tail")
                    .args(["-f", "-n", &tail.to_string(), log_file.to_str().unwrap()])
                    .status()?;
                
                if !status.success() {
                    anyhow::bail!("Failed to tail logs");
                }
            }
        }
        
        #[cfg(not(unix))]
        {
            anyhow::bail!("Follow mode not supported on this platform");
        }
    } else {
        // Show last N lines
        use std::io::{BufRead, BufReader};
        
        let file = std::fs::File::open(log_file)?;
        let reader = BufReader::new(file);
        
        // Filter lines if orchestration ID is provided
        let all_lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();
        
        let filtered_lines: Vec<&String> = if let Some(ref orch_id) = orchestration {
            all_lines.iter()
                .filter(|line| line.contains(orch_id))
                .collect()
        } else {
            all_lines.iter().collect()
        };
        
        let start = if filtered_lines.len() > tail { 
            filtered_lines.len() - tail 
        } else { 
            0 
        };
        
        if let Some(ref orch_id) = orchestration {
            if filtered_lines.is_empty() {
                println!("No log entries found for orchestration: {}", orch_id);
                println!();
                println!("Tips:");
                println!("  - Check if the orchestration ID is correct");
                println!("  - Try without the filter to see all logs");
                return Ok(());
            }
            
            println!("Showing {} log entries for orchestration: {}", filtered_lines.len(), orch_id);
            println!("{}", "-".repeat(80));
            println!();
        }
        
        for line in &filtered_lines[start..] {
            println!("{}", line);
        }
        
        if let Some(_) = orchestration {
            println!();
            println!("Showing last {} matching entries (total: {} matches)", 
                     filtered_lines.len() - start, 
                     filtered_lines.len());
        }
    }
    
    Ok(())
}

fn is_running(pid_file: &Path) -> Result<bool> {
    if !pid_file.exists() {
        return Ok(false);
    }
    
    let pid = read_pid(pid_file)?;
    
    // Check if process exists
    #[cfg(unix)]
    {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        
        // Sending None (signal 0) checks if process exists without actually sending a signal
        Ok(kill(Pid::from_raw(pid), None).is_ok())
    }
    
    #[cfg(not(unix))]
    {
        Ok(true) // Assume running on non-Unix platforms
    }
}

fn read_pid(pid_file: &Path) -> Result<i32> {
    let contents = std::fs::read_to_string(pid_file)?;
    contents.trim().parse::<i32>()
        .map_err(|e| anyhow::anyhow!("Invalid PID file: {}", e))
}

/// Ensure the API server is running, auto-start if needed
pub async fn ensure_server_running() -> Result<()> {
    use std::path::PathBuf;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    // First, try to connect to the API
    if let Ok(response) = reqwest::get(format!("{}/health", api_url)).await {
        if response.status().is_success() {
            // Server is running
            return Ok(());
        }
    }
    
    // If using a non-local API, can't auto-start
    if !api_url.starts_with("http://localhost") && !api_url.starts_with("http://127.0.0.1") {
        anyhow::bail!("Cannot connect to remote API: {}\nPlease ensure the server is running.", api_url);
    }
    
    // Check if local server should be running
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let toygres_dir = PathBuf::from(home).join(".toygres");
    let pid_file = toygres_dir.join("server.pid");
    let log_file = toygres_dir.join("server.log");
    
    // Check if server process exists
    if is_running(&pid_file)? {
        // PID file exists but API not responding - give it more time
        println!("Server is starting up, waiting...");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        
        if let Ok(response) = reqwest::get(format!("{}/health", api_url)).await {
            if response.status().is_success() {
                return Ok(());
            }
        }
        
        anyhow::bail!("Server is running but not responding. Check logs: ./toygres server logs");
    }
    
    anyhow::bail!("Server is required for this command. Start it with: ./toygres server start");
}

