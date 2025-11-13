//! Delete PostgreSQL instance orchestration

use duroxide::OrchestrationContext;
use crate::types::{DeleteInstanceInput, DeleteInstanceOutput};
use crate::activity_names::activities;
use crate::activity_types::{DeletePostgresInput, DeletePostgresOutput};

pub async fn delete_instance_orchestration(
    ctx: OrchestrationContext,
    input: DeleteInstanceInput,
) -> Result<DeleteInstanceOutput, String> {
    ctx.trace_info(format!("Deleting PostgreSQL instance: {}", input.name));
    
    let namespace = input.namespace.clone().unwrap_or_else(|| "toygres".to_string());
    
    // Step 1: Delete PostgreSQL resources
    ctx.trace_info("Step 1: Deleting PostgreSQL from Kubernetes");
    let delete_input = DeletePostgresInput {
        namespace: namespace.clone(),
        instance_name: input.name.clone(),
    };
    
    let delete_output = ctx
        .schedule_activity_typed::<DeletePostgresInput, DeletePostgresOutput>(activities::DELETE_POSTGRES, &delete_input)
        .into_activity_typed::<DeletePostgresOutput>()
        .await?;
    
    ctx.trace_info(format!("Instance deletion complete (deleted: {})", delete_output.deleted));
    
    // Return output
    Ok(DeleteInstanceOutput {
        instance_name: input.name,
        deleted: delete_output.deleted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_delete_instance_input_serialization() {
        let input = DeleteInstanceInput {
            name: "test-pg".to_string(),
            namespace: Some("toygres".to_string()),
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: DeleteInstanceInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
    }
    
    #[test]
    fn test_delete_instance_output_serialization() {
        let output = DeleteInstanceOutput {
            instance_name: "test-pg".to_string(),
            deleted: true,
        };
        
        let json = serde_json::to_string(&output).unwrap();
        let parsed: DeleteInstanceOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }
}

