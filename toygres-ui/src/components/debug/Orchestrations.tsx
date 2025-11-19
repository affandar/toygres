import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { X, ChevronDown, ChevronRight, StopCircle, RefreshCw, AlertTriangle } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useToast } from '@/lib/toast';
import { api } from '@/lib/api';
import { formatRelativeTime, getStatusIcon } from '@/lib/utils';

export function Orchestrations() {
  const [selectedOrch, setSelectedOrch] = useState<string | null>(null);
  const [expandedEvents, setExpandedEvents] = useState<Set<number>>(new Set());
  const [showCancelModal, setShowCancelModal] = useState(false);
  const [showRecreateModal, setShowRecreateModal] = useState(false);
  const [historyLimit, setHistoryLimit] = useState<'full' | '5' | '10'>('5');
  const [showActiveOnly, setShowActiveOnly] = useState(false);
  const queryClient = useQueryClient();
  const { showToast } = useToast();

  const { data: orchestrations } = useQuery({
    queryKey: ['orchestrations'],
    queryFn: () => api.listOrchestrations(),
    refetchInterval: 5000,
  });

  const { data: orchDetail } = useQuery({
    queryKey: ['orchestration', selectedOrch, historyLimit],
    queryFn: () => api.getOrchestration(selectedOrch!, historyLimit),
    enabled: !!selectedOrch,
  });

  const cancelMutation = useMutation({
    mutationFn: (id: string) => api.cancelOrchestration(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['orchestrations'] });
      showToast('success', 'Orchestration cancelled successfully');
      setShowCancelModal(false);
    },
    onError: (error: Error) => {
      showToast('error', `Failed to cancel: ${error.message}`);
    },
  });

  const recreateMutation = useMutation({
    mutationFn: (id: string) => api.recreateOrchestration(id),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['orchestrations'] });
      showToast('success', `Orchestration recreated as ${data.new_instance_id}`);
      setShowRecreateModal(false);
      setSelectedOrch(data.new_instance_id);
    },
    onError: (error: Error) => {
      showToast('error', `Failed to recreate: ${error.message}`);
    },
  });

  const handleCancel = () => {
    if (selectedOrch) {
      cancelMutation.mutate(selectedOrch);
    }
  };

  const handleRecreate = () => {
    if (selectedOrch) {
      recreateMutation.mutate(selectedOrch);
    }
  };

  const toggleEvent = (idx: number) => {
    const newExpanded = new Set(expandedEvents);
    if (newExpanded.has(idx)) {
      newExpanded.delete(idx);
    } else {
      newExpanded.add(idx);
    }
    setExpandedEvents(newExpanded);
  };

  const parseEvent = (eventStr: string) => {
    try {
      // Extract event type and details
      const typeMatch = eventStr.match(/^(\w+)\s*\{/);
      const eventType = typeMatch ? typeMatch[1] : 'Unknown';
      
      // Extract key fields
      const eventIdMatch = eventStr.match(/event_id:\s*(\d+)/);
      const executionIdMatch = eventStr.match(/execution_id:\s*(\d+)/);
      const nameMatch = eventStr.match(/name:\s*"([^"]+)"/);
      const sourceMatch = eventStr.match(/source_event_id:\s*(\d+)/);
      
      return {
        type: eventType,
        eventId: eventIdMatch ? parseInt(eventIdMatch[1]) : 0,
        executionId: executionIdMatch ? parseInt(executionIdMatch[1]) : 0,
        name: nameMatch ? nameMatch[1] : undefined,
        sourceEventId: sourceMatch ? parseInt(sourceMatch[1]) : undefined,
        raw: eventStr,
      };
    } catch {
      return {
        type: 'Unknown',
        eventId: 0,
        executionId: 0,
        raw: eventStr,
      };
    }
  };

  const getEventColor = (eventType: string) => {
    if (eventType.includes('Failed')) return 'text-red-600 dark:text-red-400';
    if (eventType.includes('Completed')) return 'text-green-600 dark:text-green-400';
    if (eventType.includes('Scheduled')) return 'text-blue-600 dark:text-blue-400';
    if (eventType.includes('Started')) return 'text-purple-600 dark:text-purple-400';
    return 'text-gray-600 dark:text-gray-400';
  };

  // Separate instance actors from other orchestrations
  const allInstanceActors = orchestrations?.filter(o => 
    o.orchestration_name.includes('instance-actor')
  ) || [];
  
  const allOtherOrchestrations = orchestrations?.filter(o => 
    !o.orchestration_name.includes('instance-actor')
  ) || [];

  // Apply active-only filter if enabled
  const instanceActors = showActiveOnly
    ? allInstanceActors.filter(o => o.status === 'Running')
    : allInstanceActors;
  
  const otherOrchestrations = showActiveOnly
    ? allOtherOrchestrations.filter(o => o.status === 'Running')
    : allOtherOrchestrations;

  const OrchestrationTable = ({ orchs }: { orchs: typeof orchestrations }) => (
    <div className="overflow-x-auto">
      <table className="w-full">
        <thead>
          <tr className="border-b text-left text-sm text-muted-foreground">
            <th className="pb-3 font-medium">ID</th>
            <th className="pb-3 font-medium">Type</th>
            <th className="pb-3 font-medium">Status</th>
            <th className="pb-3 font-medium">Exec</th>
            <th className="pb-3 font-medium">Started</th>
          </tr>
        </thead>
        <tbody>
          {orchs?.map((orch) => {
            const shortType = orch.orchestration_name.split('::').pop() || orch.orchestration_name;
            return (
              <tr
                key={orch.instance_id}
                className="border-b cursor-pointer hover:bg-accent/50 transition-colors"
                onClick={() => setSelectedOrch(orch.instance_id)}
              >
                <td className="py-3 font-mono text-xs">{orch.instance_id}</td>
                <td className="py-3 text-sm">{shortType}</td>
                <td className="py-3 text-sm">
                  {getStatusIcon(orch.status)} {orch.status}
                </td>
                <td className="py-3 text-sm">#{orch.current_execution_id}</td>
                <td className="py-3 text-sm text-muted-foreground">
                  {formatRelativeTime(orch.created_at)}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Orchestrations</h1>
          <p className="text-muted-foreground">
            Advanced orchestration diagnostics and debugging
          </p>
        </div>
        <div className="flex items-center gap-2">
          <label htmlFor="show-active" className="text-sm text-muted-foreground">
            Show:
          </label>
          <select
            id="show-active"
            value={showActiveOnly ? 'active' : 'all'}
            onChange={(e) => setShowActiveOnly(e.target.value === 'active')}
            className="text-sm border rounded px-3 py-1.5 bg-background"
          >
            <option value="all">All Orchestrations</option>
            <option value="active">Active Only</option>
          </select>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm">Total</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{orchestrations?.length || 0}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm">Instance Actors</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{instanceActors.length}</div>
            <p className="text-xs text-muted-foreground">Health monitors</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm">Operations</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{otherOrchestrations.length}</div>
            <p className="text-xs text-muted-foreground">Create/Delete</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Instance Actors ({instanceActors.length})</CardTitle>
          <p className="text-sm text-muted-foreground mt-1">
            Continuous health monitoring orchestrations (one per instance)
          </p>
        </CardHeader>
        <CardContent>
          {instanceActors.length === 0 ? (
            <p className="text-sm text-muted-foreground">No instance actors running</p>
          ) : (
            <OrchestrationTable orchs={instanceActors} />
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Operations ({otherOrchestrations.length})</CardTitle>
          <p className="text-sm text-muted-foreground mt-1">
            Instance lifecycle orchestrations (create, delete)
          </p>
        </CardHeader>
        <CardContent>
          {otherOrchestrations.length === 0 ? (
            <p className="text-sm text-muted-foreground">No operations found</p>
          ) : (
            <OrchestrationTable orchs={otherOrchestrations} />
          )}
        </CardContent>
      </Card>

      {selectedOrch && orchDetail && (
        <>
          <Card>
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle>Orchestration: {selectedOrch}</CardTitle>
                <div className="flex items-center space-x-2">
                  {orchDetail.status === 'Running' && (
                    <Button
                      size="sm"
                      variant="destructive"
                      onClick={() => setShowCancelModal(true)}
                    >
                      <StopCircle className="h-4 w-4 mr-1" />
                      Cancel
                    </Button>
                  )}
                  {(orchDetail.status === 'Failed' || orchDetail.status === 'Completed') && (
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => setShowRecreateModal(true)}
                    >
                      <RefreshCw className="h-4 w-4 mr-1" />
                      Recreate
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => {
                      setSelectedOrch(null);
                      setExpandedEvents(new Set());
                    }}
                  >
                    <X className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            </CardHeader>
            <CardContent className="space-y-4">
            <div className="grid grid-cols-2 gap-4 p-4 bg-muted/50 rounded-md">
              <div>
                <p className="text-sm text-muted-foreground">Status</p>
                <p className="text-sm font-medium">{getStatusIcon(orchDetail.status)} {orchDetail.status}</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Execution</p>
                <p className="text-sm">#{orchDetail.current_execution_id}</p>
              </div>
              <div className="col-span-2">
                <p className="text-sm text-muted-foreground">Type</p>
                <p className="text-xs font-mono">{orchDetail.orchestration_name}</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Version</p>
                <p className="text-sm">{orchDetail.orchestration_version}</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Started</p>
                <p className="text-sm">{formatRelativeTime(orchDetail.created_at)}</p>
              </div>
            </div>

            {orchDetail.output && (
              <div>
                <p className="text-sm font-medium mb-2">Output</p>
                <pre className="bg-muted p-3 rounded-md text-xs overflow-x-auto">
                  {orchDetail.output}
                </pre>
              </div>
            )}

            {orchDetail.history && orchDetail.history.length > 0 && (
              <div>
                <div className="flex items-center justify-between mb-3">
                  <p className="text-sm font-medium">
                    Execution History ({orchDetail.history.length} events)
                  </p>
                  <div className="flex items-center gap-2">
                    <label htmlFor="history-limit" className="text-xs text-muted-foreground">
                      Show:
                    </label>
                    <select
                      id="history-limit"
                      value={historyLimit}
                      onChange={(e) => setHistoryLimit(e.target.value as 'full' | '5' | '10')}
                      className="text-xs border rounded px-2 py-1 bg-background"
                    >
                      <option value="full">Full History</option>
                      <option value="5">Last 5</option>
                      <option value="10">Last 10</option>
                    </select>
                  </div>
                </div>
                <div className="border rounded-md">
                  <table className="w-full text-sm">
                    <thead className="bg-muted/50">
                      <tr className="border-b">
                        <th className="text-left p-2 font-medium w-12"></th>
                        <th className="text-left p-2 font-medium w-16">Event</th>
                        <th className="text-left p-2 font-medium w-16">Exec</th>
                        <th className="text-left p-2 font-medium">Type</th>
                        <th className="text-left p-2 font-medium">Details</th>
                      </tr>
                    </thead>
                    <tbody>
                      {orchDetail.history.map((historyItem, idx) => {
                        const parsed = parseEvent(historyItem.event);
                        const isExpanded = expandedEvents.has(idx);
                        const shortName = parsed.name?.split('::').pop() || parsed.name;
                        
                        return (
                          <tr key={idx} className="border-b hover:bg-muted/30">
                            <td className="p-2">
                              <button
                                onClick={() => toggleEvent(idx)}
                                className="hover:bg-accent rounded p-1"
                              >
                                {isExpanded ? (
                                  <ChevronDown className="h-4 w-4" />
                                ) : (
                                  <ChevronRight className="h-4 w-4" />
                                )}
                              </button>
                            </td>
                            <td className="p-2 font-mono text-xs text-muted-foreground">
                              {parsed.eventId}
                            </td>
                            <td className="p-2 font-mono text-xs text-muted-foreground">
                              #{parsed.executionId}
                            </td>
                            <td className={`p-2 text-xs font-medium ${getEventColor(parsed.type)}`}>
                              {parsed.type}
                            </td>
                            <td className="p-2">
                              {!isExpanded ? (
                                <div className="text-xs text-muted-foreground truncate">
                                  {shortName && <span className="font-mono">{shortName}</span>}
                                  {parsed.sourceEventId && (
                                    <span className="ml-2">→ event {parsed.sourceEventId}</span>
                                  )}
                                </div>
                              ) : (
                                <pre className="text-xs whitespace-pre-wrap bg-muted/50 p-2 rounded mt-2">
                                  {parsed.raw}
                                </pre>
                              )}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Cancel Confirmation Modal */}
        {showCancelModal && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
            <Card className="w-full max-w-md mx-4">
              <CardHeader>
                <CardTitle className="flex items-center space-x-2">
                  <AlertTriangle className="h-5 w-5 text-destructive" />
                  <span>Cancel Orchestration</span>
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <p className="text-sm">
                  Are you sure you want to cancel <strong>{selectedOrch}</strong>?
                </p>
                <div className="bg-destructive/10 border border-destructive/20 rounded-md p-3">
                  <p className="text-sm text-destructive font-medium">Warning:</p>
                  <ul className="text-sm text-destructive/80 mt-2 space-y-1 list-disc list-inside">
                    <li>The orchestration will be stopped immediately</li>
                    <li>Any in-progress activities may be left incomplete</li>
                    <li>This action cannot be undone</li>
                  </ul>
                </div>
                <p className="text-xs text-muted-foreground">
                  Note: Cancel is not yet implemented in Duroxide management API. This button is a placeholder.
                </p>
                <div className="flex justify-end space-x-3 pt-2">
                  <Button
                    variant="outline"
                    onClick={() => setShowCancelModal(false)}
                    disabled={cancelMutation.isPending}
                  >
                    Close
                  </Button>
                  <Button
                    variant="destructive"
                    onClick={handleCancel}
                    disabled={cancelMutation.isPending}
                  >
                    {cancelMutation.isPending ? 'Cancelling...' : 'Cancel Orchestration'}
                  </Button>
                </div>
              </CardContent>
            </Card>
          </div>
        )}

        {/* Recreate Confirmation Modal */}
        {showRecreateModal && orchDetail && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
            <Card className="w-full max-w-md mx-4">
              <CardHeader>
                <CardTitle className="flex items-center space-x-2">
                  <RefreshCw className="h-5 w-5 text-primary" />
                  <span>Recreate Orchestration</span>
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <p className="text-sm">
                  Create a new orchestration with the same parameters?
                </p>
                <div className="bg-muted/50 border rounded-md p-3 space-y-2">
                  <div className="flex justify-between text-sm">
                    <span className="text-muted-foreground">Original ID:</span>
                    <code className="text-xs">{selectedOrch}</code>
                  </div>
                  <div className="flex justify-between text-sm">
                    <span className="text-muted-foreground">Type:</span>
                    <code className="text-xs">{orchDetail.orchestration_name.split('::').pop()}</code>
                  </div>
                  <div className="flex justify-between text-sm">
                    <span className="text-muted-foreground">Version:</span>
                    <span className="text-xs">{orchDetail.orchestration_version}</span>
                  </div>
                  <div className="flex justify-between text-sm">
                    <span className="text-muted-foreground">Status:</span>
                    <span className="text-xs">{orchDetail.status}</span>
                  </div>
                </div>
                <p className="text-xs text-muted-foreground">
                  A new orchestration will be created with a new instance ID but identical input parameters and version.
                </p>
                <div className="flex justify-end space-x-3 pt-2">
                  <Button
                    variant="outline"
                    onClick={() => setShowRecreateModal(false)}
                    disabled={recreateMutation.isPending}
                  >
                    Cancel
                  </Button>
                  <Button
                    onClick={handleRecreate}
                    disabled={recreateMutation.isPending}
                  >
                    {recreateMutation.isPending ? 'Recreating...' : 'Recreate'}
                  </Button>
                </div>
              </CardContent>
            </Card>
          </div>
        )}
        </>
      )}
    </div>
  );
}

