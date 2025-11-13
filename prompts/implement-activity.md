# Prompt: Implement Duroxide Activity

## Context

I'm working on the Toygres project, a Rust-based control plane for hosting PostgreSQL containers on AKS using the Duroxide durable workflow framework.

**Project Structure**: Cargo workspace with `toygres-models`, `toygres-activities`, `toygres-orchestrations`, and `toygres-server` crates.

**What are Activities?** Activities in Duroxide are atomic, retriable operations. They should:
- Be idempotent when possible
- Handle errors gracefully
- Use `anyhow::Result` for error handling
- Be async functions
- Be registered using Duroxide's activity registration system

## Task

Implement the `[ACTIVITY_NAME]` in `toygres-activities/src/[file_name].rs`.

**Purpose**: [Describe what this activity does]

**Inputs**: [List input parameters and types]

**Outputs**: [Describe return type]

**Dependencies**:
- Access to Kubernetes API (via `kube` crate)
- Access to metadata database (via `sqlx`)
- Shared models from `toygres-models`

## Requirements

1. Define the activity input/output structs with `Serialize`/`Deserialize`
2. Implement the activity function with proper error handling
3. Make it idempotent where possible
4. Add appropriate logging with `tracing`
5. Follow the Duroxide activity pattern (see their docs)
6. Export the activity from `toygres-activities/src/lib.rs`

## Example Pattern

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyActivityInput {
    pub param1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyActivityOutput {
    pub result: String,
}

pub struct MyActivity;

// Implement according to Duroxide's activity trait/pattern
pub async fn my_activity(input: MyActivityInput) -> Result<MyActivityOutput> {
    info!("Executing MyActivity with input: {:?}", input);
    
    // Activity logic here
    
    Ok(MyActivityOutput {
        result: "success".to_string(),
    })
}
```

## Additional Context

[Add any specific details about the activity you're implementing, such as K8s resources to create, database queries to run, etc.]

