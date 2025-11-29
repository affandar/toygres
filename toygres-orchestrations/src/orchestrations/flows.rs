//! Static flow diagrams for orchestrations
//!
//! These Mermaid diagrams represent the expected flow of each orchestration.
//! They can be used by the UI to show execution progress against the expected flow.

/// Node IDs map to activity names for matching against execution history
pub struct FlowDiagram {
    /// The orchestration name this flow belongs to
    pub orchestration_name: &'static str,
    /// Mermaid flowchart definition
    pub mermaid: &'static str,
    /// Mapping of node IDs to activity name patterns (for matching history events)
    pub node_mappings: &'static [(&'static str, &'static str)],
}

/// Create Instance orchestration flow
pub const CREATE_INSTANCE_FLOW: FlowDiagram = FlowDiagram {
    orchestration_name: "toygres-orchestrations::orchestration::create-instance",
    mermaid: r#"flowchart TD
    subgraph init["Initialization"]
        start(["▶ Start"])
        cms_record["📋 Create CMS Record<br/><small>Reserve DNS name</small>"]
    end

    subgraph deploy["Deploy to Kubernetes"]
        deploy_k8s["📋 Deploy PostgreSQL<br/><small>PVC + StatefulSet + Service</small>"]
        wait_ready{"⏳ Pod Ready?"}
        timer_wait["⏱ Wait 5s"]
        timeout_check{"Attempt < 60?"}
        timeout_fail(["💥 Timeout"])
    end

    subgraph connect["Connection Setup"]
        get_conn["📋 Get Connection Strings<br/><small>with retry (5x)</small>"]
        test_conn["📋 Test Connection<br/><small>with retry (5x)</small>"]
    end

    subgraph finalize["Finalize"]
        update_running["📋 Update State: Running"]
        start_actor["📦 Start Instance Actor"]
        record_actor["📋 Record Actor ID"]
        success(["🏁 Success"])
    end

    subgraph failure["Failure Path"]
        mark_failed["📋 Mark Failed"]
        free_dns["📋 Free DNS Name"]
        cleanup["📦 Cleanup Sub-Orch"]
        failed(["💥 Failed"])
    end

    start --> cms_record
    cms_record --> deploy_k8s
    deploy_k8s --> wait_ready
    wait_ready -->|No| timeout_check
    timeout_check -->|Yes| timer_wait
    timer_wait --> wait_ready
    timeout_check -->|No| timeout_fail
    timeout_fail --> mark_failed
    wait_ready -->|Yes| get_conn
    get_conn --> test_conn
    test_conn -->|Success| update_running
    test_conn -->|Fail| mark_failed
    update_running --> start_actor
    start_actor --> record_actor
    record_actor --> success
    mark_failed --> free_dns
    free_dns --> cleanup
    cleanup --> failed

    classDef activity fill:#3b82f6,color:#fff,stroke:#1d4ed8
    classDef timer fill:#06b6d4,color:#fff,stroke:#0891b2
    classDef decision fill:#f59e0b,color:#000,stroke:#d97706
    classDef success fill:#22c55e,color:#fff,stroke:#16a34a
    classDef failure fill:#ef4444,color:#fff,stroke:#dc2626
    classDef suborg fill:#8b5cf6,color:#fff,stroke:#7c3aed
    classDef start fill:#a855f7,color:#fff,stroke:#9333ea

    class start start
    class cms_record,deploy_k8s,get_conn,test_conn,update_running,record_actor,mark_failed,free_dns activity
    class timer_wait timer
    class wait_ready,timeout_check decision
    class success success
    class timeout_fail,failed failure
    class cleanup,start_actor suborg"#,
    node_mappings: &[
        ("cms_record", "cms-create-instance-record"),
        ("deploy_k8s", "deploy-postgres"),
        ("wait_ready", "wait-for-ready"),
        ("get_conn", "get-connection-strings"),
        ("test_conn", "test-connection"),
        ("update_running", "cms-update-instance-state"),
        ("start_actor", "instance-actor"),
        ("record_actor", "cms-record-instance-actor"),
        ("mark_failed", "cms-update-instance-state"),
        ("free_dns", "cms-free-dns-name"),
        ("cleanup", "delete-instance"),
    ],
};

/// Delete Instance orchestration flow
pub const DELETE_INSTANCE_FLOW: FlowDiagram = FlowDiagram {
    orchestration_name: "toygres-orchestrations::orchestration::delete-instance",
    mermaid: r#"flowchart TD
    subgraph init["Initialization"]
        start(["▶ Start"])
        get_cms["📋 Get CMS Record<br/><small>with retry (3x)</small>"]
        check_found{"Record Found?"}
        mark_deleting["📋 Mark State: Deleting"]
    end

    subgraph delete["Delete Resources"]
        delete_k8s["📋 Delete K8s Resources<br/><small>with retry (3x)</small>"]
        mark_deleted["📋 Mark State: Deleted"]
    end

    subgraph cleanup["Cleanup"]
        delete_record["📋 Delete CMS Record"]
        free_dns["📋 Free DNS Name"]
        success(["🏁 Success"])
    end

    start --> get_cms
    get_cms --> check_found
    check_found -->|Yes| mark_deleting
    check_found -->|No| delete_k8s
    mark_deleting --> delete_k8s
    delete_k8s --> mark_deleted
    mark_deleted --> delete_record
    delete_record --> free_dns
    free_dns --> success

    classDef activity fill:#3b82f6,color:#fff,stroke:#1d4ed8
    classDef decision fill:#f59e0b,color:#000,stroke:#d97706
    classDef success fill:#22c55e,color:#fff,stroke:#16a34a
    classDef start fill:#a855f7,color:#fff,stroke:#9333ea

    class start start
    class get_cms,mark_deleting,delete_k8s,mark_deleted,delete_record,free_dns activity
    class check_found decision
    class success success"#,
    node_mappings: &[
        ("get_cms", "cms-get-instance-by-k8s-name"),
        ("mark_deleting", "cms-update-instance-state"),
        ("delete_k8s", "delete-postgres"),
        ("mark_deleted", "cms-update-instance-state"),
        ("delete_record", "cms-delete-instance-record"),
        ("free_dns", "cms-free-dns-name"),
    ],
};

/// Instance Actor orchestration flow (single iteration)
pub const INSTANCE_ACTOR_FLOW: FlowDiagram = FlowDiagram {
    orchestration_name: "toygres-orchestrations::orchestration::instance-actor",
    mermaid: r#"flowchart TD
    subgraph health["Health Check Iteration"]
        start(["▶ Start Iteration"])
        get_conn["📋 Get Instance Connection<br/><small>with retry (3x)</small>"]
        check_exists{"Instance Exists?"}
        check_conn{"Has Connection<br/>String?"}
        test_conn["📋 Test Connection<br/><small>with retry (3x)</small>"]
        record_health["📋 Record Health Check"]
        update_health["📋 Update Health Status"]
    end

    subgraph wait["Wait for Next Cycle"]
        race{{"⚡ Race"}}
        timer["⏱ Wait 30s"]
        deletion_signal["⏳ Wait: InstanceDeleted"]
    end

    subgraph exit["Exit Conditions"]
        not_found(["🏁 Instance Gone"])
        deleted(["🏁 Deletion Signal"])
        continue_new(["🔄 Continue As New"])
        no_conn_continue(["🔄 Continue As New<br/><small>No connection yet</small>"])
    end

    start --> get_conn
    get_conn --> check_exists
    check_exists -->|No| not_found
    check_exists -->|Yes| check_conn
    check_conn -->|No| no_conn_continue
    check_conn -->|Yes| test_conn
    test_conn --> record_health
    record_health --> update_health
    update_health --> race
    race --> timer
    race --> deletion_signal
    timer -->|Winner| continue_new
    deletion_signal -->|Winner| deleted

    classDef activity fill:#3b82f6,color:#fff,stroke:#1d4ed8
    classDef timer fill:#06b6d4,color:#fff,stroke:#0891b2
    classDef decision fill:#f59e0b,color:#000,stroke:#d97706
    classDef success fill:#22c55e,color:#fff,stroke:#16a34a
    classDef continue fill:#a855f7,color:#fff,stroke:#9333ea
    classDef start fill:#a855f7,color:#fff,stroke:#9333ea
    classDef race fill:#ec4899,color:#fff,stroke:#db2777
    classDef wait fill:#eab308,color:#000,stroke:#ca8a04

    class start start
    class get_conn,test_conn,record_health,update_health activity
    class timer timer
    class check_exists,check_conn decision
    class not_found,deleted success
    class continue_new,no_conn_continue continue
    class race race
    class deletion_signal wait"#,
    node_mappings: &[
        ("get_conn", "cms-get-instance-connection"),
        ("test_conn", "test-connection"),
        ("record_health", "cms-record-health-check"),
        ("update_health", "cms-update-instance-health"),
    ],
};

/// Get all flow diagrams
pub fn get_all_flows() -> Vec<&'static FlowDiagram> {
    vec![
        &CREATE_INSTANCE_FLOW,
        &DELETE_INSTANCE_FLOW,
        &INSTANCE_ACTOR_FLOW,
    ]
}

/// Get flow diagram by orchestration name
pub fn get_flow_by_name(name: &str) -> Option<&'static FlowDiagram> {
    // Match by full name or short name
    let short_name = name.split("::").last().unwrap_or(name);
    
    match short_name {
        "create-instance" => Some(&CREATE_INSTANCE_FLOW),
        "delete-instance" => Some(&DELETE_INSTANCE_FLOW),
        "instance-actor" => Some(&INSTANCE_ACTOR_FLOW),
        _ => {
            // Try full name match
            if name.contains("create-instance") {
                Some(&CREATE_INSTANCE_FLOW)
            } else if name.contains("delete-instance") {
                Some(&DELETE_INSTANCE_FLOW)
            } else if name.contains("instance-actor") {
                Some(&INSTANCE_ACTOR_FLOW)
            } else {
                None
            }
        }
    }
}

