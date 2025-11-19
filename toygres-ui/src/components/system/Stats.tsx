import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { api } from '@/lib/api';

export function Stats() {
  const { data: instances } = useQuery({
    queryKey: ['instances'],
    queryFn: () => api.listInstances(),
    refetchInterval: 2000,
  });

  const { data: orchestrations } = useQuery({
    queryKey: ['orchestrations'],
    queryFn: () => api.listOrchestrations(),
    refetchInterval: 2000,
  });

  const instanceStats = {
    total: instances?.length || 0,
    running: instances?.filter(i => i.state === 'running').length || 0,
    creating: instances?.filter(i => i.state === 'creating').length || 0,
    failed: instances?.filter(i => i.state === 'failed').length || 0,
    healthy: instances?.filter(i => i.health_status === 'healthy').length || 0,
    unhealthy: instances?.filter(i => i.health_status === 'unhealthy').length || 0,
  };

  const orchestrationStats = {
    running: orchestrations?.filter(o => o.status === 'Running').length || 0,
    completed: orchestrations?.filter(o => o.status === 'Completed').length || 0,
    failed: orchestrations?.filter(o => o.status === 'Failed').length || 0,
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">System Statistics</h1>
        <p className="text-muted-foreground">
          Real-time metrics and system status
        </p>
      </div>

      <div className="space-y-4">
        <Card>
          <CardHeader>
            <CardTitle>Instances</CardTitle>
          </CardHeader>
          <CardContent className="grid gap-4 md:grid-cols-3">
            <div>
              <div className="text-2xl font-bold">{instanceStats.total}</div>
              <p className="text-xs text-muted-foreground">Total</p>
            </div>
            <div>
              <div className="text-2xl font-bold text-green-600">{instanceStats.running}</div>
              <p className="text-xs text-muted-foreground">Running</p>
            </div>
            <div>
              <div className="text-2xl font-bold text-blue-600">{instanceStats.creating}</div>
              <p className="text-xs text-muted-foreground">Creating</p>
            </div>
            <div>
              <div className="text-2xl font-bold text-red-600">{instanceStats.failed}</div>
              <p className="text-xs text-muted-foreground">Failed</p>
            </div>
            <div>
              <div className="text-2xl font-bold text-green-600">{instanceStats.healthy}</div>
              <p className="text-xs text-muted-foreground">Healthy</p>
            </div>
            <div>
              <div className="text-2xl font-bold text-red-600">{instanceStats.unhealthy}</div>
              <p className="text-xs text-muted-foreground">Unhealthy</p>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Orchestrations</CardTitle>
          </CardHeader>
          <CardContent className="grid gap-4 md:grid-cols-3">
            <div>
              <div className="text-2xl font-bold text-blue-600">{orchestrationStats.running}</div>
              <p className="text-xs text-muted-foreground">Running</p>
            </div>
            <div>
              <div className="text-2xl font-bold text-green-600">{orchestrationStats.completed}</div>
              <p className="text-xs text-muted-foreground">Completed</p>
            </div>
            <div>
              <div className="text-2xl font-bold text-red-600">{orchestrationStats.failed}</div>
              <p className="text-xs text-muted-foreground">Failed</p>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

