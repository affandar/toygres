export interface Instance {
  user_name: string;
  k8s_name: string;
  dns_name: string | null;
  state: 'creating' | 'running' | 'deleting' | 'deleted' | 'failed';
  health_status: 'unknown' | 'healthy' | 'unhealthy';
  postgres_version: string;
  storage_size_gb: number;
  created_at: string;
  updated_at?: string;
  ip_connection_string?: string;
  dns_connection_string?: string;
  external_ip?: string;
  message?: string;
}

export interface InstanceDetail extends Instance {
  id: string;
  namespace: string;
  use_load_balancer: boolean;
  create_orchestration_id: string | null;
  delete_orchestration_id: string | null;
  instance_actor_orchestration_id: string | null;
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
}

export interface ServerStatus {
  serverRunning: boolean;
  apiHealthy: boolean;
  version?: string;
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

