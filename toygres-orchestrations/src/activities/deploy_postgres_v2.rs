//! Deploy PostgreSQL activity (v2 - supports image override)

use duroxide::ActivityContext;
use crate::activity_types::{DeployPostgresV2Input, DeployPostgresOutput, ImageType};
use crate::k8s_client::{get_k8s_client, check_resources_exist};
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Service, Secret, ConfigMap};
use k8s_openapi::api::apps::v1::StatefulSet;
use kube::api::{Api, PostParams};
use tera::{Tera, Context as TeraContext};

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::deploy-postgres-v2";

/// Default ACR for pg_durable images
const DEFAULT_PG_DURABLE_REGISTRY: &str = "toygresaksacr.azurecr.io";

/// Get the Docker image based on image type or override
fn get_image(input: &DeployPostgresV2Input) -> String {
    if let Some(image_override) = &input.image_override {
        return image_override.clone();
    }

    match input.image_type {
        ImageType::Stock => format!("postgres:{}", input.postgres_version),
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
    input: DeployPostgresV2Input,
) -> Result<DeployPostgresOutput, String> {
    let image_type_str = input.image_type.as_str();
    ctx.trace_info(format!(
        "[v2] Deploying PostgreSQL: {} (image_type: {})",
        input.instance_name, image_type_str
    ));

    let client = get_k8s_client().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;

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

    create_k8s_resources(&client, &input, &ctx).await
        .map_err(|e| format!("Failed to create K8s resources: {}", e))?;

    ctx.trace_info("[v2] PostgreSQL deployment complete");

    Ok(DeployPostgresOutput {
        instance_name: input.instance_name,
        namespace: input.namespace,
        created: true,
    })
}

async fn create_k8s_resources(
    client: &kube::Client,
    input: &DeployPostgresV2Input,
    ctx: &ActivityContext,
) -> anyhow::Result<()> {
    let mut tera = Tera::default();

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

    let image = get_image(input);
    ctx.trace_info(format!("[v2] Using image: {}", image));

    let acr_host = std::env::var("TOYGRES_ACR_HOST").unwrap_or_else(|_| "toygresaksacr.azurecr.io".to_string());
    let needs_acr_secret = image.starts_with(&format!("{}/", acr_host));

    let mut template_ctx = TeraContext::new();
    template_ctx.insert("name", &input.instance_name);
    template_ctx.insert("namespace", &input.namespace);
    template_ctx.insert("password", &input.password);
    template_ctx.insert("storage_size", &input.storage_size_gb);
    template_ctx.insert("postgres_version", &input.postgres_version);
    template_ctx.insert(
        "service_type",
        if input.use_load_balancer { "LoadBalancer" } else { "ClusterIP" },
    );
    template_ctx.insert("dns_label", &input.dns_label.as_deref().unwrap_or(""));
    template_ctx.insert("image", &image);
    template_ctx.insert("image_type", input.image_type.as_str());
    template_ctx.insert("needs_acr_secret", &needs_acr_secret);

    ctx.trace_info("Creating Secret");
    let secret_yaml = tera.render("secret", &template_ctx)?;
    let secret: Secret = serde_yaml::from_str(&secret_yaml)?;
    let secrets: Api<Secret> = Api::namespaced(client.clone(), &input.namespace);
    secrets.create(&PostParams::default(), &secret).await?;

    ctx.trace_info("Creating ConfigMap");
    let config_yaml = tera.render("config", &template_ctx)?;
    let configmap: ConfigMap = serde_yaml::from_str(&config_yaml)?;
    let configmaps: Api<ConfigMap> = Api::namespaced(client.clone(), &input.namespace);
    configmaps.create(&PostParams::default(), &configmap).await?;

    ctx.trace_info("Creating PersistentVolumeClaim");
    let pvc_yaml = tera.render("pvc", &template_ctx)?;
    let pvc: PersistentVolumeClaim = serde_yaml::from_str(&pvc_yaml)?;
    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), &input.namespace);
    pvcs.create(&PostParams::default(), &pvc).await?;

    ctx.trace_info("Creating StatefulSet");
    let statefulset_yaml = tera.render("statefulset", &template_ctx)?;
    let statefulset: StatefulSet = serde_yaml::from_str(&statefulset_yaml)?;
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), &input.namespace);
    statefulsets.create(&PostParams::default(), &statefulset).await?;

    ctx.trace_info("Creating Service");
    let service_yaml = tera.render("service", &template_ctx)?;
    let service: Service = serde_yaml::from_str(&service_yaml)?;
    let services: Api<Service> = Api::namespaced(client.clone(), &input.namespace);
    services.create(&PostParams::default(), &service).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_override_takes_precedence() {
        let input = DeployPostgresV2Input {
            namespace: "test".to_string(),
            instance_name: "test-pg".to_string(),
            password: "password".to_string(),
            postgres_version: "18".to_string(),
            storage_size_gb: 10,
            use_load_balancer: true,
            dns_label: None,
            image_type: ImageType::Stock,
            image_registry: None,
            image_override: Some("toygresacr.azurecr.io/custom@sha256:deadbeef".to_string()),
        };

        assert_eq!(get_image(&input), "toygresacr.azurecr.io/custom@sha256:deadbeef");
    }
}
