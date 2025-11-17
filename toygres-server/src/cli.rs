use clap::{Parser, Subcommand};

/// Toygres - PostgreSQL as a Service on AKS
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub mode: Mode,
}

#[derive(Subcommand, Debug)]
pub enum Mode {
    /// Run as standalone server (API + Workers)
    Standalone {
        /// API port
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Number of worker threads
        #[arg(short, long, default_value = "1")]
        workers: usize,
    },
    
    /// Run as API server only (no workers)
    Api {
        /// API port
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    
    /// Run as worker only (no API)
    Worker {
        /// Worker ID
        #[arg(long)]
        worker_id: Option<String>,
    },
    
    /// Create a new PostgreSQL instance
    Create {
        /// DNS name for the instance (e.g., "mydb" creates mydb.<region>.cloudapp.azure.com)
        name: String,
        
        /// PostgreSQL password
        #[arg(short, long)]
        password: String,
        
        /// PostgreSQL version (default: "18")
        #[arg(long, default_value = "18")]
        version: Option<String>,
        
        /// Storage size in GB (default: 10)
        #[arg(long, default_value = "10")]
        storage: Option<i32>,
        
        /// Use ClusterIP instead of LoadBalancer (no public IP)
        #[arg(long)]
        internal: bool,
        
        /// Kubernetes namespace (default: "toygres")
        #[arg(long, default_value = "toygres")]
        namespace: Option<String>,
    },
    
    /// Delete a PostgreSQL instance
    Delete {
        /// DNS name of the instance to delete (e.g., "adardb5")
        name: String,
        
        /// Kubernetes namespace (default: "toygres")
        #[arg(long, default_value = "toygres")]
        namespace: Option<String>,
    },
    
    /// List all PostgreSQL instances
    List {
        /// Output format
        #[arg(short, long, default_value = "table")]
        output: String,
    },
    
    /// Get details of a specific instance
    Get {
        /// DNS name of the instance
        name: String,
        
        /// Output format
        #[arg(short, long, default_value = "table")]
        output: String,
    },
    
    /// Manage local development server
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum ServerCommand {
    /// Start the server in background
    Start {
        /// API port
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Run in foreground (default: background)
        #[arg(short, long)]
        foreground: bool,
    },
    
    /// Stop the server
    Stop,
    
    /// Check if server is running
    Status,
    
    /// View server logs
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        
        /// Number of lines to show
        #[arg(short = 'n', long, default_value = "100")]
        tail: usize,
    },
    
    /// List orchestrations (advanced diagnostics)
    Orchestrations {
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
        
        /// Limit number of results
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    
    /// Get orchestration details (advanced diagnostics)
    Orchestration {
        /// Orchestration ID
        id: String,
        
        /// Show execution history
        #[arg(long)]
        history: bool,
    },
    
    /// Cancel a running orchestration
    Cancel {
        /// Orchestration ID to cancel
        id: String,
        
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
    
    /// Show system statistics and metrics
    Stats {
        /// Watch mode (refresh every 2s)
        #[arg(short, long)]
        watch: bool,
    },
    
    /// Show current configuration
    Config,
    
    /// Show environment variables
    Env {
        /// Show actual secret values (use with caution)
        #[arg(long)]
        show_secrets: bool,
    },
    
    /// Show worker status and activity
    Workers {
        /// Watch mode (live updates)
        #[arg(short, long)]
        watch: bool,
    },
}

