import { useState, useCallback, useEffect, useRef } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { X, ChevronDown, ChevronRight, ChevronLeft, StopCircle, RefreshCw, AlertTriangle, TableIcon, GitBranch } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useToast } from '@/lib/toast';
import { api } from '@/lib/api';
import { formatRelativeTime, getStatusIcon } from '@/lib/utils';
import mermaid from 'mermaid';

// Initialize mermaid
mermaid.initialize({
  startOnLoad: false,
  theme: 'neutral',
  flowchart: {
    useMaxWidth: true,
    htmlLabels: true,
    curve: 'basis',
  },
});

export function Orchestrations() {
  const [selectedOrchs, setSelectedOrchs] = useState<Set<string>>(new Set());
  const [lastClickedOrch, setLastClickedOrch] = useState<string | null>(null);
  const [expandedEvents, setExpandedEvents] = useState<Set<number>>(new Set());
  const [showCancelModal, setShowCancelModal] = useState(false);
  const [showRecreateModal, setShowRecreateModal] = useState(false);
  const [showRaiseEventModal, setShowRaiseEventModal] = useState(false);
  const [eventName, setEventName] = useState('InstanceDeleted');
  const [eventData, setEventData] = useState('{}');
  const [historyLimit, setHistoryLimit] = useState<'full' | '5' | '10'>('5');
  const [historyView, setHistoryView] = useState<'table' | 'graph'>('table');
  const [showActiveOnly, setShowActiveOnly] = useState(false);
  const [bulkActionType, setBulkActionType] = useState<'cancel' | 'recreate' | null>(null);
  const [actorsPage, setActorsPage] = useState(1);
  const [operationsPage, setOperationsPage] = useState(1);
  const pageSize = 10;
  const queryClient = useQueryClient();
  const { showToast } = useToast();
  
  // For detail view, use the first selected orchestration
  const selectedOrch = selectedOrchs.size === 1 ? Array.from(selectedOrchs)[0] : null;

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

  // Fetch static flow diagram for the orchestration
  const { data: staticFlow } = useQuery({
    queryKey: ['orchestration-flow', orchDetail?.orchestration_name],
    queryFn: () => api.getOrchestrationFlow(orchDetail!.orchestration_name),
    enabled: !!orchDetail?.orchestration_name,
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
      setSelectedOrchs(new Set([data.new_instance_id]));
    },
    onError: (error: Error) => {
      showToast('error', `Failed to recreate: ${error.message}`);
    },
  });

  // Bulk operations
  const bulkCancelMutation = useMutation({
    mutationFn: async (ids: string[]) => {
      const results = await Promise.allSettled(ids.map(id => api.cancelOrchestration(id)));
      const succeeded = results.filter(r => r.status === 'fulfilled').length;
      const failed = results.filter(r => r.status === 'rejected').length;
      return { succeeded, failed };
    },
    onSuccess: ({ succeeded, failed }) => {
      queryClient.invalidateQueries({ queryKey: ['orchestrations'] });
      if (failed === 0) {
        showToast('success', `${succeeded} orchestration(s) cancelled`);
      } else {
        showToast('error', `${succeeded} cancelled, ${failed} failed`);
      }
      setShowCancelModal(false);
      setBulkActionType(null);
      setSelectedOrchs(new Set());
    },
    onError: (error: Error) => {
      showToast('error', `Bulk cancel failed: ${error.message}`);
    },
  });

  const bulkRecreateMutation = useMutation({
    mutationFn: async (ids: string[]) => {
      const results = await Promise.allSettled(ids.map(id => api.recreateOrchestration(id)));
      const succeeded = results.filter(r => r.status === 'fulfilled').length;
      const failed = results.filter(r => r.status === 'rejected').length;
      return { succeeded, failed };
    },
    onSuccess: ({ succeeded, failed }) => {
      queryClient.invalidateQueries({ queryKey: ['orchestrations'] });
      if (failed === 0) {
        showToast('success', `${succeeded} orchestration(s) recreated`);
      } else {
        showToast('error', `${succeeded} recreated, ${failed} failed`);
      }
      setShowRecreateModal(false);
      setBulkActionType(null);
      setSelectedOrchs(new Set());
    },
    onError: (error: Error) => {
      showToast('error', `Bulk recreate failed: ${error.message}`);
    },
  });

  const raiseEventMutation = useMutation({
    mutationFn: ({ id, name, data }: { id: string; name: string; data: string }) =>
      api.raiseEvent(id, name, data),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['orchestration', selectedOrch, historyLimit] });
      showToast('success', `Event "${data.event_name}" raised successfully`);
      setShowRaiseEventModal(false);
    },
    onError: (error: Error) => {
      showToast('error', `Failed to raise event: ${error.message}`);
    },
  });

  const handleCancel = () => {
    if (bulkActionType === 'cancel' && selectedOrchs.size > 1) {
      bulkCancelMutation.mutate(Array.from(selectedOrchs));
    } else if (selectedOrch) {
      cancelMutation.mutate(selectedOrch);
    }
  };

  const handleRecreate = () => {
    if (bulkActionType === 'recreate' && selectedOrchs.size > 1) {
      bulkRecreateMutation.mutate(Array.from(selectedOrchs));
    } else if (selectedOrch) {
      recreateMutation.mutate(selectedOrch);
    }
  };

  // Multi-select row click handler
  const handleRowClick = useCallback((
    orchId: string,
    event: React.MouseEvent,
    allOrchIds: string[]
  ) => {
    if (event.shiftKey && lastClickedOrch) {
      // Shift+click: range selection
      const startIdx = allOrchIds.indexOf(lastClickedOrch);
      const endIdx = allOrchIds.indexOf(orchId);
      if (startIdx !== -1 && endIdx !== -1) {
        const [from, to] = startIdx < endIdx ? [startIdx, endIdx] : [endIdx, startIdx];
        const rangeIds = allOrchIds.slice(from, to + 1);
        setSelectedOrchs(prev => {
          const next = new Set(prev);
          rangeIds.forEach(id => next.add(id));
          return next;
        });
      }
    } else if (event.ctrlKey || event.metaKey) {
      // Ctrl/Cmd+click: toggle individual selection
      setSelectedOrchs(prev => {
        const next = new Set(prev);
        if (next.has(orchId)) {
          next.delete(orchId);
        } else {
          next.add(orchId);
        }
        return next;
      });
      setLastClickedOrch(orchId);
    } else {
      // Regular click: single selection
      setSelectedOrchs(new Set([orchId]));
      setLastClickedOrch(orchId);
    }
  }, [lastClickedOrch]);

  // Clear selection
  const clearSelection = () => {
    setSelectedOrchs(new Set());
    setLastClickedOrch(null);
    setExpandedEvents(new Set());
  };

  const handleRaiseEvent = () => {
    if (selectedOrch) {
      raiseEventMutation.mutate({ id: selectedOrch, name: eventName, data: eventData });
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
      // Extract event type - now inside "kind: TypeName { ... }" since Event is a struct
      // Try new format first: "Event { ..., kind: TypeName { ... } }"
      let eventType = 'Unknown';
      const kindMatch = eventStr.match(/kind:\s*(\w+)\s*[{\[]/);
      if (kindMatch) {
        eventType = kindMatch[1];
      } else {
        // Fallback: old format "TypeName { ... }" at start
        const typeMatch = eventStr.match(/^(\w+)\s*\{/);
        if (typeMatch) {
          eventType = typeMatch[1];
        }
      }
      
      // Extract key fields
      const eventIdMatch = eventStr.match(/event_id:\s*(\d+)/);
      const executionIdMatch = eventStr.match(/execution_id:\s*(\d+)/);
      // Look for name in the kind block - it may appear multiple times, get the one after "kind:"
      const kindSection = eventStr.match(/kind:\s*\w+\s*\{([^}]+)\}/);
      const nameMatch = kindSection 
        ? kindSection[1].match(/name:\s*"([^"]+)"/)
        : eventStr.match(/name:\s*"([^"]+)"/);
      const sourceMatch = eventStr.match(/source_event_id:\s*Some\((\d+)\)|source_event_id:\s*(\d+)/);
      const fireAtMatch = eventStr.match(/fire_at_ms:\s*(\d+)/);
      const durationMatch = eventStr.match(/duration_ms:\s*(\d+)/);
      
      return {
        type: eventType,
        eventId: eventIdMatch ? parseInt(eventIdMatch[1]) : 0,
        executionId: executionIdMatch ? parseInt(executionIdMatch[1]) : 0,
        name: nameMatch ? nameMatch[1] : undefined,
        sourceEventId: sourceMatch ? parseInt(sourceMatch[1] || sourceMatch[2]) : undefined,
        fireAtMs: fireAtMatch ? parseInt(fireAtMatch[1]) : undefined,
        durationMs: durationMatch ? parseInt(durationMatch[1]) : undefined,
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
    if (eventType.includes('Created')) return 'text-cyan-600 dark:text-cyan-400';
    if (eventType.includes('Fired')) return 'text-orange-600 dark:text-orange-400';
    if (eventType.includes('Started')) return 'text-purple-600 dark:text-purple-400';
    return 'text-gray-600 dark:text-gray-400';
  };

  // Color palette for linking source/completion events
  const linkColors = [
    'border-blue-400 bg-blue-50 dark:bg-blue-950/30',
    'border-emerald-400 bg-emerald-50 dark:bg-emerald-950/30',
    'border-violet-400 bg-violet-50 dark:bg-violet-950/30',
    'border-amber-400 bg-amber-50 dark:bg-amber-950/30',
    'border-rose-400 bg-rose-50 dark:bg-rose-950/30',
    'border-cyan-400 bg-cyan-50 dark:bg-cyan-950/30',
    'border-fuchsia-400 bg-fuchsia-50 dark:bg-fuchsia-950/30',
    'border-lime-400 bg-lime-50 dark:bg-lime-950/30',
  ];

  // Build a map of source events for correlation
  const buildEventCorrelation = (history: { event: string }[]) => {
    const parsed = history.map(h => parseEvent(h.event));
    const sourceMap = new Map<number, { name?: string; type: string; colorIndex: number }>();
    let colorIndex = 0;

    // First pass: identify all "source" events (Scheduled, Created)
    parsed.forEach(p => {
      if (p.type === 'ActivityScheduled' || p.type === 'TimerCreated' || 
          p.type === 'SubOrchestrationScheduled' || p.type === 'WaitScheduled') {
        sourceMap.set(p.eventId, { 
          name: p.name, 
          type: p.type,
          colorIndex: colorIndex++ % linkColors.length 
        });
      }
    });

    return { parsed, sourceMap };
  };

  // Generate Mermaid flowchart from execution history
  const generateMermaidDiagram = (history: { event: string }[]) => {
    const parsed = history.map(h => parseEvent(h.event));
    const lines: string[] = ['flowchart TD'];
    const nodeStyles: string[] = [];
    
    // Track pending operations to link completions
    const pendingOps = new Map<number, { nodeId: string; name: string; type: string }>();
    // Track the last node in the main flow to link the next scheduling event
    let lastFlowNodeId: string | null = null;
    
    parsed.forEach((event) => {
      const shortName = event.name?.split('::').pop() || event.name || '';
      const nodeId = `e${event.eventId}`;
      
      if (event.type === 'OrchestrationStarted') {
        lines.push(`  ${nodeId}(["▶ Start #${event.executionId}"])`);
        nodeStyles.push(`style ${nodeId} fill:#a855f7,color:#fff`);
        lastFlowNodeId = nodeId;
      } else if (event.type === 'ActivityScheduled') {
        const label = shortName.length > 25 ? shortName.substring(0, 22) + '...' : shortName;
        lines.push(`  ${nodeId}["📋 ${label}"]`);
        nodeStyles.push(`style ${nodeId} fill:#3b82f6,color:#fff`);
        pendingOps.set(event.eventId, { nodeId, name: shortName, type: 'activity' });
        
        // Link from last flow node
        if (lastFlowNodeId) {
          lines.push(`  ${lastFlowNodeId} --> ${nodeId}`);
        }
        // Don't update lastFlowNodeId yet - wait for completion
      } else if (event.type === 'ActivityCompleted') {
        const source = pendingOps.get(event.sourceEventId!);
        if (source) {
          lines.push(`  ${nodeId}(["✓"])`);
          nodeStyles.push(`style ${nodeId} fill:#22c55e,color:#fff`);
          lines.push(`  ${source.nodeId} --> ${nodeId}`);
          pendingOps.delete(event.sourceEventId!);
          lastFlowNodeId = nodeId; // Completion becomes the new flow point
        }
      } else if (event.type === 'ActivityFailed') {
        const source = pendingOps.get(event.sourceEventId!);
        if (source) {
          lines.push(`  ${nodeId}(["✗"])`);
          nodeStyles.push(`style ${nodeId} fill:#ef4444,color:#fff`);
          lines.push(`  ${source.nodeId} --> ${nodeId}`);
          pendingOps.delete(event.sourceEventId!);
          lastFlowNodeId = nodeId;
        }
      } else if (event.type === 'TimerCreated') {
        lines.push(`  ${nodeId}{{"⏱ Timer"}}`);
        nodeStyles.push(`style ${nodeId} fill:#06b6d4,color:#fff`);
        pendingOps.set(event.eventId, { nodeId, name: 'Timer', type: 'timer' });
        
        if (lastFlowNodeId) {
          lines.push(`  ${lastFlowNodeId} --> ${nodeId}`);
        }
      } else if (event.type === 'TimerFired') {
        const source = pendingOps.get(event.sourceEventId!);
        if (source) {
          lines.push(`  ${nodeId}(["🔔"])`);
          nodeStyles.push(`style ${nodeId} fill:#f97316,color:#fff`);
          lines.push(`  ${source.nodeId} --> ${nodeId}`);
          pendingOps.delete(event.sourceEventId!);
          lastFlowNodeId = nodeId;
        }
      } else if (event.type === 'SubOrchestrationScheduled') {
        const label = shortName.length > 20 ? shortName.substring(0, 17) + '...' : shortName;
        lines.push(`  ${nodeId}[/"📦 ${label}"/]`);
        nodeStyles.push(`style ${nodeId} fill:#8b5cf6,color:#fff`);
        pendingOps.set(event.eventId, { nodeId, name: shortName, type: 'sub-orch' });
        
        if (lastFlowNodeId) {
          lines.push(`  ${lastFlowNodeId} --> ${nodeId}`);
        }
      } else if (event.type === 'SubOrchestrationCompleted') {
        const source = pendingOps.get(event.sourceEventId!);
        if (source) {
          lines.push(`  ${nodeId}(["✓"])`);
          nodeStyles.push(`style ${nodeId} fill:#22c55e,color:#fff`);
          lines.push(`  ${source.nodeId} --> ${nodeId}`);
          pendingOps.delete(event.sourceEventId!);
          lastFlowNodeId = nodeId;
        }
      } else if (event.type === 'SubOrchestrationFailed') {
        const source = pendingOps.get(event.sourceEventId!);
        if (source) {
          lines.push(`  ${nodeId}(["✗"])`);
          nodeStyles.push(`style ${nodeId} fill:#ef4444,color:#fff`);
          lines.push(`  ${source.nodeId} --> ${nodeId}`);
          pendingOps.delete(event.sourceEventId!);
          lastFlowNodeId = nodeId;
        }
      } else if (event.type === 'ExternalSubscribed') {
        const label = shortName || 'Event';
        lines.push(`  ${nodeId}[/"⏳ Wait: ${label}"/]`);
        nodeStyles.push(`style ${nodeId} fill:#eab308,color:#000`);
        pendingOps.set(event.eventId, { nodeId, name: label, type: 'wait' });
        
        if (lastFlowNodeId) {
          lines.push(`  ${lastFlowNodeId} --> ${nodeId}`);
        }
      } else if (event.type === 'ExternalEvent') {
        const source = pendingOps.get(event.sourceEventId!);
        if (source) {
          lines.push(`  ${nodeId}(["📨"])`);
          nodeStyles.push(`style ${nodeId} fill:#22c55e,color:#fff`);
          lines.push(`  ${source.nodeId} --> ${nodeId}`);
          pendingOps.delete(event.sourceEventId!);
          lastFlowNodeId = nodeId;
        }
      } else if (event.type === 'OrchestrationCompleted') {
        lines.push(`  ${nodeId}(["🏁 Complete"])`);
        nodeStyles.push(`style ${nodeId} fill:#22c55e,color:#fff`);
        
        if (lastFlowNodeId) {
          lines.push(`  ${lastFlowNodeId} --> ${nodeId}`);
        }
        lastFlowNodeId = nodeId;
      } else if (event.type === 'OrchestrationFailed') {
        lines.push(`  ${nodeId}(["💥 Failed"])`);
        nodeStyles.push(`style ${nodeId} fill:#ef4444,color:#fff`);
        
        if (lastFlowNodeId) {
          lines.push(`  ${lastFlowNodeId} --> ${nodeId}`);
        }
        lastFlowNodeId = nodeId;
      } else if (event.type === 'OrchestrationContinuedAsNew') {
        lines.push(`  ${nodeId}(["🔄 Continue As New"])`);
        nodeStyles.push(`style ${nodeId} fill:#a855f7,color:#fff`);
        
        if (lastFlowNodeId) {
          lines.push(`  ${lastFlowNodeId} --> ${nodeId}`);
        }
        lastFlowNodeId = nodeId;
      } else if (event.type === 'SystemCall') {
        // Skip system calls in graph view for cleaner visualization
      }
    });
    
    // Add styles
    lines.push(...nodeStyles);
    
    return lines.join('\n');
  };

  // Apply execution state overlay to static flow diagram
  const applyExecutionStateToFlow = (
    staticMermaid: string,
    nodeMappings: Array<{ node_id: string; activity_pattern: string }>,
    history: { event: string }[]
  ): string => {
    // Parse history to determine completed activities
    const completedActivities = new Set<string>();
    const failedActivities = new Set<string>();
    const inProgressActivities = new Set<string>();
    
    const parsed = history.map(h => parseEvent(h.event));
    const pendingActivities = new Map<number, string>();
    
    parsed.forEach(event => {
      if (event.type === 'ActivityScheduled' && event.name) {
        pendingActivities.set(event.eventId, event.name);
        inProgressActivities.add(event.name);
      } else if (event.type === 'ActivityCompleted' && event.sourceEventId) {
        const activityName = pendingActivities.get(event.sourceEventId);
        if (activityName) {
          completedActivities.add(activityName);
          inProgressActivities.delete(activityName);
        }
      } else if (event.type === 'ActivityFailed' && event.sourceEventId) {
        const activityName = pendingActivities.get(event.sourceEventId);
        if (activityName) {
          failedActivities.add(activityName);
          inProgressActivities.delete(activityName);
        }
      }
    });
    
    // Build style overrides for nodes based on execution state
    const styleOverrides: string[] = [];
    
    nodeMappings.forEach(({ node_id, activity_pattern }) => {
      // Check if any completed activity matches this pattern
      const isCompleted = Array.from(completedActivities).some(a => a.includes(activity_pattern));
      const isFailed = Array.from(failedActivities).some(a => a.includes(activity_pattern));
      const isInProgress = Array.from(inProgressActivities).some(a => a.includes(activity_pattern));
      
      if (isFailed) {
        styleOverrides.push(`style ${node_id} fill:#ef4444,color:#fff,stroke:#991b1b,stroke-width:3px`);
      } else if (isCompleted) {
        styleOverrides.push(`style ${node_id} fill:#22c55e,color:#fff,stroke:#166534,stroke-width:2px`);
      } else if (isInProgress) {
        styleOverrides.push(`style ${node_id} fill:#3b82f6,color:#fff,stroke:#1d4ed8,stroke-width:3px,stroke-dasharray:5`);
      }
      // Pending nodes keep their default style
    });
    
    // Append style overrides to the mermaid diagram
    if (styleOverrides.length > 0) {
      return staticMermaid + '\n' + styleOverrides.join('\n');
    }
    return staticMermaid;
  };

  // Mermaid diagram component
  const MermaidDiagram = ({ chart, title }: { chart: string; title?: string }) => {
    const containerRef = useRef<HTMLDivElement>(null);
    
    useEffect(() => {
      const renderDiagram = async () => {
        if (containerRef.current && chart) {
          try {
            containerRef.current.innerHTML = '';
            const { svg } = await mermaid.render(`mermaid-${Date.now()}`, chart);
            containerRef.current.innerHTML = svg;
          } catch (err) {
            console.error('Mermaid render error:', err);
            containerRef.current.innerHTML = `<pre class="text-xs text-red-500 p-4">Failed to render diagram</pre>`;
          }
        }
      };
      renderDiagram();
    }, [chart]);
    
    return (
      <div className="space-y-2">
        {title && (
          <div className="flex items-center gap-4 text-xs text-muted-foreground">
            <span className="font-medium">{title}</span>
            <div className="flex items-center gap-3">
              <span className="flex items-center gap-1">
                <span className="w-3 h-3 rounded bg-green-500"></span> Completed
              </span>
              <span className="flex items-center gap-1">
                <span className="w-3 h-3 rounded bg-blue-500 border-2 border-dashed border-blue-700"></span> In Progress
              </span>
              <span className="flex items-center gap-1">
                <span className="w-3 h-3 rounded bg-red-500"></span> Failed
              </span>
              <span className="flex items-center gap-1">
                <span className="w-3 h-3 rounded bg-gray-300 dark:bg-gray-600"></span> Pending
              </span>
            </div>
          </div>
        )}
        <div 
          ref={containerRef} 
          className="bg-white dark:bg-gray-900 rounded-md border p-4 overflow-x-auto min-h-[200px]"
        />
      </div>
    );
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

  const OrchestrationTable = ({ 
    orchs, 
    page, 
    setPage 
  }: { 
    orchs: typeof orchestrations;
    page: number;
    setPage: (page: number) => void;
  }) => {
    const allOrchIds = orchs?.map(o => o.instance_id) || [];
    const totalItems = orchs?.length || 0;
    const totalPages = Math.ceil(totalItems / pageSize);
    
    // Paginate the data
    const startIdx = (page - 1) * pageSize;
    const paginatedOrchs = orchs?.slice(startIdx, startIdx + pageSize) || [];
    const paginatedIds = paginatedOrchs.map(o => o.instance_id);
    
    const allSelected = paginatedOrchs.every(o => selectedOrchs.has(o.instance_id)) && paginatedOrchs.length > 0;
    const someSelected = paginatedOrchs.some(o => selectedOrchs.has(o.instance_id));
    
    const toggleSelectAll = () => {
      if (allSelected) {
        // Deselect all on current page
        setSelectedOrchs(prev => {
          const next = new Set(prev);
          paginatedIds.forEach(id => next.delete(id));
          return next;
        });
      } else {
        // Select all on current page
        setSelectedOrchs(prev => {
          const next = new Set(prev);
          paginatedIds.forEach(id => next.add(id));
          return next;
        });
      }
    };
    
    return (
      <div>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b text-left text-sm text-muted-foreground">
                <th className="pb-3 font-medium w-8">
                  <input
                    type="checkbox"
                    checked={allSelected}
                    ref={(el) => { if (el) el.indeterminate = !allSelected && !!someSelected; }}
                    onChange={toggleSelectAll}
                    className="rounded border-gray-300"
                  />
                </th>
                <th className="pb-3 font-medium">ID</th>
                <th className="pb-3 font-medium">Type</th>
                <th className="pb-3 font-medium">Status</th>
                <th className="pb-3 font-medium">Exec</th>
                <th className="pb-3 font-medium">Started</th>
              </tr>
            </thead>
            <tbody>
              {paginatedOrchs.map((orch) => {
                const shortType = orch.orchestration_name.split('::').pop() || orch.orchestration_name;
                const isSelected = selectedOrchs.has(orch.instance_id);
                return (
                  <tr
                    key={orch.instance_id}
                    className={`border-b cursor-pointer transition-colors ${
                      isSelected 
                        ? 'bg-accent' 
                        : 'hover:bg-accent/50'
                    }`}
                    onClick={(e) => handleRowClick(orch.instance_id, e, allOrchIds)}
                  >
                    <td className="py-3">
                      <input
                        type="checkbox"
                        checked={isSelected}
                        onChange={() => {}}
                        onClick={(e) => e.stopPropagation()}
                        className="rounded border-gray-300"
                      />
                    </td>
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
        
        {/* Pagination Controls */}
        {totalPages > 1 && (
          <div className="flex items-center justify-between mt-4 pt-4 border-t">
            <div className="text-sm text-muted-foreground">
              Showing {startIdx + 1}-{Math.min(startIdx + pageSize, totalItems)} of {totalItems}
            </div>
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPage(1)}
                disabled={page === 1}
              >
                First
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPage(page - 1)}
                disabled={page === 1}
              >
                <ChevronLeft className="h-4 w-4" />
              </Button>
              <div className="flex items-center gap-1">
                {/* Page numbers */}
                {Array.from({ length: Math.min(5, totalPages) }, (_, i) => {
                  let pageNum: number;
                  if (totalPages <= 5) {
                    pageNum = i + 1;
                  } else if (page <= 3) {
                    pageNum = i + 1;
                  } else if (page >= totalPages - 2) {
                    pageNum = totalPages - 4 + i;
                  } else {
                    pageNum = page - 2 + i;
                  }
                  return (
                    <Button
                      key={pageNum}
                      size="sm"
                      variant={page === pageNum ? 'default' : 'outline'}
                      onClick={() => setPage(pageNum)}
                      className="w-8 h-8 p-0"
                    >
                      {pageNum}
                    </Button>
                  );
                })}
              </div>
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPage(page + 1)}
                disabled={page === totalPages}
              >
                <ChevronRight className="h-4 w-4" />
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPage(totalPages)}
                disabled={page === totalPages}
              >
                Last
              </Button>
            </div>
          </div>
        )}
      </div>
    );
  };

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

      {/* Bulk Action Bar */}
      {selectedOrchs.size > 0 && (
        <div className="flex items-center justify-between bg-accent/50 border rounded-md px-4 py-3">
          <div className="flex items-center gap-3">
            <span className="text-sm font-medium">
              {selectedOrchs.size} selected
            </span>
            <span className="text-xs text-muted-foreground">
              (Shift+click for range, Ctrl/⌘+click for multi-select)
            </span>
          </div>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={() => {
                setBulkActionType('recreate');
                setShowRecreateModal(true);
              }}
            >
              <RefreshCw className="h-4 w-4 mr-1" />
              Recreate ({selectedOrchs.size})
            </Button>
            <Button
              size="sm"
              variant="destructive"
              onClick={() => {
                setBulkActionType('cancel');
                setShowCancelModal(true);
              }}
            >
              <StopCircle className="h-4 w-4 mr-1" />
              Cancel ({selectedOrchs.size})
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={clearSelection}
            >
              <X className="h-4 w-4" />
            </Button>
          </div>
        </div>
      )}

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
            <OrchestrationTable orchs={instanceActors} page={actorsPage} setPage={setActorsPage} />
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
            <OrchestrationTable orchs={otherOrchestrations} page={operationsPage} setPage={setOperationsPage} />
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
                  {orchDetail.status === 'Running' && (
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => setShowRaiseEventModal(true)}
                    >
                      🔔 Raise Event
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={clearSelection}
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

            {orchDetail.history && orchDetail.history.length > 0 && (() => {
              const { parsed: parsedHistory, sourceMap } = buildEventCorrelation(orchDetail.history);
              const mermaidChart = generateMermaidDiagram(orchDetail.history);
              
              return (
                <div>
                  <div className="flex items-center justify-between mb-3">
                    <p className="text-sm font-medium">
                      Execution History ({orchDetail.history.length} events)
                    </p>
                    <div className="flex items-center gap-3">
                      {/* View toggle */}
                      <div className="flex items-center border rounded-md overflow-hidden">
                        <button
                          onClick={() => setHistoryView('table')}
                          className={`flex items-center gap-1 px-2 py-1 text-xs ${
                            historyView === 'table' 
                              ? 'bg-primary text-primary-foreground' 
                              : 'bg-background hover:bg-muted'
                          }`}
                        >
                          <TableIcon className="h-3 w-3" />
                          Table
                        </button>
                        <button
                          onClick={() => setHistoryView('graph')}
                          className={`flex items-center gap-1 px-2 py-1 text-xs ${
                            historyView === 'graph' 
                              ? 'bg-primary text-primary-foreground' 
                              : 'bg-background hover:bg-muted'
                          }`}
                        >
                          <GitBranch className="h-3 w-3" />
                          Graph
                        </button>
                      </div>
                      {/* History limit dropdown */}
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
                  </div>

                  {/* Graph View */}
                  {historyView === 'graph' && (
                    staticFlow ? (
                      <MermaidDiagram 
                        chart={applyExecutionStateToFlow(
                          staticFlow.mermaid, 
                          staticFlow.node_mappings, 
                          orchDetail.history
                        )}
                        title="Orchestration Flow (with execution state)"
                      />
                    ) : (
                      <MermaidDiagram 
                        chart={mermaidChart}
                        title="Execution Flow (dynamic)"
                      />
                    )
                  )}

                  {/* Table View */}
                  {historyView === 'table' && (
                    <div className="border rounded-md overflow-hidden">
                      <table className="w-full text-sm">
                        <thead className="bg-muted/50">
                          <tr className="border-b">
                            <th className="text-left p-2 font-medium w-12"></th>
                            <th className="text-left p-2 font-medium w-16">Event</th>
                            <th className="text-left p-2 font-medium w-16">Exec</th>
                            <th className="text-left p-2 font-medium w-40">Type</th>
                            <th className="text-left p-2 font-medium">Details</th>
                          </tr>
                        </thead>
                        <tbody>
                          {parsedHistory.map((parsed, idx) => {
                            const isExpanded = expandedEvents.has(idx);
                            const shortName = parsed.name?.split('::').pop() || parsed.name;
                            
                            // Determine if this is a source or completion event
                            const isSourceEvent = sourceMap.has(parsed.eventId);
                            const sourceInfo = parsed.sourceEventId ? sourceMap.get(parsed.sourceEventId) : undefined;
                            const isCompletionEvent = !!sourceInfo;
                            
                            // Get color for linked events
                            const colorClass = isSourceEvent 
                              ? linkColors[sourceMap.get(parsed.eventId)!.colorIndex]
                              : isCompletionEvent
                                ? linkColors[sourceInfo!.colorIndex]
                                : '';
                            
                            // Get source activity name for completion events
                            const sourceShortName = sourceInfo?.name?.split('::').pop() || sourceInfo?.name;
                            
                            return (
                              <tr 
                                key={idx} 
                                className={`border-b transition-colors ${
                                  isCompletionEvent ? 'border-l-4 ' + colorClass : 
                                  isSourceEvent ? 'border-l-4 ' + colorClass : 
                                  'hover:bg-muted/30'
                                }`}
                              >
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
                                  <span className={isCompletionEvent ? 'pl-4' : ''}>
                                    {parsed.type}
                                  </span>
                                </td>
                                <td className="p-2">
                                  {!isExpanded ? (
                                    <div className="text-xs truncate">
                                      {/* For source events, show the activity/timer name */}
                                      {isSourceEvent && shortName && (
                                        <span className="font-mono text-foreground">{shortName}</span>
                                      )}
                                      {/* For completion events, show source info */}
                                      {isCompletionEvent && (
                                        <span className="text-muted-foreground">
                                          <span className="opacity-60">←</span>
                                          <span className="font-mono ml-1">{sourceShortName || `event ${parsed.sourceEventId}`}</span>
                                          <span className="text-xs opacity-50 ml-2">(#{parsed.sourceEventId})</span>
                                          {parsed.durationMs !== undefined && (
                                            <span className="ml-2 text-xs opacity-70">
                                              {parsed.durationMs}ms
                                            </span>
                                          )}
                                        </span>
                                      )}
                                      {/* For other events */}
                                      {!isSourceEvent && !isCompletionEvent && shortName && (
                                        <span className="font-mono text-muted-foreground">{shortName}</span>
                                      )}
                                    </div>
                                  ) : (
                                    <pre className="text-xs whitespace-pre-wrap bg-muted/50 p-2 rounded mt-2 overflow-x-auto">
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
                  )}
                </div>
              );
            })()}
          </CardContent>
        </Card>

        {/* Cancel Confirmation Modal */}
        {showCancelModal && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
            <Card className="w-full max-w-md mx-4">
              <CardHeader>
                <CardTitle className="flex items-center space-x-2">
                  <AlertTriangle className="h-5 w-5 text-destructive" />
                  <span>Cancel {bulkActionType === 'cancel' && selectedOrchs.size > 1 ? `${selectedOrchs.size} Orchestrations` : 'Orchestration'}</span>
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                {bulkActionType === 'cancel' && selectedOrchs.size > 1 ? (
                  <>
                    <p className="text-sm">
                      Are you sure you want to cancel <strong>{selectedOrchs.size} orchestrations</strong>?
                    </p>
                    <div className="max-h-32 overflow-y-auto bg-muted/50 rounded-md p-2">
                      {Array.from(selectedOrchs).map(id => (
                        <div key={id} className="text-xs font-mono py-1">{id}</div>
                      ))}
                    </div>
                  </>
                ) : (
                  <p className="text-sm">
                    Are you sure you want to cancel <strong>{selectedOrch}</strong>?
                  </p>
                )}
                <div className="bg-destructive/10 border border-destructive/20 rounded-md p-3">
                  <p className="text-sm text-destructive font-medium">Warning:</p>
                  <ul className="text-sm text-destructive/80 mt-2 space-y-1 list-disc list-inside">
                    <li>The orchestration(s) will be stopped immediately</li>
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
                    onClick={() => { setShowCancelModal(false); setBulkActionType(null); }}
                    disabled={cancelMutation.isPending || bulkCancelMutation.isPending}
                  >
                    Close
                  </Button>
                  <Button
                    variant="destructive"
                    onClick={handleCancel}
                    disabled={cancelMutation.isPending || bulkCancelMutation.isPending}
                  >
                    {(cancelMutation.isPending || bulkCancelMutation.isPending) ? 'Cancelling...' : `Cancel ${bulkActionType === 'cancel' && selectedOrchs.size > 1 ? selectedOrchs.size : ''} Orchestration${bulkActionType === 'cancel' && selectedOrchs.size > 1 ? 's' : ''}`}
                  </Button>
                </div>
              </CardContent>
            </Card>
          </div>
        )}

        {/* Recreate Confirmation Modal */}
        {showRecreateModal && (bulkActionType === 'recreate' || orchDetail) && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
            <Card className="w-full max-w-md mx-4">
              <CardHeader>
                <CardTitle className="flex items-center space-x-2">
                  <RefreshCw className="h-5 w-5 text-primary" />
                  <span>Recreate {bulkActionType === 'recreate' && selectedOrchs.size > 1 ? `${selectedOrchs.size} Orchestrations` : 'Orchestration'}</span>
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                {bulkActionType === 'recreate' && selectedOrchs.size > 1 ? (
                  <>
                    <p className="text-sm">
                      Create new orchestrations with the same parameters for <strong>{selectedOrchs.size} selected items</strong>?
                    </p>
                    <div className="max-h-32 overflow-y-auto bg-muted/50 rounded-md p-2">
                      {Array.from(selectedOrchs).map(id => (
                        <div key={id} className="text-xs font-mono py-1">{id}</div>
                      ))}
                    </div>
                  </>
                ) : orchDetail && (
                  <>
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
                  </>
                )}
                <p className="text-xs text-muted-foreground">
                  New orchestrations will be created with new instance IDs but identical input parameters and version.
                </p>
                <div className="flex justify-end space-x-3 pt-2">
                  <Button
                    variant="outline"
                    onClick={() => { setShowRecreateModal(false); setBulkActionType(null); }}
                    disabled={recreateMutation.isPending || bulkRecreateMutation.isPending}
                  >
                    Cancel
                  </Button>
                  <Button
                    onClick={handleRecreate}
                    disabled={recreateMutation.isPending || bulkRecreateMutation.isPending}
                  >
                    {(recreateMutation.isPending || bulkRecreateMutation.isPending) ? 'Recreating...' : `Recreate ${bulkActionType === 'recreate' && selectedOrchs.size > 1 ? selectedOrchs.size : ''}`}
                  </Button>
                </div>
              </CardContent>
            </Card>
          </div>
        )}

        {/* Raise Event Modal */}
        {showRaiseEventModal && orchDetail && (
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <Card className="w-full max-w-lg mx-4">
              <CardHeader>
                <div className="flex items-center justify-between">
                  <CardTitle>Raise External Event</CardTitle>
                  <button
                    onClick={() => setShowRaiseEventModal(false)}
                    className="text-muted-foreground hover:text-foreground"
                  >
                    <X className="h-5 w-5" />
                  </button>
                </div>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="bg-muted/50 border rounded-md p-3 space-y-2">
                  <div className="flex justify-between text-sm">
                    <span className="text-muted-foreground">Target Instance:</span>
                    <code className="text-xs">{selectedOrch}</code>
                  </div>
                  <div className="flex justify-between text-sm">
                    <span className="text-muted-foreground">Status:</span>
                    <span className="text-xs">{orchDetail.status}</span>
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium mb-1">
                    Event Name
                  </label>
                  <input
                    type="text"
                    value={eventName}
                    onChange={(e) => setEventName(e.target.value)}
                    placeholder="e.g., InstanceDeleted"
                    className="w-full border rounded-md px-3 py-2 text-sm"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium mb-1">
                    Event Data (JSON)
                  </label>
                  <textarea
                    value={eventData}
                    onChange={(e) => setEventData(e.target.value)}
                    placeholder='{"key": "value"}'
                    rows={4}
                    className="w-full border rounded-md px-3 py-2 text-sm font-mono"
                  />
                </div>
                <p className="text-xs text-muted-foreground">
                  ⚠️ Debug feature: This will raise an external event to the orchestration. 
                  The orchestration must be waiting for this event via schedule_wait().
                </p>
                <div className="flex justify-end space-x-3 pt-2">
                  <Button
                    variant="outline"
                    onClick={() => setShowRaiseEventModal(false)}
                    disabled={raiseEventMutation.isPending}
                  >
                    Cancel
                  </Button>
                  <Button
                    onClick={handleRaiseEvent}
                    disabled={raiseEventMutation.isPending}
                  >
                    {raiseEventMutation.isPending ? 'Raising...' : 'Raise Event'}
                  </Button>
                </div>
              </CardContent>
            </Card>
          </div>
        )}
        </>
      )}

      {/* Bulk Cancel Modal - outside detail view conditional */}
      {showCancelModal && bulkActionType === 'cancel' && selectedOrchs.size > 1 && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <Card className="w-full max-w-md mx-4">
            <CardHeader>
              <CardTitle className="flex items-center space-x-2">
                <AlertTriangle className="h-5 w-5 text-destructive" />
                <span>Cancel {selectedOrchs.size} Orchestrations</span>
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm">
                Are you sure you want to cancel <strong>{selectedOrchs.size} orchestrations</strong>?
              </p>
              <div className="max-h-32 overflow-y-auto bg-muted/50 rounded-md p-2">
                {Array.from(selectedOrchs).map(id => (
                  <div key={id} className="text-xs font-mono py-1">{id}</div>
                ))}
              </div>
              <div className="bg-destructive/10 border border-destructive/20 rounded-md p-3">
                <p className="text-sm text-destructive font-medium">Warning:</p>
                <ul className="text-sm text-destructive/80 mt-2 space-y-1 list-disc list-inside">
                  <li>The orchestrations will be stopped immediately</li>
                  <li>Any in-progress activities may be left incomplete</li>
                  <li>This action cannot be undone</li>
                </ul>
              </div>
              <div className="flex justify-end space-x-3 pt-2">
                <Button
                  variant="outline"
                  onClick={() => { setShowCancelModal(false); setBulkActionType(null); }}
                  disabled={bulkCancelMutation.isPending}
                >
                  Close
                </Button>
                <Button
                  variant="destructive"
                  onClick={handleCancel}
                  disabled={bulkCancelMutation.isPending}
                >
                  {bulkCancelMutation.isPending ? 'Cancelling...' : `Cancel ${selectedOrchs.size} Orchestrations`}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Bulk Recreate Modal - outside detail view conditional */}
      {showRecreateModal && bulkActionType === 'recreate' && selectedOrchs.size > 1 && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <Card className="w-full max-w-md mx-4">
            <CardHeader>
              <CardTitle className="flex items-center space-x-2">
                <RefreshCw className="h-5 w-5 text-primary" />
                <span>Recreate {selectedOrchs.size} Orchestrations</span>
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm">
                Create new orchestrations with the same parameters for <strong>{selectedOrchs.size} selected items</strong>?
              </p>
              <div className="max-h-32 overflow-y-auto bg-muted/50 rounded-md p-2">
                {Array.from(selectedOrchs).map(id => (
                  <div key={id} className="text-xs font-mono py-1">{id}</div>
                ))}
              </div>
              <p className="text-xs text-muted-foreground">
                New orchestrations will be created with new instance IDs but identical input parameters and version.
              </p>
              <div className="flex justify-end space-x-3 pt-2">
                <Button
                  variant="outline"
                  onClick={() => { setShowRecreateModal(false); setBulkActionType(null); }}
                  disabled={bulkRecreateMutation.isPending}
                >
                  Cancel
                </Button>
                <Button
                  onClick={handleRecreate}
                  disabled={bulkRecreateMutation.isPending}
                >
                  {bulkRecreateMutation.isPending ? 'Recreating...' : `Recreate ${selectedOrchs.size}`}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}

