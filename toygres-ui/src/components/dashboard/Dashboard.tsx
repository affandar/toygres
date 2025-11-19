import { useQuery } from '@tanstack/react-query';
import { Database, CheckCircle, Zap, Activity } from 'lucide-react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/Card';
import { api } from '@/lib/api';
import { formatRelativeTime } from '@/lib/utils';

function StatCard({ 
  title, 
  value, 
  icon: Icon 
}: { 
  title: string; 
  value: number | string; 
  icon: React.ComponentType<{ className?: string }>;
}) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-sm font-medium">{title}</CardTitle>
        <Icon className="h-4 w-4 text-muted-foreground" />
      </CardHeader>
      <CardContent>
        <div className="text-2xl font-bold">{value}</div>
      </CardContent>
    </Card>
  );
}

export function Dashboard() {
  const { data: instances } = useQuery({
    queryKey: ['instances'],
    queryFn: () => api.listInstances(),
    refetchInterval: 5000,
  });

  const { data: orchestrations } = useQuery({
    queryKey: ['orchestrations'],
    queryFn: () => api.listOrchestrations(),
    refetchInterval: 5000,
  });

  const stats = {
    totalInstances: instances?.length || 0,
    healthyInstances: instances?.filter(i => i.health_status === 'healthy').length || 0,
    runningOrchestrations: orchestrations?.filter(o => o.status === 'Running').length || 0,
  };

  // Get recent activity (last 5 orchestrations by updated_at)
  const recentActivity = orchestrations
    ?.slice()
    .sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime())
    .slice(0, 5) || [];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
        <p className="text-muted-foreground">
          Overview of your PostgreSQL instances and system status
        </p>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <StatCard
          title="Total Instances"
          value={stats.totalInstances}
          icon={Database}
        />
        <StatCard
          title="Healthy Instances"
          value={stats.healthyInstances}
          icon={CheckCircle}
        />
        <StatCard
          title="Active Orchestrations"
          value={stats.runningOrchestrations}
          icon={Zap}
        />
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Recent Activity</CardTitle>
            <CardDescription>Latest orchestration updates</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {recentActivity.length === 0 ? (
                <p className="text-sm text-muted-foreground">No recent activity</p>
              ) : (
                recentActivity.map((orch) => {
                  const shortName = orch.orchestration_name.split('::').pop() || orch.orchestration_name;
                  const instanceName = orch.instance_id.split('-')[0] || orch.instance_id;
                  
                  return (
                    <div key={orch.instance_id} className="flex items-start space-x-3 text-sm">
                      <Activity className="mt-0.5 h-4 w-4 text-muted-foreground" />
                      <div className="flex-1">
                        <p className="font-medium">
                          {instanceName} - {shortName}
                        </p>
                        <p className="text-muted-foreground">
                          {orch.status} · {formatRelativeTime(orch.updated_at)}
                        </p>
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>System Status</CardTitle>
            <CardDescription>Health check summary</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm">Database</span>
                <span className="flex items-center text-sm text-green-600">
                  <CheckCircle className="mr-1 h-4 w-4" />
                  Connected
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm">Kubernetes</span>
                <span className="flex items-center text-sm text-green-600">
                  <CheckCircle className="mr-1 h-4 w-4" />
                  Connected
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm">Duroxide Runtime</span>
                <span className="flex items-center text-sm text-green-600">
                  <CheckCircle className="mr-1 h-4 w-4" />
                  {stats.runningOrchestrations} orchestrations active
                </span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

