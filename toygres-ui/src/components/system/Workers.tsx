import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { api } from '@/lib/api';

export function Workers() {
  const { data: orchestrations } = useQuery({
    queryKey: ['orchestrations'],
    queryFn: () => api.listOrchestrations(),
    refetchInterval: 2000,
  });

  const runningOrchestrations = orchestrations?.filter(o => o.status === 'Running') || [];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Worker Status</h1>
        <p className="text-muted-foreground">
          View active workers and their current tasks
        </p>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Orchestration Workers</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">2</div>
            <p className="text-xs text-muted-foreground">Concurrent workers</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">Activity Workers</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">2</div>
            <p className="text-xs text-muted-foreground">Concurrent workers</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">Active Tasks</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{runningOrchestrations.length}</div>
            <p className="text-xs text-muted-foreground">Running orchestrations</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Running Orchestrations</CardTitle>
        </CardHeader>
        <CardContent>
          {runningOrchestrations.length === 0 ? (
            <p className="text-sm text-muted-foreground">No orchestrations currently running</p>
          ) : (
            <div className="space-y-2">
              {runningOrchestrations.slice(0, 10).map((orch) => {
                const shortType = orch.orchestration_name.split('::').pop() || orch.orchestration_name;
                return (
                  <div
                    key={orch.instance_id}
                    className="flex items-center justify-between p-3 rounded-md border"
                  >
                    <div>
                      <p className="text-sm font-medium">{orch.instance_id}</p>
                      <p className="text-xs text-muted-foreground">{shortType} (exec #{orch.current_execution_id})</p>
                    </div>
                    <span className="text-xs text-blue-600">● Running</span>
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Worker Configuration</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex justify-between">
            <span className="text-sm text-muted-foreground">Orchestrator Lock Timeout</span>
            <span className="text-sm font-medium">5 seconds</span>
          </div>
          <div className="flex justify-between">
            <span className="text-sm text-muted-foreground">Activity Lock Timeout</span>
            <span className="text-sm font-medium">5 minutes</span>
          </div>
          <div className="flex justify-between">
            <span className="text-sm text-muted-foreground">Dispatcher Idle Sleep</span>
            <span className="text-sm font-medium">100 ms</span>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

