export type ImageType = 'stock' | 'pg_durable';

export type InstanceState = 'creating' | 'running' | 'stopped' | 'deleting' | 'deleted' | 'failed';

export type ImageState = 'creating' | 'ready' | 'failed' | 'deleting' | 'deleted';

export interface Image {
  id: string;
  name: string;
  description: string | null;
  source_k8s_name: string;
  state: ImageState;
  storage_size_gb: number;
  postgres_version: string;
  image_type: string;
  backup_size_bytes: number | null;
  created_at: string;
  ready_at: string | null;
}

export interface ImageDetail extends Image {
  source_instance_id: string | null;
  source_namespace: string;
  blob_storage_url: string;
  blob_container: string;
  blob_path: string;
  backup_checksum: string | null;
  error_message: string | null;
  orchestration_id: string;
}

export interface Instance {
  user_name: string;
  k8s_name: string;
  dns_name: string | null;
  state: InstanceState;
  health_status: 'unknown' | 'healthy' | 'unhealthy';
  postgres_version: string;
  storage_size_gb: number;
  created_at: string;
  updated_at?: string;
  ip_connection_string?: string;
  dns_connection_string?: string;
  external_ip?: string;
  message?: string;
  image_type: ImageType;
}

export interface InstanceDetail extends Instance {
  id: string;
  namespace: string;
  use_load_balancer: boolean;
  create_orchestration_id: string | null;
  delete_orchestration_id: string | null;
  instance_actor_orchestration_id: string | null;
  last_health_check: string | null;
}

export interface Orchestration {
  instance_id: string;
  orchestration_name: string;
  orchestration_version: string;
  status: 'Running' | 'Completed' | 'Failed';
  created_at: string;
  updated_at: string;
  current_execution_id: number;
  output?: string;
  history?: OrchestrationEvent[];
}

export interface OrchestrationEvent {
  event: string;
  execution_id: number;
}

export interface HealthResponse {
  status: string;
  service: string;
  version: string;
  cms_db_hostname?: string;
  duroxide_db_hostname?: string;
}

export interface ServerStatus {
  serverRunning: boolean;
  apiHealthy: boolean;
  version?: string;
  cmsDbHostname?: string;
  duroxideDbHostname?: string;
}

export interface Stats {
  instances: {
    total: number;
    running: number;
    creating: number;
    failed: number;
    healthy: number;
    unhealthy: number;
  };
  orchestrations: {
    running: number;
    completed: number;
    failed: number;
  };
}

