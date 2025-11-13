use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub aks_cluster_name: String,
    pub aks_resource_group: String,
    pub aks_namespace: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .context("DATABASE_URL must be set")?,
            server_host: std::env::var("SERVER_HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: std::env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .context("SERVER_PORT must be a valid port number")?,
            aks_cluster_name: std::env::var("AKS_CLUSTER_NAME")
                .context("AKS_CLUSTER_NAME must be set")?,
            aks_resource_group: std::env::var("AKS_RESOURCE_GROUP")
                .context("AKS_RESOURCE_GROUP must be set")?,
            aks_namespace: std::env::var("AKS_NAMESPACE")
                .unwrap_or_else(|_| "toygres".to_string()),
        })
    }
}

