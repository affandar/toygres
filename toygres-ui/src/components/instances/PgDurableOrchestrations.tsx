import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
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
  Ban,
  Send,
  Play,
  Info,
  History,
  X,
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useToast } from '@/lib/toast';
import { api } from '@/lib/api';

interface PgDurableOrchestrationsProps {
  instanceName: string;
}

// Status config for badges
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
    icon: <Ban className="h-4 w-4" />,
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

// ============================================================================
// Metrics Cards
// ============================================================================

function MetricsCards({ instanceName }: { instanceName: string }) {
  const { data, isLoading } = useQuery({
    queryKey: ['pg-durable-metrics', instanceName],
    queryFn: () => api.getPgDurableMetrics(instanceName),
    refetchInterval: 10000,
  });

  if (isLoading || !data) {
    return (
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        {[...Array(4)].map((_, i) => (
          <Card key={i}>
            <CardContent className="p-4">
              <div className="h-10 animate-pulse bg-muted rounded" />
            </CardContent>
          </Card>
        ))}
      </div>
    );
  }

  const metrics = [
    { label: 'Total', value: data.total_instances, color: 'text-foreground' },
    { label: 'Running', value: data.running_instances, color: 'text-blue-400' },
    { label: 'Completed', value: data.completed_instances, color: 'text-green-400' },
    { label: 'Failed', value: data.failed_instances, color: 'text-red-400' },
  ];

  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
      {metrics.map((m) => (
        <Card key={m.label}>
          <CardContent className="p-4">
            <p className="text-xs text-muted-foreground">{m.label}</p>
            <p className={`text-2xl font-bold ${m.color}`}>{m.value}</p>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

// ============================================================================
// Cancel Modal
// ============================================================================

function CancelModal({ instanceName, instanceId, onClose }: {
  instanceName: string;
  instanceId: string;
  onClose: () => void;
}) {
  const [reason, setReason] = useState('Cancelled by user');
  const queryClient = useQueryClient();
  const { showToast } = useToast();

  const cancelMutation = useMutation({
    mutationFn: () => api.cancelPgDurableInstance(instanceName, instanceId, reason),
    onSuccess: () => {
      showToast('success', `Cancelled ${instanceId}`);
      queryClient.invalidateQueries({ queryKey: ['pg-durable-functions'] });
      queryClient.invalidateQueries({ queryKey: ['pg-durable-metrics'] });
      onClose();
    },
    onError: (e: Error) => showToast('error', e.message),
  });

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <div className="bg-background border rounded-lg p-6 w-full max-w-md" onClick={e => e.stopPropagation()}>
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold">Cancel Durable Function</h3>
          <button onClick={onClose} className="text-muted-foreground hover:text-foreground">
            <X className="h-5 w-5" />
          </button>
        </div>
        <p className="text-sm text-muted-foreground mb-4">
          Cancel instance <span className="font-mono text-foreground">{instanceId}</span>?
        </p>
        <label className="block text-sm font-medium mb-1">Reason</label>
        <input
          type="text"
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          className="w-full bg-muted border rounded px-3 py-2 text-sm mb-4"
        />
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={onClose}>Cancel</Button>
          <Button
            size="sm"
            onClick={() => cancelMutation.mutate()}
            disabled={cancelMutation.isPending}
            className="bg-red-600 hover:bg-red-700 text-white"
          >
            {cancelMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin mr-1" /> : <Ban className="h-4 w-4 mr-1" />}
            Confirm Cancel
          </Button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Signal Modal
// ============================================================================

function SignalModal({ instanceName, instanceId, onClose }: {
  instanceName: string;
  instanceId: string;
  onClose: () => void;
}) {
  const [signalName, setSignalName] = useState('');
  const [signalData, setSignalData] = useState('{}');
  const queryClient = useQueryClient();
  const { showToast } = useToast();

  const signalMutation = useMutation({
    mutationFn: () => api.signalPgDurableInstance(instanceName, instanceId, signalName, signalData),
    onSuccess: () => {
      showToast('success', `Signal "${signalName}" sent to ${instanceId}`);
      queryClient.invalidateQueries({ queryKey: ['pg-durable-functions'] });
      onClose();
    },
    onError: (e: Error) => showToast('error', e.message),
  });

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <div className="bg-background border rounded-lg p-6 w-full max-w-md" onClick={e => e.stopPropagation()}>
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold">Send Signal</h3>
          <button onClick={onClose} className="text-muted-foreground hover:text-foreground">
            <X className="h-5 w-5" />
          </button>
        </div>
        <p className="text-sm text-muted-foreground mb-4">
          Send a signal to <span className="font-mono text-foreground">{instanceId}</span>
        </p>
        <label className="block text-sm font-medium mb-1">Signal Name</label>
        <input
          type="text"
          value={signalName}
          onChange={(e) => setSignalName(e.target.value)}
          placeholder="e.g. approval, continue, cancel"
          className="w-full bg-muted border rounded px-3 py-2 text-sm mb-3"
        />
        <label className="block text-sm font-medium mb-1">Signal Data (JSON)</label>
        <textarea
          value={signalData}
          onChange={(e) => setSignalData(e.target.value)}
          rows={3}
          className="w-full bg-muted border rounded px-3 py-2 text-sm font-mono mb-4"
        />
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={onClose}>Cancel</Button>
          <Button
            size="sm"
            onClick={() => signalMutation.mutate()}
            disabled={signalMutation.isPending || !signalName.trim()}
          >
            {signalMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin mr-1" /> : <Send className="h-4 w-4 mr-1" />}
            Send Signal
          </Button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Create Function Panel
// ============================================================================

function CreateFunctionPanel({ instanceName, onClose }: {
  instanceName: string;
  onClose: () => void;
}) {
  const [expression, setExpression] = useState('');
  const [label, setLabel] = useState('');
  const [variables, setVariables] = useState<Array<{ key: string; value: string }>>([]);
  const queryClient = useQueryClient();
  const { showToast } = useToast();

  const startMutation = useMutation({
    mutationFn: () => {
      const vars: Record<string, string> = {};
      variables.forEach(v => { if (v.key.trim()) vars[v.key] = v.value; });
      return api.startPgDurableFunction(instanceName, expression, label || undefined, Object.keys(vars).length > 0 ? vars : undefined);
    },
    onSuccess: (data) => {
      showToast('success', `Started durable function: ${data.instance_id}`);
      queryClient.invalidateQueries({ queryKey: ['pg-durable-functions'] });
      queryClient.invalidateQueries({ queryKey: ['pg-durable-metrics'] });
      onClose();
    },
    onError: (e: Error) => showToast('error', e.message),
  });

  const addVariable = () => setVariables([...variables, { key: '', value: '' }]);
  const removeVariable = (index: number) => setVariables(variables.filter((_, i) => i !== index));
  const updateVariable = (index: number, field: 'key' | 'value', value: string) => {
    const updated = [...variables];
    updated[index][field] = value;
    setVariables(updated);
  };

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-lg">Create Durable Function</CardTitle>
        <button onClick={onClose} className="text-muted-foreground hover:text-foreground">
          <X className="h-5 w-5" />
        </button>
      </CardHeader>
      <CardContent className="space-y-4">
        <div>
          <label className="block text-sm font-medium mb-1">Label (optional)</label>
          <input
            type="text"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="my-workflow"
            className="w-full bg-muted border rounded px-3 py-2 text-sm"
          />
        </div>
        <div>
          <label className="block text-sm font-medium mb-1">DSL Expression</label>
          <textarea
            value={expression}
            onChange={(e) => setExpression(e.target.value)}
            rows={6}
            placeholder={"df.sql('SELECT 1') ~> df.sql('SELECT 2')"}
            className="w-full bg-slate-900 text-slate-200 border rounded px-3 py-2 text-sm font-mono"
          />
          <p className="text-xs text-muted-foreground mt-1">
            Use pg_durable DSL: df.sql(), df.sleep(), df.http(), ~&gt; (sequence), &amp; (parallel), | (race)
          </p>
        </div>
        {variables.length > 0 && (
          <div>
            <label className="block text-sm font-medium mb-1">Variables</label>
            {variables.map((v, i) => (
              <div key={i} className="flex gap-2 mb-2">
                <input
                  type="text"
                  value={v.key}
                  onChange={(e) => updateVariable(i, 'key', e.target.value)}
                  placeholder="name"
                  className="flex-1 bg-muted border rounded px-3 py-1.5 text-sm font-mono"
                />
                <input
                  type="text"
                  value={v.value}
                  onChange={(e) => updateVariable(i, 'value', e.target.value)}
                  placeholder="value"
                  className="flex-1 bg-muted border rounded px-3 py-1.5 text-sm font-mono"
                />
                <button onClick={() => removeVariable(i)} className="text-muted-foreground hover:text-red-400">
                  <X className="h-4 w-4" />
                </button>
              </div>
            ))}
          </div>
        )}
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={addVariable}>
            + Add Variable
          </Button>
        </div>
        <div className="flex justify-end gap-2 pt-2 border-t">
          <Button variant="outline" size="sm" onClick={onClose}>Cancel</Button>
          <Button
            size="sm"
            onClick={() => startMutation.mutate()}
            disabled={startMutation.isPending || !expression.trim()}
          >
            {startMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin mr-1" /> : <Play className="h-4 w-4 mr-1" />}
            Start Function
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

// ============================================================================
// Instance Detail Panel
// ============================================================================

function InstanceDetailPanel({ instanceName, instanceId, onClose }: {
  instanceName: string;
  instanceId: string;
  onClose: () => void;
}) {
  const [activeTab, setActiveTab] = useState<'info' | 'executions' | 'nodes' | 'explain'>('info');

  const { data: info, isLoading: infoLoading } = useQuery({
    queryKey: ['pg-durable-info', instanceName, instanceId],
    queryFn: () => api.getPgDurableInstanceInfo(instanceName, instanceId),
    refetchInterval: activeTab === 'info' ? 5000 : false,
  });

  const { data: executions, isLoading: execLoading } = useQuery({
    queryKey: ['pg-durable-executions', instanceName, instanceId],
    queryFn: () => api.getPgDurableInstanceExecutions(instanceName, instanceId, 10),
    enabled: activeTab === 'executions',
  });

  const { data: nodes, isLoading: nodesLoading } = useQuery({
    queryKey: ['pg-durable-nodes', instanceName, instanceId],
    queryFn: () => api.getPgDurableInstanceNodes(instanceName, instanceId, 5),
    enabled: activeTab === 'nodes',
    refetchInterval: activeTab === 'nodes' && info?.status?.toLowerCase() === 'running' ? 3000 : false,
  });

  const { data: explain, isLoading: explainLoading, isFetching: explainFetching } = useQuery({
    queryKey: ['pg-durable-explain', instanceName, instanceId],
    queryFn: () => api.getPgDurableExplain(instanceName, instanceId),
    enabled: activeTab === 'explain',
    refetchInterval: activeTab === 'explain' && info?.status?.toLowerCase() === 'running' ? 3000 : false,
  });

  const statusConfig = info ? getStatusConfig(info.status) : null;

  const tabs = [
    { id: 'info' as const, label: 'Info', icon: <Info className="h-3.5 w-3.5" /> },
    { id: 'executions' as const, label: 'Executions', icon: <History className="h-3.5 w-3.5" /> },
    { id: 'nodes' as const, label: 'Nodes', icon: <ChevronRight className="h-3.5 w-3.5" /> },
    { id: 'explain' as const, label: 'Explain', icon: <AlertTriangle className="h-3.5 w-3.5" /> },
  ];

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <div>
          <CardTitle className="text-lg font-mono">{instanceId}</CardTitle>
          {info && statusConfig && (
            <div className="flex items-center gap-2 mt-1">
              <span className={`inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded ${statusConfig.bgColor} ${statusConfig.color}`}>
                {statusConfig.icon}
                {statusConfig.label}
              </span>
              {info.label && <span className="text-xs text-muted-foreground">{info.label}</span>}
            </div>
          )}
        </div>
        <button onClick={onClose} className="text-muted-foreground hover:text-foreground">
          <X className="h-5 w-5" />
        </button>
      </CardHeader>
      <CardContent>
        {/* Tabs */}
        <div className="flex gap-1 mb-4 border-b">
          {tabs.map(tab => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium border-b-2 transition-colors ${
                activeTab === tab.id
                  ? 'border-primary text-foreground'
                  : 'border-transparent text-muted-foreground hover:text-foreground'
              }`}
            >
              {tab.icon}
              {tab.label}
            </button>
          ))}
        </div>

        {/* Info Tab */}
        {activeTab === 'info' && (
          infoLoading ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground p-4">
              <RefreshCw className="h-4 w-4 animate-spin" /> Loading...
            </div>
          ) : info ? (
            <div className="grid grid-cols-2 gap-3 text-sm">
              <div><span className="text-muted-foreground">Instance ID:</span> <span className="font-mono">{info.instance_id}</span></div>
              <div><span className="text-muted-foreground">Status:</span> {info.status}</div>
              <div><span className="text-muted-foreground">Function:</span> {info.function_name || '—'}</div>
              <div><span className="text-muted-foreground">Version:</span> {info.function_version || '—'}</div>
              <div><span className="text-muted-foreground">Label:</span> {info.label || '—'}</div>
              <div><span className="text-muted-foreground">Execution:</span> #{info.current_execution_id ?? '—'}</div>
              {info.output && (
                <div className="col-span-2">
                  <span className="text-muted-foreground">Output:</span>
                  <pre className="mt-1 font-mono text-xs bg-muted p-2 rounded overflow-x-auto">{info.output}</pre>
                </div>
              )}
            </div>
          ) : null
        )}

        {/* Executions Tab */}
        {activeTab === 'executions' && (
          execLoading ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground p-4">
              <RefreshCw className="h-4 w-4 animate-spin" /> Loading...
            </div>
          ) : executions && executions.executions.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b bg-muted/50">
                    <th className="text-left p-2 font-medium">Exec #</th>
                    <th className="text-left p-2 font-medium">Status</th>
                    <th className="text-right p-2 font-medium">Events</th>
                    <th className="text-right p-2 font-medium">Duration</th>
                    <th className="text-left p-2 font-medium">Output</th>
                  </tr>
                </thead>
                <tbody>
                  {executions.executions.map((ex) => {
                    const sc = getStatusConfig(ex.status);
                    return (
                      <tr key={ex.execution_id} className="border-b hover:bg-muted/30">
                        <td className="p-2 font-mono text-xs">#{ex.execution_id}</td>
                        <td className="p-2">
                          <span className={`inline-flex items-center gap-1 text-xs px-1.5 py-0.5 rounded ${sc.bgColor} ${sc.color}`}>
                            {sc.icon} {sc.label}
                          </span>
                        </td>
                        <td className="p-2 text-right text-muted-foreground">{ex.event_count}</td>
                        <td className="p-2 text-right text-muted-foreground">
                          {ex.duration_ms != null ? `${(ex.duration_ms / 1000).toFixed(1)}s` : '—'}
                        </td>
                        <td className="p-2 text-xs font-mono truncate max-w-[200px]">{ex.output || '—'}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          ) : (
            <p className="text-sm text-muted-foreground p-4">No executions found</p>
          )
        )}

        {/* Nodes Tab */}
        {activeTab === 'nodes' && (
          nodesLoading ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground p-4">
              <RefreshCw className="h-4 w-4 animate-spin" /> Loading...
            </div>
          ) : nodes && nodes.nodes.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b bg-muted/50">
                    <th className="text-left p-2 font-medium">Node</th>
                    <th className="text-left p-2 font-medium">Type</th>
                    <th className="text-left p-2 font-medium">Query</th>
                    <th className="text-left p-2 font-medium">Status</th>
                    <th className="text-left p-2 font-medium">Result</th>
                  </tr>
                </thead>
                <tbody>
                  {nodes.nodes.map((node, i) => {
                    const sc = getStatusConfig(node.status);
                    return (
                      <tr key={`${node.execution_id}-${node.node_id}-${i}`} className="border-b hover:bg-muted/30">
                        <td className="p-2 font-mono">{node.node_id}{node.result_name ? ` (${node.result_name})` : ''}</td>
                        <td className="p-2">
                          <span className="bg-muted px-1.5 py-0.5 rounded">{node.node_type}</span>
                        </td>
                        <td className="p-2 font-mono truncate max-w-[250px]">{node.query || '—'}</td>
                        <td className="p-2">
                          <span className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded ${sc.bgColor} ${sc.color}`}>
                            {sc.icon} {sc.label}
                          </span>
                        </td>
                        <td className="p-2 font-mono truncate max-w-[200px]">{node.result || '—'}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          ) : (
            <p className="text-sm text-muted-foreground p-4">No nodes found</p>
          )
        )}

        {/* Explain Tab */}
        {activeTab === 'explain' && (
          explainLoading ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground p-4">
              <RefreshCw className="h-4 w-4 animate-spin" /> Loading...
            </div>
          ) : explain ? (
            <div>
              {explainFetching && !explainLoading && (
                <div className="flex items-center gap-1 mb-2 text-xs text-muted-foreground">
                  <RefreshCw className="h-3 w-3 animate-spin" /> Refreshing...
                </div>
              )}
              <pre className="font-mono text-xs bg-slate-900 text-slate-200 p-4 rounded overflow-x-auto whitespace-pre">
                {explain.explain}
              </pre>
            </div>
          ) : (
            <p className="text-sm text-muted-foreground p-4">No data available</p>
          )
        )}
      </CardContent>
    </Card>
  );
}

// ============================================================================
// Main Component: Expandable row
// ============================================================================

function ExpandableInstanceRow({ 
  instance, 
  pgInstanceName,
  isExpanded,
  onToggle,
  onCancel,
  onSignal,
  onDetail,
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
  onCancel: (id: string) => void;
  onSignal: (id: string) => void;
  onDetail: (id: string) => void;
}) {
  const statusConfig = getStatusConfig(instance.status);
  const isRunning = instance.status.toLowerCase() === 'running';

  const { data: explainData, isLoading: explainLoading, isFetching: explainFetching } = useQuery({
    queryKey: ['pg-durable-explain', pgInstanceName, instance.instance_id],
    queryFn: () => api.getPgDurableExplain(pgInstanceName, instance.instance_id),
    enabled: isExpanded,
    refetchInterval: isExpanded && isRunning ? 3000 : false,
    staleTime: 2000,
  });

  return (
    <>
      <tr className="border-b hover:bg-muted/30 cursor-pointer" onClick={onToggle}>
        <td className="p-3">
          <button className="text-muted-foreground hover:text-foreground">
            {isExpanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
          </button>
        </td>
        <td className="p-3 font-mono text-xs">
          <button
            onClick={(e) => { e.stopPropagation(); onDetail(instance.instance_id); }}
            className="text-blue-400 hover:text-blue-300 hover:underline"
            title="View details"
          >
            {instance.instance_id}
          </button>
        </td>
        <td className="p-3 text-sm">{instance.label || '—'}</td>
        <td className="p-3 text-sm text-muted-foreground">{instance.function_name || '—'}</td>
        <td className="p-3">
          <span className={`inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded ${statusConfig.bgColor} ${statusConfig.color}`}>
            {statusConfig.icon}
            {statusConfig.label}
          </span>
        </td>
        <td className="p-3 text-center text-sm">{instance.execution_count}</td>
        <td className="p-3">
          <div className="flex items-center gap-1" onClick={e => e.stopPropagation()}>
            <button
              onClick={() => onDetail(instance.instance_id)}
              className="p-1 text-muted-foreground hover:text-foreground rounded hover:bg-muted"
              title="View Details"
            >
              <Info className="h-3.5 w-3.5" />
            </button>
            {isRunning && (
              <>
                <button
                  onClick={() => onCancel(instance.instance_id)}
                  className="p-1 text-muted-foreground hover:text-red-400 rounded hover:bg-muted"
                  title="Cancel"
                >
                  <Ban className="h-3.5 w-3.5" />
                </button>
                <button
                  onClick={() => onSignal(instance.instance_id)}
                  className="p-1 text-muted-foreground hover:text-blue-400 rounded hover:bg-muted"
                  title="Send Signal"
                >
                  <Send className="h-3.5 w-3.5" />
                </button>
              </>
            )}
          </div>
        </td>
      </tr>
      
      {isExpanded && (
        <tr className="bg-muted/20">
          <td colSpan={7} className="p-0">
            <div className="p-4">
              <div className="flex items-center gap-2 mb-3">
                <h4 className="text-sm font-medium">Function Graph</h4>
                {explainFetching && !explainLoading && (
                  <RefreshCw className="h-3 w-3 animate-spin text-muted-foreground" />
                )}
              </div>
              {explainLoading ? (
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <RefreshCw className="h-4 w-4 animate-spin" /> Loading...
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

// ============================================================================
// Main PgDurableOrchestrations Component
// ============================================================================

export function PgDurableOrchestrations({ instanceName }: PgDurableOrchestrationsProps) {
  const [limit, setLimit] = useState(50);
  const [statusFilter, setStatusFilter] = useState<string>('');
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [showCreate, setShowCreate] = useState(false);
  const [detailId, setDetailId] = useState<string | null>(null);
  const [cancelId, setCancelId] = useState<string | null>(null);
  const [signalId, setSignalId] = useState<string | null>(null);
  const { showToast } = useToast();
  const queryClient = useQueryClient();

  const { data, isLoading, error, refetch, isFetching } = useQuery({
    queryKey: ['pg-durable-functions', instanceName, limit, statusFilter],
    queryFn: () => api.getPgDurableFunctions(instanceName, limit, statusFilter || undefined),
    refetchInterval: 10000,
    staleTime: 5000,
  });

  const runMutation = useMutation({
    mutationFn: () => api.runPgDurable(instanceName),
    onSuccess: () => {
      showToast('success', 'Triggered run for pending functions');
      queryClient.invalidateQueries({ queryKey: ['pg-durable-functions'] });
    },
    onError: (e: Error) => showToast('error', e.message),
  });

  const toggleExpanded = (id: string) => {
    setExpandedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  return (
    <div className="space-y-4">
      {/* Metrics */}
      <MetricsCards instanceName={instanceName} />

      {/* Create Panel */}
      {showCreate && (
        <CreateFunctionPanel instanceName={instanceName} onClose={() => setShowCreate(false)} />
      )}

      {/* Detail Panel */}
      {detailId && (
        <InstanceDetailPanel instanceName={instanceName} instanceId={detailId} onClose={() => setDetailId(null)} />
      )}

      {/* Cancel Modal */}
      {cancelId && (
        <CancelModal instanceName={instanceName} instanceId={cancelId} onClose={() => setCancelId(null)} />
      )}

      {/* Signal Modal */}
      {signalId && (
        <SignalModal instanceName={instanceName} instanceId={signalId} onClose={() => setSignalId(null)} />
      )}

      {/* Functions Table */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-lg flex items-center gap-2">
            ⚡ Durable SQL Functions
            <span className="text-xs bg-blue-500/20 text-blue-400 px-2 py-0.5 rounded font-normal">
              pg_durable
            </span>
          </CardTitle>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowCreate(!showCreate)}
            >
              <Play className="h-3 w-3 mr-1" />
              Create
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => runMutation.mutate()}
              disabled={runMutation.isPending}
              title="Trigger pending functions"
            >
              {runMutation.isPending ? <Loader2 className="h-3 w-3 animate-spin" /> : <PlayCircle className="h-3 w-3" />}
            </Button>
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
              <Button variant="outline" size="sm" onClick={() => refetch()} className="mt-4">
                Retry
              </Button>
            </div>
          ) : data && data.functions && data.functions.length > 0 ? (
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
                    <th className="text-left p-3 font-medium w-20">Actions</th>
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
                      onCancel={(id) => setCancelId(id)}
                      onSignal={(id) => setSignalId(id)}
                      onDetail={(id) => setDetailId(id)}
                    />
                  ))}
                </tbody>
              </table>
              <div className="flex justify-between items-center pt-3 text-xs text-muted-foreground">
                <span>Showing {data.functions.length} of {data.count} function instances</span>
                <span>Click a row to view graph · Click ID for details</span>
              </div>
            </div>
          ) : (
            <div className="flex flex-col items-center justify-center h-48 text-center">
              <p className="text-sm text-muted-foreground">No durable functions found</p>
              <p className="text-xs text-muted-foreground mt-1">
                {statusFilter ? `No functions with status "${statusFilter}"` : 'Create a durable SQL function to get started'}
              </p>
              <Button variant="outline" size="sm" onClick={() => setShowCreate(true)} className="mt-4">
                <Play className="h-3 w-3 mr-1" /> Create Function
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
