import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { 
  RefreshCw, 
  CheckCircle2, 
  XCircle, 
  Clock, 
  PlayCircle, 
  Loader2,
  ChevronDown,
  ChevronRight,
  AlertTriangle,
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/api';

interface PgDurableOrchestrationsProps {
  instanceName: string;
}

// Friendly labels for orchestration statuses
const STATUS_CONFIG: Record<string, { icon: React.ReactNode; label: string; color: string; bgColor: string }> = {
  'running': {
    icon: <PlayCircle className="h-4 w-4" />,
    label: 'Running',
    color: 'text-blue-400',
    bgColor: 'bg-blue-500/20',
  },
  'completed': {
    icon: <CheckCircle2 className="h-4 w-4" />,
    label: 'Completed',
    color: 'text-green-400',
    bgColor: 'bg-green-500/20',
  },
  'failed': {
    icon: <XCircle className="h-4 w-4" />,
    label: 'Failed',
    color: 'text-red-400',
    bgColor: 'bg-red-500/20',
  },
  'pending': {
    icon: <Clock className="h-4 w-4" />,
    label: 'Pending',
    color: 'text-yellow-400',
    bgColor: 'bg-yellow-500/20',
  },
  'suspended': {
    icon: <Loader2 className="h-4 w-4" />,
    label: 'Suspended',
    color: 'text-slate-400',
    bgColor: 'bg-slate-500/20',
  },
  'cancelled': {
    icon: <XCircle className="h-4 w-4" />,
    label: 'Cancelled',
    color: 'text-orange-400',
    bgColor: 'bg-orange-500/20',
  },
};

function getStatusConfig(status: string) {
  return STATUS_CONFIG[status.toLowerCase()] || {
    icon: <AlertTriangle className="h-4 w-4" />,
    label: status,
    color: 'text-slate-400',
    bgColor: 'bg-slate-400/20',
  };
}

// Expandable row component that shows explain visualization when clicked
function ExpandableInstanceRow({ 
  instance, 
  pgInstanceName,
  isExpanded,
  onToggle 
}: { 
  instance: {
    instance_id: string;
    label: string | null;
    function_name: string | null;
    status: string;
    execution_count: number;
    output: string | null;
  };
  pgInstanceName: string;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  const statusConfig = getStatusConfig(instance.status);
  
  // Fetch explain output when expanded (with auto-refresh for running instances)
  const { data: explainData, isLoading: explainLoading, isFetching: explainFetching } = useQuery({
    queryKey: ['pg-durable-explain', pgInstanceName, instance.instance_id],
    queryFn: () => api.getPgDurableExplain(pgInstanceName, instance.instance_id),
    enabled: isExpanded,
    refetchInterval: isExpanded && instance.status.toLowerCase() === 'running' ? 3000 : false,
    staleTime: 2000,
  });

  return (
    <>
      {/* Main Row */}
      <tr 
        className="border-b hover:bg-muted/30 cursor-pointer"
        onClick={onToggle}
      >
        <td className="p-3">
          <button className="text-muted-foreground hover:text-foreground">
            {isExpanded ? (
              <ChevronDown className="h-4 w-4" />
            ) : (
              <ChevronRight className="h-4 w-4" />
            )}
          </button>
        </td>
        <td className="p-3 font-mono text-xs">{instance.instance_id}</td>
        <td className="p-3 text-sm">{instance.label || '—'}</td>
        <td className="p-3 text-sm text-muted-foreground">{instance.function_name || '—'}</td>
        <td className="p-3">
          <span className={`inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded ${statusConfig.bgColor} ${statusConfig.color}`}>
            {statusConfig.icon}
            {statusConfig.label}
          </span>
        </td>
        <td className="p-3 text-center text-sm">{instance.execution_count}</td>
      </tr>
      
      {/* Expanded Details - durable.explain() output */}
      {isExpanded && (
        <tr className="bg-muted/20">
          <td colSpan={6} className="p-0">
            <div className="p-4">
              <div className="flex items-center gap-2 mb-3">
                <h4 className="text-sm font-medium">Function Graph</h4>
                {explainFetching && !explainLoading && (
                  <RefreshCw className="h-3 w-3 animate-spin text-muted-foreground" />
                )}
              </div>
              {explainLoading ? (
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <RefreshCw className="h-4 w-4 animate-spin" />
                  Loading...
                </div>
              ) : explainData ? (
                <pre className="font-mono text-xs bg-slate-900 text-slate-200 p-4 rounded overflow-x-auto whitespace-pre">
                  {explainData.explain}
                </pre>
              ) : (
                <p className="text-xs text-muted-foreground">No data available</p>
              )}
            </div>
          </td>
        </tr>
      )}
    </>
  );
}

export function PgDurableOrchestrations({ instanceName }: PgDurableOrchestrationsProps) {
  const [limit, setLimit] = useState(50);
  const [statusFilter, setStatusFilter] = useState<string>('');
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

  const { data, isLoading, error, refetch, isFetching } = useQuery({
    queryKey: ['pg-durable-functions', instanceName, limit, statusFilter],
    queryFn: () => api.getPgDurableFunctions(instanceName, limit, statusFilter || undefined),
    refetchInterval: 10000,
    staleTime: 5000,
  });

  const toggleExpanded = (id: string) => {
    setExpandedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-lg flex items-center gap-2">
          ⚡ Durable SQL Functions
          <span className="text-xs bg-blue-500/20 text-blue-400 px-2 py-0.5 rounded font-normal">
            pg_durable
          </span>
        </CardTitle>
        <div className="flex items-center gap-2">
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            className="text-xs bg-background border rounded px-2 py-1"
          >
            <option value="">All Status</option>
            <option value="Running">Running</option>
            <option value="Completed">Completed</option>
            <option value="Failed">Failed</option>
            <option value="Cancelled">Cancelled</option>
          </select>
          <select
            value={limit}
            onChange={(e) => setLimit(Number(e.target.value))}
            className="text-xs bg-background border rounded px-2 py-1"
          >
            <option value={25}>Last 25</option>
            <option value={50}>Last 50</option>
            <option value={100}>Last 100</option>
          </select>
          <Button
            variant="outline"
            size="sm"
            onClick={() => refetch()}
            disabled={isFetching}
          >
            <RefreshCw className={`h-3 w-3 ${isFetching ? 'animate-spin' : ''}`} />
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <div className="flex items-center justify-center h-48">
            <RefreshCw className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : error ? (
          <div className="flex flex-col items-center justify-center h-48 text-center">
            <p className="text-sm text-destructive mb-2">Failed to load functions</p>
            <p className="text-xs text-muted-foreground max-w-md">
              {error instanceof Error ? error.message : 'Unknown error'}
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => refetch()}
              className="mt-4"
            >
              Retry
            </Button>
          </div>
        ) : data && data.functions.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b bg-muted/50">
                  <th className="p-3 w-8"></th>
                  <th className="text-left p-3 font-medium">Instance ID</th>
                  <th className="text-left p-3 font-medium">Label</th>
                  <th className="text-left p-3 font-medium">Function</th>
                  <th className="text-left p-3 font-medium">Status</th>
                  <th className="text-center p-3 font-medium">Executions</th>
                </tr>
              </thead>
              <tbody>
                {data.functions.map((fn) => (
                  <ExpandableInstanceRow
                    key={fn.instance_id}
                    instance={fn}
                    pgInstanceName={instanceName}
                    isExpanded={expandedIds.has(fn.instance_id)}
                    onToggle={() => toggleExpanded(fn.instance_id)}
                  />
                ))}
              </tbody>
            </table>
            <div className="flex justify-between items-center pt-3 text-xs text-muted-foreground">
              <span>Showing {data.functions.length} of {data.count} function instances</span>
              <span className="text-muted-foreground">Click a row to view details</span>
            </div>
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center h-48 text-center">
            <p className="text-sm text-muted-foreground">No durable functions found</p>
            <p className="text-xs text-muted-foreground mt-1">
              {statusFilter ? `No functions with status "${statusFilter}"` : 'Start a durable SQL function to see it here'}
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
