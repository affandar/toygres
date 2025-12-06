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

// Node type colors
const NODE_TYPE_COLORS: Record<string, string> = {
  'SQL': 'text-blue-400 bg-blue-500/20',
  'THEN': 'text-purple-400 bg-purple-500/20',
  'LOOP': 'text-orange-400 bg-orange-500/20',
  'WAIT_SCHEDULE': 'text-yellow-400 bg-yellow-500/20',
  'JOIN': 'text-cyan-400 bg-cyan-500/20',
  'IF': 'text-pink-400 bg-pink-500/20',
};

// Expandable row component that shows nodes when clicked
function ExpandableInstanceRow({ 
  instance, 
  pgInstanceName,
  isExpanded,
  onToggle 
}: { 
  instance: {
    instance_id: string;
    label: string | null;
    orchestration_name: string | null;
    status: string;
    execution_count: number;
    output: string | null;
  };
  pgInstanceName: string;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  const statusConfig = getStatusConfig(instance.status);
  
  // Fetch nodes when expanded (with auto-refresh)
  const { data: nodesData, isLoading: nodesLoading, isFetching: nodesFetching } = useQuery({
    queryKey: ['pg-durable-nodes', pgInstanceName, instance.instance_id],
    queryFn: () => api.getPgDurableInstanceNodes(pgInstanceName, instance.instance_id, 5),
    enabled: isExpanded,
    refetchInterval: isExpanded ? 5000 : false, // Refresh every 5s when expanded
    staleTime: 2000,
  });

  // Try to parse output as JSON
  const formatOutput = (output: string | null) => {
    if (!output) return null;
    try {
      const parsed = JSON.parse(output);
      return JSON.stringify(parsed, null, 2);
    } catch {
      return output;
    }
  };

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
        <td className="p-3 text-sm text-muted-foreground">{instance.orchestration_name || '—'}</td>
        <td className="p-3">
          <span className={`inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded ${statusConfig.bgColor} ${statusConfig.color}`}>
            {statusConfig.icon}
            {statusConfig.label}
          </span>
        </td>
        <td className="p-3 text-center text-sm">{instance.execution_count}</td>
      </tr>
      
      {/* Expanded Details */}
      {isExpanded && (
        <tr className="bg-muted/20">
          <td colSpan={6} className="p-0">
            <div className="p-4 space-y-4">
              {/* Output Section */}
              <div>
                <h4 className="text-sm font-medium mb-2">Output / Result</h4>
                {instance.output ? (
                  <pre className={`text-xs p-3 rounded overflow-x-auto max-h-32 ${
                    instance.status.toLowerCase() === 'failed' 
                      ? 'bg-red-500/10 text-red-300 border border-red-500/20' 
                      : 'bg-slate-900 text-slate-200'
                  }`}>
                    {formatOutput(instance.output)}
                  </pre>
                ) : (
                  <p className="text-xs text-muted-foreground">No output available</p>
                )}
              </div>
              
              {/* Nodes Section */}
              <div>
                <div className="flex items-center gap-2 mb-2">
                  <h4 className="text-sm font-medium">Execution Nodes</h4>
                  {nodesFetching && !nodesLoading && (
                    <RefreshCw className="h-3 w-3 animate-spin text-muted-foreground" />
                  )}
                </div>
                {nodesLoading ? (
                  <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <RefreshCw className="h-4 w-4 animate-spin" />
                    Loading nodes...
                  </div>
                ) : nodesData && nodesData.nodes.length > 0 ? (
                  <div className="overflow-x-auto">
                    <table className="w-full text-xs border rounded">
                      <thead>
                        <tr className="bg-muted/50 border-b">
                          <th className="text-left p-2 font-medium">Exec</th>
                          <th className="text-left p-2 font-medium">Type</th>
                          <th className="text-left p-2 font-medium">Status</th>
                          <th className="text-left p-2 font-medium">Query</th>
                          <th className="text-left p-2 font-medium">Result</th>
                        </tr>
                      </thead>
                      <tbody>
                        {nodesData.nodes.map((node, idx) => {
                          const nodeStatus = getStatusConfig(node.status);
                          const typeColor = NODE_TYPE_COLORS[node.node_type] || 'text-slate-400 bg-slate-500/20';
                          return (
                            <tr key={`${node.execution_id}-${node.node_id}-${idx}`} className="border-b last:border-0">
                              <td className="p-2 font-mono text-muted-foreground">#{node.execution_id}</td>
                              <td className="p-2">
                                <span className={`px-1.5 py-0.5 rounded ${typeColor}`}>
                                  {node.node_type}
                                </span>
                              </td>
                              <td className="p-2">
                                <span className={`inline-flex items-center gap-1 ${nodeStatus.color}`}>
                                  {nodeStatus.icon}
                                </span>
                              </td>
                              <td className="p-2 max-w-xs">
                                {node.query ? (
                                  <code className="block bg-muted p-1 rounded truncate" title={node.query}>
                                    {node.query.length > 50 ? node.query.slice(0, 50) + '...' : node.query}
                                  </code>
                                ) : '—'}
                              </td>
                              <td className="p-2 max-w-xs">
                                {node.result ? (
                                  <code className={`block p-1 rounded truncate ${
                                    node.status.toLowerCase() === 'failed' ? 'bg-red-500/10 text-red-300' : 'bg-muted'
                                  }`} title={node.result}>
                                    {node.result.length > 50 ? node.result.slice(0, 50) + '...' : node.result}
                                  </code>
                                ) : '—'}
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                ) : (
                  <p className="text-xs text-muted-foreground">No execution nodes found</p>
                )}
              </div>
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
    queryKey: ['pg-durable-orchestrations', instanceName, limit, statusFilter],
    queryFn: () => api.getPgDurableOrchestrations(instanceName, limit, statusFilter || undefined),
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
        ) : data && data.orchestrations.length > 0 ? (
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
                {data.orchestrations.map((orch) => (
                  <ExpandableInstanceRow
                    key={orch.instance_id}
                    instance={orch}
                    pgInstanceName={instanceName}
                    isExpanded={expandedIds.has(orch.instance_id)}
                    onToggle={() => toggleExpanded(orch.instance_id)}
                  />
                ))}
              </tbody>
            </table>
            <div className="flex justify-between items-center pt-3 text-xs text-muted-foreground">
              <span>Showing {data.orchestrations.length} of {data.count} function instances</span>
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
