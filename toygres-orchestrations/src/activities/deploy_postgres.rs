//! Deploy PostgreSQL activity

use duroxide::ActivityContext;
use crate::activity_types::{DeployPostgresInput, DeployPostgresOutput, ImageType};
use crate::k8s_client::{get_k8s_client, check_resources_exist};
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Service, Secret, ConfigMap};
use k8s_openapi::api::apps::v1::StatefulSet;
use kube::api::{Api, PostParams};
use tera::{Tera, Context as TeraContext};

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::deploy-postgres";

/// Default ACR for pg_durable images
const DEFAULT_PG_DURABLE_REGISTRY: &str = "toygresacr.azurecr.io";

/// Get the Docker image based on image type
fn get_image(input: &DeployPostgresInput) -> String {
    match input.image_type {
        ImageType::Stock => {
            format!("postgres:{}", input.postgres_version)
        }
        ImageType::PgDurable => {
            let registry = input.image_registry
                .as_deref()
                .unwrap_or(DEFAULT_PG_DURABLE_REGISTRY);
            format!("{}/pg_durable:latest", registry)
        }
    }
}

pub async fn activity(
    ctx: ActivityContext,
    input: DeployPostgresInput,
) -> Result<DeployPostgresOutput, String> {
    let image_type_str = input.image_type.as_str();
    ctx.trace_info(format!("Deploying PostgreSQL: {} (image: {})", input.instance_name, image_type_str));
    
    // 2. Get K8s client
    let client = get_k8s_client().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;
    
    // 3. Check idempotency - do resources already exist?
    let already_exists = check_resources_exist(&client, &input.namespace, &input.instance_name).await
        .map_err(|e| format!("Failed to check if resources exist: {}", e))?;
    
    if already_exists {
        ctx.trace_info("Resources already exist, skipping creation");
        return Ok(DeployPostgresOutput {
            instance_name: input.instance_name,
            namespace: input.namespace,
            created: false,
        });
    }
    
    // 4. Create resources using templates
    create_k8s_resources(&client, &input, &ctx).await
        .map_err(|e| format!("Failed to create K8s resources: {}", e))?;
    
    ctx.trace_info("PostgreSQL deployment complete");
    
    // 5. Return output
    Ok(DeployPostgresOutput {
        instance_name: input.instance_name,
        namespace: input.namespace,
        created: true,
    })
}

async fn create_k8s_resources(
    client: &kube::Client,
    input: &DeployPostgresInput,
    ctx: &ActivityContext,
) -> anyhow::Result<()> {
    // Initialize template engine
    let mut tera = Tera::default();
    
    // Load templates
    let secret_template = include_str!("../templates/postgres-secret.yaml");
    let config_template = include_str!("../templates/postgres-config.yaml");
    let pvc_template = include_str!("../templates/postgres-pvc.yaml");
    let statefulset_template = include_str!("../templates/postgres-statefulset.yaml");
    let service_template = include_str!("../templates/postgres-service.yaml");
    
    tera.add_raw_template("secret", secret_template)?;
    tera.add_raw_template("config", config_template)?;
    tera.add_raw_template("pvc", pvc_template)?;
    tera.add_raw_template("statefulset", statefulset_template)?;
    tera.add_raw_template("service", service_template)?;
    
    // Determine the image to use
    let image = get_image(input);
    ctx.trace_info(format!("Using image: {}", image));
    
    // Prepare template context
    let mut template_ctx = TeraContext::new();
    template_ctx.insert("name", &input.instance_name);
    template_ctx.insert("namespace", &input.namespace);
    template_ctx.insert("password", &input.password);
    template_ctx.insert("storage_size", &input.storage_size_gb);
    template_ctx.insert("postgres_version", &input.postgres_version);
    template_ctx.insert("service_type", if input.use_load_balancer { "LoadBalancer" } else { "ClusterIP" });
    template_ctx.insert("dns_label", &input.dns_label.as_deref().unwrap_or(""));
    template_ctx.insert("image", &image);
    template_ctx.insert("image_type", input.image_type.as_str());
    
    // 1. Create Secret for password
    ctx.trace_info("Creating Secret");
    let secret_yaml = tera.render("secret", &template_ctx)?;
    let secret: Secret = serde_yaml::from_str(&secret_yaml)?;
    
    let secrets: Api<Secret> = Api::namespaced(client.clone(), &input.namespace);
    secrets.create(&PostParams::default(), &secret).await?;
    ctx.trace_info("Secret created");
    
    // 2. Create ConfigMap for pg_hba.conf (enables replication for backups)
    ctx.trace_info("Creating ConfigMap");
    let config_yaml = tera.render("config", &template_ctx)?;
    let configmap: ConfigMap = serde_yaml::from_str(&config_yaml)?;
    
    let configmaps: Api<ConfigMap> = Api::namespaced(client.clone(), &input.namespace);
    configmaps.create(&PostParams::default(), &configmap).await?;
    ctx.trace_info("ConfigMap created");
    
    // 3. Create PersistentVolumeClaim
    ctx.trace_info("Creating PersistentVolumeClaim");
    let pvc_yaml = tera.render("pvc", &template_ctx)?;
    let pvc: PersistentVolumeClaim = serde_yaml::from_str(&pvc_yaml)?;
    
    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), &input.namespace);
    pvcs.create(&PostParams::default(), &pvc).await?;
    ctx.trace_info("PersistentVolumeClaim created");
    
    // 4. Create StatefulSet
    ctx.trace_info("Creating StatefulSet");
    let statefulset_yaml = tera.render("statefulset", &template_ctx)?;
    let statefulset: StatefulSet = serde_yaml::from_str(&statefulset_yaml)?;
    
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), &input.namespace);
    statefulsets.create(&PostParams::default(), &statefulset).await?;
    ctx.trace_info("StatefulSet created");
    
    // 5. Create Service
    ctx.trace_info("Creating Service");
    let service_yaml = tera.render("service", &template_ctx)?;
    let service: Service = serde_yaml::from_str(&service_yaml)?;
    
    let services: Api<Service> = Api::namespaced(client.clone(), &input.namespace);
    services.create(&PostParams::default(), &service).await?;
    ctx.trace_info("Service created");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_deploy_postgres_input_serialization() {
        let input = DeployPostgresInput {
            namespace: "test".to_string(),
            instance_name: "test-pg".to_string(),
            password: "password123".to_string(),
            postgres_version: "18".to_string(),
            storage_size_gb: 10,
            use_load_balancer: true,
            dns_label: Some("testlabel".to_string()),
            image_type: ImageType::Stock,
            image_registry: None,
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: DeployPostgresInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
    }
    
    #[test]
    fn test_deploy_postgres_input_pg_durable() {
        let input = DeployPostgresInput {
            namespace: "test".to_string(),
            instance_name: "test-pg-durable".to_string(),
            password: "password123".to_string(),
            postgres_version: "16".to_string(),
            storage_size_gb: 20,
            use_load_balancer: true,
            dns_label: Some("testlabel".to_string()),
            image_type: ImageType::PgDurable,
            image_registry: Some("myacr.azurecr.io".to_string()),
        };
        
        let image = get_image(&input);
        assert_eq!(image, "myacr.azurecr.io/pg_durable:latest");
    }
    
    #[test]
    fn test_deploy_postgres_output_serialization() {
        let output = DeployPostgresOutput {
            instance_name: "test-pg".to_string(),
            namespace: "test".to_string(),
            created: true,
        };
        
        let json = serde_json::to_string(&output).unwrap();
        let parsed: DeployPostgresOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }
}

