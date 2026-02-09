//! Deploy PostgreSQL StatefulSet and Service using an existing PVC.
//!
//! This activity is used during restore operations when the PVC has
//! already been created and populated with backup data.

use duroxide::ActivityContext;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::{Service, Secret, ConfigMap};
use kube::api::{Api, PostParams};
use kube::Client;
use serde::{Deserialize, Serialize};
use tera::{Context as TeraContext, Tera};

use crate::activity_types::ImageType;

pub const NAME: &str = "toygres-orchestrations::activity::deploy-postgres-from-pvc";

/// Default ACR for pg_durable images
const DEFAULT_PG_DURABLE_REGISTRY: &str = "toygresaksacr.azurecr.io";

/// Input for the deploy_postgres_from_pvc activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployPostgresFromPvcInput {
    /// Instance name
    pub instance_name: String,
    /// Namespace for deployment
    pub namespace: String,
    /// PostgreSQL version (e.g., "17", "16")
    pub postgres_version: String,
    /// Password for the postgres user
    pub password: String,
    /// Whether to use LoadBalancer for external access
    pub use_load_balancer: bool,
    /// DNS label for the LoadBalancer (Azure-specific)
    pub dns_label: Option<String>,
    /// Type of PostgreSQL image to use
    pub image_type: ImageType,
    /// Custom image registry (for pg_durable)
    pub image_registry: Option<String>,
    /// Name of the existing PVC to use
    pub pvc_name: String,
}

/// Output for the deploy_postgres_from_pvc activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployPostgresFromPvcOutput {
    /// Instance name
    pub instance_name: String,
    /// Namespace
    pub namespace: String,
    /// Whether resources were created (false if already existed)
    pub created: bool,
}

/// Get the Docker image based on image type
fn get_image(input: &DeployPostgresFromPvcInput) -> String {
    match input.image_type {
        ImageType::Stock => {
            format!("postgres:{}", input.postgres_version)
        }
        ImageType::PgDurable => {
            let registry = input
                .image_registry
                .as_deref()
                .unwrap_or(DEFAULT_PG_DURABLE_REGISTRY);
            format!("{}/pg_durable:latest", registry)
        }
    }
}

pub async fn activity(
    ctx: ActivityContext,
    input: DeployPostgresFromPvcInput,
) -> Result<DeployPostgresFromPvcOutput, String> {
    ctx.trace_info(format!(
        "Deploying PostgreSQL from existing PVC: {} (pvc: {})",
        input.instance_name, input.pvc_name
    ));

    let client = Client::try_default()
        .await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;

    // Check if StatefulSet already exists (idempotency)
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), &input.namespace);
    match statefulsets.get(&input.instance_name).await {
        Ok(_) => {
            ctx.trace_info("StatefulSet already exists, skipping creation");
            return Ok(DeployPostgresFromPvcOutput {
                instance_name: input.instance_name,
                namespace: input.namespace,
                created: false,
            });
        }
        Err(kube::Error::Api(e)) if e.code == 404 => {
            // StatefulSet doesn't exist, proceed
        }
        Err(e) => {
            return Err(format!("Failed to check if StatefulSet exists: {}", e));
        }
    }

    // Load templates
    let mut tera = Tera::default();
    let secret_template = include_str!("../templates/postgres-secret.yaml");
    let config_template = include_str!("../templates/postgres-config.yaml");
    let statefulset_template = include_str!("../templates/postgres-statefulset.yaml");
    let service_template = include_str!("../templates/postgres-service.yaml");

    tera.add_raw_template("secret", secret_template)
        .map_err(|e| format!("Failed to parse Secret template: {}", e))?;
    tera.add_raw_template("config", config_template)
        .map_err(|e| format!("Failed to parse ConfigMap template: {}", e))?;
    tera.add_raw_template("statefulset", statefulset_template)
        .map_err(|e| format!("Failed to parse StatefulSet template: {}", e))?;
    tera.add_raw_template("service", service_template)
        .map_err(|e| format!("Failed to parse Service template: {}", e))?;

    // Determine the image to use
    let image = get_image(&input);
    ctx.trace_info(format!("Using image: {}", image));

    // Prepare template context
    let mut template_ctx = TeraContext::new();
    template_ctx.insert("name", &input.instance_name);
    template_ctx.insert("namespace", &input.namespace);
    template_ctx.insert("password", &input.password);
    template_ctx.insert("postgres_version", &input.postgres_version);
    template_ctx.insert(
        "service_type",
        if input.use_load_balancer {
            "LoadBalancer"
        } else {
            "ClusterIP"
        },
    );
    template_ctx.insert("dns_label", &input.dns_label.as_deref().unwrap_or(""));
    template_ctx.insert("image", &image);
    template_ctx.insert("image_type", input.image_type.as_str());
    // Storage size not needed since we're using existing PVC
    template_ctx.insert("storage_size", &10_u32); // Placeholder, not used

    // Create Secret for password
    ctx.trace_info("Creating Secret");
    let secret_yaml = tera
        .render("secret", &template_ctx)
        .map_err(|e| format!("Failed to render Secret template: {}", e))?;
    let secret: Secret = serde_yaml::from_str(&secret_yaml)
        .map_err(|e| format!("Failed to parse Secret YAML: {}", e))?;

    let secrets: Api<Secret> = Api::namespaced(client.clone(), &input.namespace);
    secrets
        .create(&PostParams::default(), &secret)
        .await
        .map_err(|e| format!("Failed to create Secret: {}", e))?;
    ctx.trace_info("Secret created");

    // Create ConfigMap for pg_hba.conf
    ctx.trace_info("Creating ConfigMap");
    let config_yaml = tera
        .render("config", &template_ctx)
        .map_err(|e| format!("Failed to render ConfigMap template: {}", e))?;
    let configmap: ConfigMap = serde_yaml::from_str(&config_yaml)
        .map_err(|e| format!("Failed to parse ConfigMap YAML: {}", e))?;

    let configmaps: Api<ConfigMap> = Api::namespaced(client.clone(), &input.namespace);
    configmaps
        .create(&PostParams::default(), &configmap)
        .await
        .map_err(|e| format!("Failed to create ConfigMap: {}", e))?;
    ctx.trace_info("ConfigMap created");

    // Create StatefulSet
    ctx.trace_info("Creating StatefulSet");
    let statefulset_yaml = tera
        .render("statefulset", &template_ctx)
        .map_err(|e| format!("Failed to render StatefulSet template: {}", e))?;
    let statefulset: StatefulSet = serde_yaml::from_str(&statefulset_yaml)
        .map_err(|e| format!("Failed to parse StatefulSet YAML: {}", e))?;

    statefulsets
        .create(&PostParams::default(), &statefulset)
        .await
        .map_err(|e| format!("Failed to create StatefulSet: {}", e))?;
    ctx.trace_info("StatefulSet created");

    // Create Service
    ctx.trace_info("Creating Service");
    let services: Api<Service> = Api::namespaced(client, &input.namespace);
    let service_yaml = tera
        .render("service", &template_ctx)
        .map_err(|e| format!("Failed to render Service template: {}", e))?;
    let service: Service = serde_yaml::from_str(&service_yaml)
        .map_err(|e| format!("Failed to parse Service YAML: {}", e))?;

    services
        .create(&PostParams::default(), &service)
        .await
        .map_err(|e| format!("Failed to create Service: {}", e))?;
    ctx.trace_info("Service created");

    Ok(DeployPostgresFromPvcOutput {
        instance_name: input.instance_name,
        namespace: input.namespace,
        created: true,
    })
}
