import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { api } from '@/lib/api';
import { Trash2, Scissors, Clock, RefreshCw, AlertCircle } from 'lucide-react';

export function PruneLog() {
  const { data: pruneData, isLoading, error, refetch } = useQuery({
    queryKey: ['prune-log'],
    queryFn: () => api.getPruneLog(),
    refetchInterval: 30000, // Refresh every 30 seconds
  });

  const formatTime = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      return date.toLocaleString();
    } catch {
      return timestamp;
    }
  };

  const getOperationIcon = (operation: string) => {
    switch (operation) {
      case 'delete':
        return <Trash2 className="h-4 w-4 text-red-500" />;
      case 'prune':
        return <Scissors className="h-4 w-4 text-orange-500" />;
      default:
        return <AlertCircle className="h-4 w-4 text-gray-500" />;
    }
  };

  const getStatusBadge = (status: string) => {
    const colors: Record<string, string> = {
      Running: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
      Completed: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
      Failed: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
    };
    return colors[status] || 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200';
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">System Pruner Log</h1>
          <p className="text-muted-foreground">
            Automatic cleanup of old orchestrations and executions
          </p>
        </div>
        <button
          onClick={() => refetch()}
          className="flex items-center gap-2 px-3 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90"
        >
          <RefreshCw className="h-4 w-4" />
          Refresh
        </button>
      </div>

      {/* Status Card */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Clock className="h-5 w-5" />
            Pruner Status
          </CardTitle>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <p className="text-muted-foreground">Loading...</p>
          ) : error ? (
            <div className="text-center py-8">
              <AlertCircle className="h-12 w-12 text-muted-foreground mx-auto mb-2" />
              <p className="text-muted-foreground">
                System pruner is not running yet.
              </p>
              <p className="text-sm text-muted-foreground mt-1">
                It will start automatically when the server starts.
              </p>
            </div>
          ) : pruneData ? (
            <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
              <div>
                <p className="text-sm text-muted-foreground">Status</p>
                <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium ${getStatusBadge(pruneData.status)}`}>
                  {pruneData.status}
                </span>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Version</p>
                <p className="font-mono text-sm">{pruneData.orchestration_version}</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Iteration</p>
                <p className="font-mono">#{pruneData.iteration || pruneData.current_execution_id}</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Started</p>
                <p className="text-sm">{formatTime(pruneData.created_at)}</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Last Run</p>
                <p className="text-sm">{pruneData.last_run ? formatTime(pruneData.last_run) : 'Pending...'}</p>
              </div>
            </div>
          ) : null}
        </CardContent>
      </Card>

      {/* Configuration Card */}
      <Card>
        <CardHeader>
          <CardTitle>Configuration</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
            <div className="flex items-center gap-2 p-3 bg-muted rounded-md">
              <Trash2 className="h-4 w-4 text-red-500" />
              <div>
                <p className="font-medium">Delete Terminal Instances</p>
                <p className="text-muted-foreground">Older than 6 hours</p>
              </div>
            </div>
            <div className="flex items-center gap-2 p-3 bg-muted rounded-md">
              <Scissors className="h-4 w-4 text-orange-500" />
              <div>
                <p className="font-medium">Prune Executions</p>
                <p className="text-muted-foreground">Keep only latest 1</p>
              </div>
            </div>
            <div className="flex items-center gap-2 p-3 bg-muted rounded-md">
              <Clock className="h-4 w-4 text-blue-500" />
              <div>
                <p className="font-medium">Run Interval</p>
                <p className="text-muted-foreground">Every 1 hour</p>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Prune Log Table */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center justify-between">
            <span>Recent Activity</span>
            {pruneData?.prune_log && (
              <span className="text-sm font-normal text-muted-foreground">
                {pruneData.prune_log.length} entries
              </span>
            )}
          </CardTitle>
        </CardHeader>
        <CardContent>
          {!pruneData?.prune_log || pruneData.prune_log.length === 0 ? (
            <div className="text-center py-8">
              <p className="text-muted-foreground">No prune activity yet.</p>
              <p className="text-sm text-muted-foreground mt-1">
                The pruner runs every hour. Check back later.
              </p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b">
                    <th className="text-left py-2 px-3">Time</th>
                    <th className="text-left py-2 px-3">Operation</th>
                    <th className="text-left py-2 px-3">Instance</th>
                    <th className="text-left py-2 px-3">Orchestration</th>
                    <th className="text-left py-2 px-3">Status</th>
                    <th className="text-left py-2 px-3">Details</th>
                  </tr>
                </thead>
                <tbody>
                  {pruneData.prune_log.map((entry, idx) => (
                    <tr key={idx} className="border-b hover:bg-muted/50">
                      <td className="py-2 px-3 whitespace-nowrap">
                        {formatTime(entry.timestamp)}
                      </td>
                      <td className="py-2 px-3">
                        <div className="flex items-center gap-2">
                          {getOperationIcon(entry.operation)}
                          <span className="capitalize">{entry.operation}</span>
                        </div>
                      </td>
                      <td className="py-2 px-3 font-mono text-xs">
                        {entry.instance_id.length > 20
                          ? `${entry.instance_id.slice(0, 20)}...`
                          : entry.instance_id}
                      </td>
                      <td className="py-2 px-3 font-mono text-xs">
                        {entry.orchestration_name.split('::').pop()}
                      </td>
                      <td className="py-2 px-3">
                        <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${getStatusBadge(entry.status)}`}>
                          {entry.status}
                        </span>
                      </td>
                      <td className="py-2 px-3 text-muted-foreground">
                        {entry.details}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Info Card */}
      <Card>
        <CardHeader>
          <CardTitle>About System Pruner</CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground space-y-2">
          <p>The system pruner is an eternal orchestration that automatically cleans up:</p>
          <ul className="list-disc list-inside space-y-1 ml-2">
            <li><strong>Terminal instances:</strong> Completed or Failed orchestrations older than 6 hours are permanently deleted</li>
            <li><strong>Old executions:</strong> For running orchestrations using continue-as-new, old execution history is pruned to keep only the current execution</li>
          </ul>
          <p className="mt-4">This prevents unbounded growth of the orchestration database and keeps the system healthy.</p>
        </CardContent>
      </Card>
    </div>
  );
}
