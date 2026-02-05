import type { Instance, InstanceDetail, Orchestration, HealthResponse, ServerStatus, Image, ImageDetail, RuntimeImage } from './types';

const API_BASE = ''; // Proxy configured in vite.config.ts

class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public statusText: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function fetchJson<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
  });

  if (!response.ok) {
    const text = await response.text();
    throw new ApiError(
      text || response.statusText,
      response.status,
      response.statusText
    );
  }

  return response.json();
}

export const api = {
  // Health & Status
  async checkHealth(): Promise<HealthResponse> {
    return fetchJson<HealthResponse>(`${API_BASE}/health`);
  },

  async getServerStatus(): Promise<ServerStatus> {
    try {
      const health = await this.checkHealth();
      return {
        serverRunning: true,
        apiHealthy: health.status === 'healthy',
        version: health.version,
        cmsDbHostname: health.cms_db_hostname,
        duroxideDbHostname: health.duroxide_db_hostname,
      };
    } catch (error) {
      return {
        serverRunning: false,
        apiHealthy: false,
      };
    }
  },

  // Instances
  async listInstances(): Promise<Instance[]> {
    return fetchJson<Instance[]>(`${API_BASE}/api/instances`);
  },

  async getInstance(name: string): Promise<InstanceDetail> {
    return fetchJson<InstanceDetail>(`${API_BASE}/api/instances/${name}`);
  },

  async getInstanceLogs(name: string, tailLines: number = 200): Promise<{
    instance_name: string;
    k8s_name: string;
    pod_name: string;
    namespace: string;
    tail_lines: number;
    log_count: number;
    logs: string[];
  }> {
    return fetchJson(`${API_BASE}/api/instances/${name}/logs?tail_lines=${tailLines}`);
  },

  async getPgDurableFunctions(name: string, limit: number = 50, status?: string): Promise<{
    instance_name: string;
    image_type: string;
    count: number;
    functions: Array<{
      instance_id: string;
      label: string | null;
      function_name: string | null;
      status: string;
      execution_count: number;
      output: string | null;
    }>;
  }> {
    const params = new URLSearchParams({ limit: limit.toString() });
    if (status) params.append('status', status);
    return fetchJson(`${API_BASE}/api/instances/${name}/durable-orchestrations?${params}`);
  },

  async getPgDurableInstanceNodes(pgInstanceName: string, orchestrationInstanceId: string, executions: number = 5): Promise<{
    pg_instance_name: string;
    orchestration_instance_id: string;
    executions_shown: number;
    count: number;
    nodes: Array<{
      execution_id: number;
      node_id: string;
      node_type: string;
      query: string | null;
      result_name: string | null;
      left_node: string | null;
      right_node: string | null;
      status: string;
      result: string | null;
    }>;
  }> {
    return fetchJson(`${API_BASE}/api/instances/${pgInstanceName}/durable-orchestrations/${orchestrationInstanceId}/nodes?executions=${executions}`);
  },

  async getPgDurableExplain(pgInstanceName: string, orchestrationInstanceId: string): Promise<{
    pg_instance_name: string;
    orchestration_instance_id: string;
    explain: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances/${pgInstanceName}/durable-orchestrations/${orchestrationInstanceId}/explain`);
  },

  async createInstance(data: {
    name: string;
    password: string;
    postgres_version?: string;
    storage_size_gb?: number;
    internal?: boolean;
    namespace?: string;
    image_type?: 'stock' | 'pg_durable';
    source_image_id?: string;
    runtime_image_id?: string;
  }): Promise<{
    instance_name: string;
    k8s_name: string;
    orchestration_id: string;
    dns_name: string;
    image_type: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  // Runtime Images (ACR catalog)
  async listRuntimeImages() {
    return fetchJson<RuntimeImage[]>(`${API_BASE}/api/runtime-images`);
  },

  async registerRuntimeImage(data: {
    name: string;
    acr_ref: string;
    digest: string;
    description?: string;
    suggested_image_type?: 'stock' | 'pg_durable';
  }) {
    return fetchJson(`${API_BASE}/api/runtime-images/register`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  async deleteInstance(name: string): Promise<{
    instance_name: string;
    k8s_name: string;
    orchestration_id: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances/${name}`, {
      method: 'DELETE',
    });
  },

  // Instance Lifecycle Controls
  async stopInstance(name: string): Promise<{
    instance_name: string;
    k8s_name: string;
    status: string;
    message: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances/${name}/stop`, {
      method: 'POST',
    });
  },

  async startInstance(name: string): Promise<{
    instance_name: string;
    k8s_name: string;
    status: string;
    message: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances/${name}/start`, {
      method: 'POST',
    });
  },

  async restartInstance(name: string): Promise<{
    instance_name: string;
    k8s_name: string;
    status: string;
    restarted_at?: string;
    message: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances/${name}/restart`, {
      method: 'POST',
    });
  },

  // Instance Actor (Health Monitoring) Controls
  async startInstanceActor(name: string): Promise<{
    instance_name: string;
    k8s_name: string;
    actor_id: string;
    status: string;
    message: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances/${name}/actor/start`, {
      method: 'POST',
    });
  },

  async restartInstanceActor(name: string): Promise<{
    instance_name: string;
    k8s_name: string;
    actor_id: string;
    cancelled_existing: boolean;
    status: string;
    message: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances/${name}/actor/restart`, {
      method: 'POST',
    });
  },

  async cancelInstanceActor(name: string): Promise<{
    instance_name: string;
    k8s_name: string;
    actor_id: string;
    status: string;
    message: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances/${name}/actor/cancel`, {
      method: 'POST',
    });
  },

  async bulkCreateInstances(data: {
    base_name: string;
    count: number;
    password: string;
    postgres_version?: string;
    storage_size_gb?: number;
    internal?: boolean;
    namespace?: string;
    image_type?: string;
    runtime_image_id?: string;
  }): Promise<{
    count: number;
    instances: Array<{
      instance_name: string;
      k8s_name: string;
      orchestration_id: string;
      dns_name: string;
    }>;
  }> {
    return fetchJson(`${API_BASE}/api/instances/bulk`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  async bulkDeleteInstances(instance_names: string[]): Promise<{
    deleted: number;
    errors: number;
    instances: Array<{
      instance_name: string;
      k8s_name: string;
      orchestration_id: string;
    }>;
    failures: Array<{
      instance_name: string;
      error: string;
    }>;
  }> {
    return fetchJson(`${API_BASE}/api/instances/bulk/delete`, {
      method: 'POST',
      body: JSON.stringify({ instance_names }),
    });
  },

  // Orchestrations
  async listOrchestrations(): Promise<Orchestration[]> {
    return fetchJson<Orchestration[]>(`${API_BASE}/api/server/orchestrations`);
  },

  async getOrchestration(id: string, historyLimit?: 'full' | '5' | '10'): Promise<Orchestration> {
    const params = new URLSearchParams();
    if (historyLimit) {
      params.append('history_limit', historyLimit);
    }
    const query = params.toString() ? `?${params.toString()}` : '';
    return fetchJson<Orchestration>(`${API_BASE}/api/server/orchestrations/${id}${query}`);
  },

  async cancelOrchestration(id: string): Promise<void> {
    await fetchJson<void>(`${API_BASE}/api/server/orchestrations/${id}/cancel`, {
      method: 'POST',
    });
  },

  async recreateOrchestration(id: string): Promise<{
    new_instance_id: string;
    original_instance_id: string;
    orchestration_name: string;
    orchestration_version: string;
  }> {
    return fetchJson(`${API_BASE}/api/server/orchestrations/${id}/recreate`, {
      method: 'POST',
    });
  },

  async raiseEvent(id: string, eventName: string, eventData: string): Promise<{
    instance_id: string;
    event_name: string;
    raised: boolean;
  }> {
    return fetchJson(`${API_BASE}/api/server/orchestrations/${id}/raise-event`, {
      method: 'POST',
      body: JSON.stringify({ event_name: eventName, event_data: eventData }),
    });
  },

  async deleteOrchestrationInstance(id: string, force: boolean = false): Promise<{
    instance_id: string;
    orchestration_name: string;
    status: string;
    deleted: boolean;
    force: boolean;
  }> {
    const params = new URLSearchParams();
    if (force) params.append('force', 'true');
    const query = params.toString() ? `?${params.toString()}` : '';
    return fetchJson(`${API_BASE}/api/server/orchestrations/${id}/delete${query}`, {
      method: 'POST',
    });
  },

  async pruneOrchestration(id: string, keepExecutions: number = 1): Promise<{
    instance_id: string;
    executions_before: number;
    executions_after: number;
    pruned: number;
    keep_executions: number;
  }> {
    return fetchJson(`${API_BASE}/api/server/orchestrations/${id}/prune`, {
      method: 'POST',
      body: JSON.stringify({ keep_executions: keepExecutions }),
    });
  },

  async getOrchestrationTree(id: string): Promise<{
    instance_id: string;
    orchestration_name: string;
    status: string;
    created_at: string;
    execution_count: number;
    parent: { instance_id: string; orchestration_name: string; status: string } | null;
    children: Array<{
      instance_id: string;
      orchestration_name: string;
      status: string;
      created_at: string;
      is_direct_child: boolean;
    }>;
    children_count: number;
    tree_size: number;
    is_root: boolean;
  }> {
    return fetchJson(`${API_BASE}/api/server/orchestrations/${id}/tree`);
  },

  // Orchestration Flows (Static Diagrams)
  async listOrchestrationFlows(): Promise<Array<{
    orchestration_name: string;
    mermaid: string;
    node_mappings: Array<{ node_id: string; activity_pattern: string }>;
  }>> {
    return fetchJson(`${API_BASE}/api/server/orchestration-flows`);
  },

  async getOrchestrationFlow(name: string): Promise<{
    orchestration_name: string;
    mermaid: string;
    node_mappings: Array<{ node_id: string; activity_pattern: string }>;
  }> {
    return fetchJson(`${API_BASE}/api/server/orchestration-flows/${encodeURIComponent(name)}`);
  },

  // Logs
  async getLogs(limit?: number, filter?: string): Promise<string[]> {
    const params = new URLSearchParams();
    if (limit) params.append('limit', limit.toString());
    if (filter) params.append('filter', filter);
    const query = params.toString() ? `?${params.toString()}` : '';
    return fetchJson<string[]>(`${API_BASE}/api/server/logs${query}`);
  },

  // System Pruner
  async getPruneLog(): Promise<{
    instance_id: string;
    orchestration_version: string;
    status: string;
    current_execution_id: number;
    created_at: string;
    last_run: string | null;
    iteration: number;
    prune_log: Array<{
      timestamp: string;
      operation: string;
      instance_id: string;
      orchestration_name: string;
      status: string;
      details: string;
    }>;
    total_entries: number;
  }> {
    return fetchJson(`${API_BASE}/api/server/prune-log`);
  },

  // Images
  async listImages(): Promise<Image[]> {
    return fetchJson<Image[]>(`${API_BASE}/api/images`);
  },

  async getImage(name: string): Promise<ImageDetail> {
    return fetchJson<ImageDetail>(`${API_BASE}/api/images/${name}`);
  },

  async getImageJobLogs(name: string): Promise<{
    image_name: string;
    job_name: string;
    job_status: string;
    logs: string;
  }> {
    return fetchJson(`${API_BASE}/api/images/${name}/logs`);
  },

  async createImage(data: {
    name: string;
    source_k8s_name: string;
    password?: string;
    description?: string;
    namespace?: string;
  }): Promise<{
    image_name: string;
    orchestration_id: string;
    message: string;
  }> {
    return fetchJson(`${API_BASE}/api/images`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  async createImageFromInstance(instanceName: string, data: {
    name: string;
    password?: string;
    description?: string;
  }): Promise<{
    image_name: string;
    source_instance: string;
    orchestration_id: string;
    message: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances/${instanceName}/images`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  async deleteImage(name: string): Promise<{
    image_name: string;
    status: string;
    message: string;
  }> {
    return fetchJson(`${API_BASE}/api/images/${name}`, {
      method: 'DELETE',
    });
  },
};

