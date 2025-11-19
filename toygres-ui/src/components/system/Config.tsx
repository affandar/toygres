import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { api } from '@/lib/api';

export function Config() {
  const { data: status } = useQuery({
    queryKey: ['server-status'],
    queryFn: () => api.getServerStatus(),
    refetchInterval: 10000,
  });

  const config = {
    server: {
      version: status?.version || 'Unknown',
      api_url: 'http://localhost:8080',
      status: status?.serverRunning ? 'Running' : 'Offline',
    },
    database: {
      provider: 'PostgreSQL',
      schema: 'toygres_cms',
    },
    kubernetes: {
      namespace: 'toygres',
      region: 'westus3',
    },
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Configuration</h1>
        <p className="text-muted-foreground">
          Current server configuration settings
        </p>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Server</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Version</span>
              <span className="text-sm font-medium">{config.server.version}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">API URL</span>
              <span className="text-sm font-mono">{config.server.api_url}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Status</span>
              <span className={`text-sm font-medium ${status?.serverRunning ? 'text-green-600' : 'text-red-600'}`}>
                {config.server.status}
              </span>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Database</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Provider</span>
              <span className="text-sm font-medium">{config.database.provider}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Schema</span>
              <span className="text-sm font-mono">{config.database.schema}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Duroxide Schema</span>
              <span className="text-sm font-mono">toygres_duroxide</span>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Kubernetes</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Namespace</span>
              <span className="text-sm font-mono">{config.kubernetes.namespace}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Region</span>
              <span className="text-sm font-medium">{config.kubernetes.region}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Provider</span>
              <span className="text-sm font-medium">Azure AKS</span>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Duroxide Runtime</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Workers</span>
              <span className="text-sm font-medium">2</span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Orchestrators</span>
              <span className="text-sm font-medium">2</span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Activity Timeout</span>
              <span className="text-sm font-medium">5 minutes</span>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
