import type { Instance, InstanceDetail, Orchestration, HealthResponse, ServerStatus } from './types';

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

  async createInstance(data: {
    name: string;
    password: string;
    postgres_version?: string;
    storage_size_gb?: number;
    internal?: boolean;
    namespace?: string;
  }): Promise<{
    instance_name: string;
    k8s_name: string;
    orchestration_id: string;
    dns_name: string;
  }> {
    return fetchJson(`${API_BASE}/api/instances`, {
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

  async bulkCreateInstances(data: {
    base_name: string;
    count: number;
    password: string;
    postgres_version?: string;
    storage_size_gb?: number;
    internal?: boolean;
    namespace?: string;
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

  // Logs
  async getLogs(limit?: number, filter?: string): Promise<string[]> {
    const params = new URLSearchParams();
    if (limit) params.append('limit', limit.toString());
    if (filter) params.append('filter', filter);
    const query = params.toString() ? `?${params.toString()}` : '';
    return fetchJson<string[]>(`${API_BASE}/api/server/logs${query}`);
  },
};

