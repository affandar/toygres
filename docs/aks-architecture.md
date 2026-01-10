# Toygres AKS Architecture

## Overview

Toygres is a PostgreSQL-as-a-Service platform running on Azure Kubernetes Service (AKS). This document provides a detailed architecture diagram and pseudo code for all key workflows.

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                    AZURE CLOUD                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                          │
│  ┌─────────────────────────────────────────────────────────────────────────────────┐    │
│  │                         AKS CLUSTER (toygres-aks)                                │    │
│  ├─────────────────────────────────────────────────────────────────────────────────┤    │
│  │                                                                                  │    │
│  │  ┌─────────────────────────────────────────────────────────────────────────┐    │    │
│  │  │                    NAMESPACE: toygres-system                             │    │    │
│  │  │                    (Control Plane Components)                            │    │    │
│  │  ├─────────────────────────────────────────────────────────────────────────┤    │    │
│  │  │                                                                          │    │    │
│  │  │  ┌─────────────────────────────────────────────────────────────────┐    │    │    │
│  │  │  │           toygres-server (Deployment, replicas: 1)              │    │    │    │
│  │  │  │  ─────────────────────────────────────────────────────────────  │    │    │    │
│  │  │  │  • API Server (Axum, port 8080)                                 │    │    │    │
│  │  │  │  • Duroxide Runtime (orchestration engine)                      │    │    │    │
│  │  │  │  • Activity Workers (K8s ops, CMS ops)                          │    │    │    │
│  │  │  │  • Static file server (React UI)                                │    │    │    │
│  │  │  └───────────────────────────────────────────────────┬─────────────┘    │    │    │
│  │  │                                                      │                   │    │    │
│  │  │  ┌─────────────────────────────────────┐     ┌──────▼──────────────┐    │    │    │
│  │  │  │ toygres-config (ConfigMap)          │     │  toygres-secrets    │    │    │    │
│  │  │  │ ─────────────────────────           │     │  (Secret)           │    │    │    │
│  │  │  │ • AKS_NAMESPACE: toygres            │     │  ────────────────   │    │    │    │
│  │  │  │ • AZURE_STORAGE_ACCOUNT             │     │  • CMS_DATABASE_URL │    │    │    │
│  │  │  │ • AZURE_STORAGE_CONTAINER           │     │  • JWT_SECRET       │    │    │    │
│  │  │  │ • RUST_LOG                          │     └─────────────────────┘    │    │    │
│  │  │  └─────────────────────────────────────┘                                │    │    │
│  │  │                                                                          │    │    │
│  │  │  ┌─────────────────────────────────────────────────────────────────┐    │    │    │
│  │  │  │               toygres-server (ServiceAccount)                    │    │    │    │
│  │  │  │  ─────────────────────────────────────────────────────────────  │    │    │    │
│  │  │  │  Annotations:                                                    │    │    │    │
│  │  │  │    azure.workload.identity/client-id: <STORAGE_CLIENT_ID>       │    │    │    │
│  │  │  │  ClusterRole: toygres-server                                     │    │    │    │
│  │  │  │    • pods, services, pvc, statefulsets, secrets, configmaps     │    │    │    │
│  │  │  │    • jobs (batch/v1)                                             │    │    │    │
│  │  │  └─────────────────────────────────────────────────────────────────┘    │    │    │
│  │  │                                                                          │    │    │
│  │  └─────────────────────────────────────────────────────────────────────────┘    │    │
│  │                                                                                  │    │
│  │  ┌─────────────────────────────────────────────────────────────────────────┐    │    │
│  │  │                    NAMESPACE: toygres                                    │    │    │
│  │  │                    (PostgreSQL Instances)                                │    │    │
│  │  ├─────────────────────────────────────────────────────────────────────────┤    │    │
│  │  │                                                                          │    │    │
│  │  │  ┌───────────────────────┐  ┌───────────────────────┐                   │    │    │
│  │  │  │ my-instance-1         │  │ my-instance-2         │    ...            │    │    │
│  │  │  │ (StatefulSet)         │  │ (StatefulSet)         │                   │    │    │
│  │  │  │ ───────────────────   │  │ ───────────────────   │                   │    │    │
│  │  │  │ postgres:17           │  │ pg_durable:18         │                   │    │    │
│  │  │  │ replicas: 1           │  │ replicas: 1           │                   │    │    │
│  │  │  └───────────┬───────────┘  └───────────┬───────────┘                   │    │    │
│  │  │              │                          │                                │    │    │
│  │  │  ┌───────────┴───────────┐  ┌───────────┴───────────┐                   │    │    │
│  │  │  │ my-instance-1-svc     │  │ my-instance-2-svc     │                   │    │    │
│  │  │  │ (Service/LoadBalancer)│  │ (Service/ClusterIP)   │                   │    │    │
│  │  │  │ ───────────────────   │  │ ───────────────────   │                   │    │    │
│  │  │  │ External IP + DNS     │  │ Internal only         │                   │    │    │
│  │  │  │ port: 5432            │  │ port: 5432            │                   │    │    │
│  │  │  └───────────────────────┘  └───────────────────────┘                   │    │    │
│  │  │                                                                          │    │    │
│  │  │  ┌───────────────────────┐  ┌───────────────────────┐                   │    │    │
│  │  │  │ my-instance-1-pvc     │  │ my-instance-2-pvc     │                   │    │    │
│  │  │  │ (PVC, 10Gi)           │  │ (PVC, 50Gi)           │                   │    │    │
│  │  │  └───────────────────────┘  └───────────────────────┘                   │    │    │
│  │  │                                                                          │    │    │
│  │  │  ┌───────────────────────┐  ┌───────────────────────┐                   │    │    │
│  │  │  │ my-instance-1-secret  │  │ my-instance-2-secret  │                   │    │    │
│  │  │  │ (Secret)              │  │ (Secret)              │                   │    │    │
│  │  │  │ POSTGRES_PASSWORD     │  │ POSTGRES_PASSWORD     │                   │    │    │
│  │  │  └───────────────────────┘  └───────────────────────┘                   │    │    │
│  │  │                                                                          │    │    │
│  │  │  ┌───────────────────────┐  ┌───────────────────────┐                   │    │    │
│  │  │  │ my-instance-1-config  │  │ my-instance-2-config  │                   │    │    │
│  │  │  │ (ConfigMap)           │  │ (ConfigMap)           │                   │    │    │
│  │  │  │ pg_hba.conf           │  │ pg_hba.conf           │                   │    │    │
│  │  │  └───────────────────────┘  └───────────────────────┘                   │    │    │
│  │  │                                                                          │    │    │
│  │  │  ┌─────────────────────────────────────────────────────────────────┐    │    │    │
│  │  │  │                  Backup/Restore Jobs (ephemeral)                 │    │    │    │
│  │  │  │  ─────────────────────────────────────────────────────────────  │    │    │    │
│  │  │  │  backup-my-image-abc123 (Job)   restore-new-inst-def456 (Job)  │    │    │    │
│  │  │  │  - Uses workload identity for Azure Blob Storage access         │    │    │    │
│  │  │  │  - pg_basebackup → blob (backup)                                │    │    │    │
│  │  │  │  - blob → PVC (restore)                                         │    │    │    │
│  │  │  └─────────────────────────────────────────────────────────────────┘    │    │    │
│  │  │                                                                          │    │    │
│  │  └─────────────────────────────────────────────────────────────────────────┘    │    │
│  │                                                                                  │    │
│  └─────────────────────────────────────────────────────────────────────────────────┘    │
│                                                                                          │
│  ┌─────────────────────────────────────────────────────────────────────────────────┐    │
│  │                    AZURE MANAGED SERVICES                                        │    │
│  ├─────────────────────────────────────────────────────────────────────────────────┤    │
│  │                                                                                  │    │
│  │  ┌─────────────────────────────┐    ┌──────────────────────────────────────┐   │    │
│  │  │ Azure PostgreSQL Flexible   │    │ Azure Blob Storage                    │   │    │
│  │  │ (CMS Database)              │    │ (toygresstorage)                      │   │    │
│  │  │ ─────────────────────────   │    │ ────────────────────────────────────  │   │    │
│  │  │ Database: toygres_cms       │    │ Container: toygres-images             │   │    │
│  │  │ Tables:                     │    │ Blobs:                                │   │    │
│  │  │   • instances               │    │   • images/snapshot-1/base.tar.gz    │   │    │
│  │  │   • dns_names               │    │   • images/snapshot-2/base.tar.gz    │   │    │
│  │  │   • images                  │    │                                       │   │    │
│  │  │   • health_checks           │    │ Access: Workload Identity             │   │    │
│  │  │ + duroxide tables           │    │                                       │   │    │
│  │  └─────────────────────────────┘    └──────────────────────────────────────┘   │    │
│  │                                                                                  │    │
│  │  ┌─────────────────────────────┐    ┌──────────────────────────────────────┐   │    │
│  │  │ Azure Container Registry    │    │ Azure Managed Identity               │   │    │
│  │  │ (toygresacr)                │    │ ────────────────────────────────────  │   │    │
│  │  │ ─────────────────────────   │    │ toygres-storage-identity             │   │    │
│  │  │ Images:                     │    │   → Role: Storage Blob Data Contrib  │   │    │
│  │  │   • toygres-server          │    │   → Scope: toygresstorage            │   │    │
│  │  │   • toygres-backup          │    │                                       │   │    │
│  │  │   • pg_durable              │    │ Federated to:                        │   │    │
│  │  └─────────────────────────────┘    │   • toygres-server SA (toygres-system)│   │    │
│  │                                      │   • toygres-server SA (toygres)      │   │    │
│  │                                      └──────────────────────────────────────┘   │    │
│  │                                                                                  │    │
│  └─────────────────────────────────────────────────────────────────────────────────┘    │
│                                                                                          │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Component Details

### Control Plane (toygres-system namespace)

| Component | Type | Purpose |
|-----------|------|---------|
| toygres-server | Deployment | API + Duroxide runtime + Activity workers |
| toygres-server-svc | Service (ClusterIP) | Internal access to API |
| toygres-config | ConfigMap | Environment configuration |
| toygres-secrets | Secret | CMS connection string, JWT secret |
| toygres-server | ServiceAccount | RBAC + Workload Identity |

### Data Plane (toygres namespace)

| Component | Type | Purpose |
|-----------|------|---------|
| {name} | StatefulSet | PostgreSQL instance (1 replica) |
| {name}-svc | Service | External (LoadBalancer) or internal (ClusterIP) access |
| {name}-pvc | PVC | Persistent storage for PostgreSQL data |
| {name}-secret | Secret | POSTGRES_PASSWORD |
| {name}-config | ConfigMap | pg_hba.conf configuration |
| backup-{image}-* | Job | Backup operations (pg_basebackup → blob) |
| restore-{instance}-* | Job | Restore operations (blob → PVC) |

---

## Key Workflows

### 1. Create Instance (Empty)

```
┌──────────────────────────────────────────────────────────────────────────┐
│                     CREATE INSTANCE ORCHESTRATION                         │
│                     (create_instance v1.0.2)                              │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ PSEUDO CODE                                                               │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  1. VALIDATE INPUT                                                        │
│     └─ Check name format, storage size, postgres version                 │
│                                                                           │
│  2. CREATE CMS RECORD [Activity: cms::create_instance_record]            │
│     └─ INSERT INTO instances (name, state='creating', ...)               │
│                                                                           │
│  3. DEPLOY POSTGRES [Activity: deploy_postgres]                          │
│     ├─ Create Secret ({name}-secret) with POSTGRES_PASSWORD              │
│     ├─ Create ConfigMap ({name}-config) with pg_hba.conf                 │
│     ├─ Create PVC ({name}-pvc, size: storage_size_gb)                    │
│     ├─ Create StatefulSet ({name}) with postgres image                   │
│     └─ Create Service ({name}-svc, LoadBalancer or ClusterIP)            │
│                                                                           │
│  4. WAIT FOR READY [Activity: wait_for_ready]                            │
│     └─ Poll pod status until Running + Ready (timeout: 5 min)            │
│                                                                           │
│  5. GET CONNECTION STRINGS [Activity: get_connection_strings]            │
│     ├─ Wait for LoadBalancer external IP (if applicable)                 │
│     └─ Build connection strings (IP-based, DNS-based, internal)          │
│                                                                           │
│  6. TEST CONNECTION [Activity: test_connection]                          │
│     └─ Connect to postgres via internal connection string                │
│     └─ Execute: SELECT version()                                         │
│                                                                           │
│  7. UPDATE CMS TO RUNNING [Activity: cms::update_instance_state]         │
│     └─ UPDATE instances SET state='running', connection_strings=...      │
│                                                                           │
│  8. START INSTANCE ACTOR [Orchestration: start_orchestration]            │
│     └─ Spawn instance_actor orchestration for health monitoring          │
│                                                                           │
│  9. RECORD ACTOR ID [Activity: cms::record_instance_actor]               │
│     └─ UPDATE instances SET instance_actor_id=...                        │
│                                                                           │
│ 10. RETURN SUCCESS                                                        │
│     └─ {instance_id, k8s_name, connection_strings}                       │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

### 2. Delete Instance

```
┌──────────────────────────────────────────────────────────────────────────┐
│                     DELETE INSTANCE ORCHESTRATION                         │
│                     (delete_instance v1.0.2)                              │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ PSEUDO CODE                                                               │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  1. VALIDATE INSTANCE EXISTS [Activity: cms::get_instance_by_k8s_name]   │
│     └─ SELECT * FROM instances WHERE k8s_name = ?                        │
│                                                                           │
│  2. UPDATE CMS TO DELETING [Activity: cms::update_instance_state]        │
│     └─ UPDATE instances SET state='deleting'                             │
│                                                                           │
│  3. SIGNAL INSTANCE ACTOR [Activity: send_external_event] (v1.0.2+)      │
│     └─ raise_event(actor_orchestration_id, "InstanceDeleted", {})        │
│     └─ This allows actor to exit gracefully before K8s cleanup           │
│                                                                           │
│  4. DELETE K8S RESOURCES [Activity: delete_postgres]                     │
│     ├─ Delete StatefulSet ({name})                                       │
│     ├─ Delete Service ({name}-svc)                                       │
│     ├─ Delete PVC ({name}-pvc)                                           │
│     ├─ Delete Secret ({name}-secret)                                     │
│     └─ Delete ConfigMap ({name}-config)                                  │
│                                                                           │
│  5. FREE DNS NAME [Activity: cms::free_dns_name]                         │
│     └─ UPDATE dns_names SET instance_id=NULL WHERE instance_id=?         │
│                                                                           │
│  6. DELETE CMS RECORD [Activity: cms::delete_instance_record]            │
│     └─ DELETE FROM instances WHERE k8s_name = ?                          │
│                                                                           │
│  7. RETURN SUCCESS                                                        │
│     └─ {deleted: true}                                                   │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

### 3. Create Image (Backup)

```
┌──────────────────────────────────────────────────────────────────────────┐
│                     CREATE IMAGE ORCHESTRATION                            │
│                     (create_image v1.0.0)                                 │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ PSEUDO CODE                                                               │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  1. VALIDATE SOURCE INSTANCE [Activity: cms::get_instance_by_k8s_name]   │
│     ├─ Check instance exists and state='running'                         │
│     └─ Get postgres_version, storage_size_gb, image_type                 │
│                                                                           │
│  2. GET SOURCE PASSWORD [Activity: get_instance_password]                │
│     └─ Read from K8s Secret ({source_instance}-secret)                   │
│                                                                           │
│  3. CREATE CMS RECORD [Activity: cms::image_ops::Create]                 │
│     └─ INSERT INTO images (name, state='creating', blob_path, ...)       │
│                                                                           │
│  4. RUN BACKUP JOB [Activity: run_backup_job]                            │
│     ├─ Create K8s Job (backup-{image_name}-{orch_suffix})                │
│     ├─ Pod uses toygres-backup image                                     │
│     ├─ Runs: pg_basebackup -h {source}-svc -U postgres -D /backup -Ft -z │
│     └─ Runs: azcopy copy /backup/base.tar.gz blob://images/{path}/       │
│                                                                           │
│  5. WAIT FOR JOB [Activity: wait_for_job + timer loop]                   │
│     ├─ Poll every 10 seconds                                             │
│     ├─ Check job.status.succeeded / job.status.failed                    │
│     └─ Timeout: 1 hour                                                   │
│                                                                           │
│  6. IF SUCCESS:                                                           │
│     ├─ UPDATE CMS TO READY [Activity: cms::image_ops::UpdateState]       │
│     │   └─ UPDATE images SET state='ready', ready_at=NOW()               │
│     └─ CLEANUP JOB [Activity: delete_job]                                │
│         └─ Delete K8s Job                                                │
│                                                                           │
│  7. IF FAILURE:                                                           │
│     ├─ UPDATE CMS TO FAILED [Activity: cms::image_ops::UpdateState]      │
│     │   └─ UPDATE images SET state='failed', error_message=...           │
│     └─ CLEANUP JOB [Activity: delete_job]                                │
│         └─ Delete K8s Job                                                │
│                                                                           │
│  8. RETURN                                                                │
│     └─ {image_name, image_id, blob_path, backup_size_bytes}              │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘

Backup Job Flow (inside K8s Job pod):
┌──────────────────────────────────────────────────────────────────────────┐
│                                                                           │
│   ┌─────────────┐      pg_basebackup       ┌─────────────────┐           │
│   │ Source      │ ───────────────────────► │ /backup/        │           │
│   │ PostgreSQL  │  (streaming replication) │ base.tar.gz     │           │
│   │ (running)   │                          │ checksum.txt    │           │
│   └─────────────┘                          └────────┬────────┘           │
│                                                      │                    │
│                              azcopy (workload identity)                   │
│                                                      │                    │
│                                                      ▼                    │
│                                             ┌─────────────────┐           │
│                                             │ Azure Blob      │           │
│                                             │ Storage         │           │
│                                             │ images/{name}/  │           │
│                                             └─────────────────┘           │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

### 4. Create Instance from Image (Restore)

```
┌──────────────────────────────────────────────────────────────────────────┐
│                     CREATE INSTANCE ORCHESTRATION                         │
│                     (create_instance v1.0.2 with source_image_id)         │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ PSEUDO CODE                                                               │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  1. VALIDATE INPUT + GET IMAGE [Activity: cms::image_ops::GetById]       │
│     ├─ Check source_image_id exists                                      │
│     ├─ Verify image state = 'ready'                                      │
│     └─ Get postgres_version, storage_size_gb, image_type, blob_path      │
│                                                                           │
│  2. GET SOURCE PASSWORD [Activity: cms::image_ops::GetSourcePassword]    │
│     └─ Return decrypted password from image.source_password_encrypted    │
│                                                                           │
│  3. CREATE CMS RECORD [Activity: cms::create_instance_record]            │
│     └─ INSERT INTO instances (name, state='creating', source_image_id)   │
│                                                                           │
│  4. CREATE EMPTY PVC [Activity: create_pvc]                              │
│     └─ Create PVC ({name}-pvc) for restore target                        │
│                                                                           │
│  5. RUN RESTORE JOB [Activity: run_restore_job]                          │
│     ├─ Create K8s Job (restore-{name}-{orch_suffix})                     │
│     ├─ Pod uses toygres-backup image                                     │
│     ├─ Runs: azcopy copy blob://images/{path}/base.tar.gz /backup/       │
│     └─ Runs: tar -xzf /backup/base.tar.gz -C /data/pgdata                │
│                                                                           │
│  6. WAIT FOR RESTORE JOB [Activity: wait_for_job + timer loop]           │
│     ├─ Poll every 10 seconds                                             │
│     └─ Timeout: 1 hour                                                   │
│                                                                           │
│  7. DEPLOY POSTGRES FROM PVC [Activity: deploy_postgres_from_pvc]        │
│     ├─ Create Secret ({name}-secret) with NEW password                   │
│     ├─ Create ConfigMap ({name}-config) with pg_hba.conf                 │
│     ├─ Create StatefulSet ({name}) - PVC already exists                  │
│     └─ Create Service ({name}-svc)                                       │
│                                                                           │
│  8. WAIT FOR READY [Activity: wait_for_ready]                            │
│     └─ Poll until Running + Ready                                        │
│                                                                           │
│  9. SET PASSWORD [Activity: set_password]                                │
│     ├─ Connect with SOURCE password (from image)                         │
│     ├─ Execute: ALTER USER postgres PASSWORD '<new_password>'            │
│     └─ Verify by reconnecting with new password                          │
│                                                                           │
│ 10. GET CONNECTION STRINGS + TEST + UPDATE CMS + START ACTOR             │
│     └─ Same as empty instance flow (steps 5-9)                           │
│                                                                           │
│ 11. CLEANUP RESTORE JOB [Activity: delete_job]                           │
│     └─ Delete K8s Job                                                    │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘

Restore Job Flow (inside K8s Job pod):
┌──────────────────────────────────────────────────────────────────────────┐
│                                                                           │
│   ┌─────────────────┐   azcopy (workload identity)   ┌─────────────────┐ │
│   │ Azure Blob      │ ─────────────────────────────► │ /backup/        │ │
│   │ Storage         │                                 │ base.tar.gz     │ │
│   │ images/{name}/  │                                 └────────┬────────┘ │
│   └─────────────────┘                                          │          │
│                                                        tar -xzf│          │
│                                                                ▼          │
│                                                       ┌─────────────────┐ │
│                                                       │ PVC             │ │
│                                                       │ /data/pgdata    │ │
│                                                       │ (mounted)       │ │
│                                                       └─────────────────┘ │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

### 5. Instance Actor (Health Monitoring)

```
┌──────────────────────────────────────────────────────────────────────────┐
│                     INSTANCE ACTOR ORCHESTRATION                          │
│                     (instance_actor v1.0.0)                               │
│                     [Eternal Orchestration - runs indefinitely]           │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ PSEUDO CODE                                                               │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  LOOP FOREVER {                                                           │
│                                                                           │
│    1. GET INSTANCE CONNECTION [Activity: cms::get_instance_connection]   │
│       └─ SELECT connection_string FROM instances WHERE k8s_name = ?      │
│                                                                           │
│    2. IF INSTANCE NOT FOUND:                                              │
│       └─ RETURN (orchestration completes - instance was deleted)         │
│                                                                           │
│    3. TEST CONNECTION [Activity: test_connection]                        │
│       ├─ Connect to postgres via internal connection string              │
│       ├─ Execute: SELECT version()                                       │
│       └─ Measure response time                                           │
│                                                                           │
│    4. RECORD HEALTH CHECK [Activity: cms::record_health_check]           │
│       └─ INSERT INTO health_checks (instance_id, status, response_ms)    │
│                                                                           │
│    5. UPDATE INSTANCE HEALTH [Activity: cms::update_instance_health]     │
│       └─ UPDATE instances SET health_status='healthy'/'unhealthy'        │
│                                                                           │
│    6. WAIT FOR NEXT CHECK OR SIGNAL                                       │
│       └─ wait_for_event_or_timer("InstanceDeleted", 60 seconds)          │
│                                                                           │
│    7. IF RECEIVED "InstanceDeleted" SIGNAL:                               │
│       └─ RETURN (orchestration completes gracefully)                     │
│                                                                           │
│  } END LOOP                                                               │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘

Health Check Timeline:
┌──────────────────────────────────────────────────────────────────────────┐
│                                                                           │
│  t=0s     t=60s    t=120s   t=180s                                        │
│   │        │         │        │                                           │
│   ▼        ▼         ▼        ▼                                           │
│  [CHECK]──[CHECK]───[CHECK]──[CHECK]──── ... ───[SIGNAL]                 │
│    │        │         │        │                    │                     │
│    └────────┴─────────┴────────┴────────────────────┘                     │
│         Regular 60s interval                  InstanceDeleted             │
│                                               (from delete_instance)      │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

### 6. System Pruner

```
┌──────────────────────────────────────────────────────────────────────────┐
│                     SYSTEM PRUNER ORCHESTRATION                           │
│                     (system_pruner v1.0.4)                                │
│                     [Eternal Orchestration - runs indefinitely]           │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ PSEUDO CODE                                                               │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  LOOP (via continue_as_new) {                                             │
│                                                                           │
│    1. RUN PRUNE ACTIVITY [Activity: system_prune_2]                       │
│       ├─ Query duroxide for all orchestration instances                  │
│       ├─ Delete terminal (Completed/Failed) instances > 6 hours old      │
│       └─ Prune executions to keep only last 3 per instance               │
│                                                                           │
│    2. LOG RESULTS                                                         │
│       └─ "Prune iteration N: X deleted, Y executions pruned"             │
│                                                                           │
│    3. WAIT 1 MINUTE                                                       │
│       └─ schedule_timer(60 seconds)                                       │
│                                                                           │
│    4. CONTINUE AS NEW                                                     │
│       └─ Restart orchestration with iteration+1                          │
│       └─ This prevents unbounded history growth                          │
│                                                                           │
│  } END LOOP                                                               │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Duroxide Runtime Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    TOYGRES-SERVER PROCESS                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                     DUROXIDE RUNTIME                                │ │
│  ├────────────────────────────────────────────────────────────────────┤ │
│  │                                                                     │ │
│  │  ┌───────────────────────────────────────────────────────────────┐ │ │
│  │  │                 ORCHESTRATION REGISTRY                         │ │ │
│  │  │ ─────────────────────────────────────────────────────────────  │ │ │
│  │  │ • create-instance (v1.0.0, v1.0.1, v1.0.2)                    │ │ │
│  │  │ • delete-instance (v1.0.0, v1.0.1, v1.0.2)                    │ │ │
│  │  │ • instance-actor                                               │ │ │
│  │  │ • create-image                                                 │ │ │
│  │  │ • system-pruner (v1.0.0..v1.0.4)                              │ │ │
│  │  └───────────────────────────────────────────────────────────────┘ │ │
│  │                                                                     │ │
│  │  ┌───────────────────────────────────────────────────────────────┐ │ │
│  │  │                   ACTIVITY REGISTRY                            │ │ │
│  │  │ ─────────────────────────────────────────────────────────────  │ │ │
│  │  │ K8s Activities:           CMS Activities:                      │ │ │
│  │  │ • deploy_postgres         • create_instance_record             │ │ │
│  │  │ • deploy_postgres_from_pvc• update_instance_state              │ │ │
│  │  │ • delete_postgres         • get_instance_by_k8s_name          │ │ │
│  │  │ • wait_for_ready          • get_instance_connection           │ │ │
│  │  │ • get_connection_strings  • record_health_check               │ │ │
│  │  │ • test_connection         • update_instance_health            │ │ │
│  │  │ • set_password            • image_ops (CRUD)                  │ │ │
│  │  │                                                                │ │ │
│  │  │ K8s Utilities (consolidated):  System:                        │ │ │
│  │  │ • create_pvc              • system_prune                       │ │ │
│  │  │ • delete_job              • system_prune_2                     │ │ │
│  │  │ • wait_for_job            • send_external_event                │ │ │
│  │  │ • get_instance_password                                        │ │ │
│  │  │                                                                │ │ │
│  │  │ Backup/Restore:                                                │ │ │
│  │  │ • run_backup_job                                               │ │ │
│  │  │ • run_restore_job                                              │ │ │
│  │  └───────────────────────────────────────────────────────────────┘ │ │
│  │                                                                     │ │
│  │  ┌───────────────────────────────────────────────────────────────┐ │ │
│  │  │                 DUROXIDE WORKER (TASK HUBS)                    │ │ │
│  │  │ ─────────────────────────────────────────────────────────────  │ │ │
│  │  │                                                                │ │ │
│  │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │ │ │
│  │  │  │ Orchestrator│  │ Activity    │  │ Timer       │            │ │ │
│  │  │  │ Dispatcher  │  │ Dispatcher  │  │ Dispatcher  │            │ │ │
│  │  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘            │ │ │
│  │  │         │                │                │                    │ │ │
│  │  │         └────────────────┼────────────────┘                    │ │ │
│  │  │                          │                                     │ │ │
│  │  │                          ▼                                     │ │ │
│  │  │              ┌───────────────────────┐                         │ │ │
│  │  │              │  PostgreSQL (CMS DB)  │                         │ │ │
│  │  │              │  duroxide_* tables    │                         │ │ │
│  │  │              └───────────────────────┘                         │ │ │
│  │  │                                                                │ │ │
│  │  └───────────────────────────────────────────────────────────────┘ │ │
│  │                                                                     │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                        AXUM API SERVER                              │ │
│  │ ─────────────────────────────────────────────────────────────────  │ │
│  │ POST /api/instances     → start_orchestration(create-instance)     │ │
│  │ DELETE /api/instances/X → start_orchestration(delete-instance)     │ │
│  │ POST /api/images        → start_orchestration(create-image)        │ │
│  │ GET /api/instances      → query CMS database                       │ │
│  │ GET /api/images         → query CMS database                       │ │
│  │ GET /health             → health check endpoint                    │ │
│  │ GET /*                  → serve React UI static files              │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Data Flow Summary

```
┌───────────┐     API      ┌──────────────┐    Duroxide    ┌────────────────┐
│  Browser  │ ──────────► │ toygres-server│ ────────────► │ CMS Database   │
│  (React)  │             │ (Axum+Duroxide)│              │ (PostgreSQL)   │
└───────────┘             └───────┬────────┘              └────────────────┘
                                  │                                │
                                  │ K8s API                        │
                                  ▼                                │
                          ┌───────────────┐                        │
                          │ AKS Cluster   │ ◄──────────────────────┘
                          │ (StatefulSets,│   (query instance state)
                          │  Jobs, PVCs)  │
                          └───────┬───────┘
                                  │
                                  │ azcopy (Workload Identity)
                                  ▼
                          ┌───────────────┐
                          │ Azure Blob    │
                          │ Storage       │
                          │ (Backups)     │
                          └───────────────┘
```

---

## Versioning Strategy

All orchestrations support versioning for backward-compatible changes:

| Orchestration | Latest | Changes |
|---------------|--------|---------|
| create-instance | v1.0.2 | v1.0.1: CMS error propagation, v1.0.2: source_image restore support |
| delete-instance | v1.0.2 | v1.0.1: CMS error propagation, v1.0.2: Signal actor before cleanup |
| system-pruner | v1.0.4 | Various timer/retention adjustments |
| create-image | v1.0.0 | Initial version |
| instance-actor | v1.0.0 | Initial version |

When `start_orchestration()` is called without a version, the **Latest** version is used.
