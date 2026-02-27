import { useParams, Link } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../../lib/api';
import { CheckCircle2, XCircle, Loader2, ArrowLeft, AlertTriangle, StopCircle } from 'lucide-react';
import { useState } from 'react';

export function BatchStatus() {
  const { batchId } = useParams<{ batchId: string }>();
  const queryClient = useQueryClient();
  const [showCancelConfirm, setShowCancelConfirm] = useState(false);

  const { data, isLoading, error } = useQuery({
    queryKey: ['batch', batchId],
    queryFn: () => api.getBatchStatus(batchId!),
    refetchInterval: (query) => {
      const status = query.state.data?.status;
      if (status === 'Completed' || status === 'Failed') return false;
      return 3000;
    },
    enabled: !!batchId,
  });

  const cancelMutation = useMutation({
    mutationFn: () => api.cancelOrchestration(batchId!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['batch', batchId] });
      setShowCancelConfirm(false);
    },
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <Loader2 className="h-8 w-8 animate-spin text-blue-500" />
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="p-6">
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4 text-red-400">
          Failed to load batch status: {(error as Error)?.message || 'Unknown error'}
        </div>
      </div>
    );
  }

  const { progress, status } = data;
  const total = progress?.total || 0;
  const completed = progress?.completed || 0;
  const failed = progress?.failed || 0;
  const creating = progress?.creating || 0;
  const errors = progress?.errors || [];
  const pct = total > 0 ? Math.round(((completed + failed) / total) * 100) : 0;
  const isTerminal = status === 'Completed' || status === 'Failed';

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6">
      {/* Header */}
      <div className="flex items-center gap-3">
        <Link to="/batches" className="text-zinc-400 hover:text-zinc-200">
          <ArrowLeft className="h-5 w-5" />
        </Link>
        <h1 className="text-xl font-semibold text-zinc-100">Batch Create</h1>
        <code className="text-xs text-zinc-500 bg-zinc-800 px-2 py-1 rounded">{batchId}</code>
        {!isTerminal && <Loader2 className="h-4 w-4 animate-spin text-blue-400 ml-2" />}
        {!isTerminal && (
          <div className="ml-auto">
            {showCancelConfirm ? (
              <div className="flex items-center gap-2">
                <span className="text-xs text-zinc-400">Cancel this batch?</span>
                <button
                  onClick={() => cancelMutation.mutate()}
                  disabled={cancelMutation.isPending}
                  className="px-2 py-1 text-xs bg-red-600 hover:bg-red-500 text-white rounded disabled:opacity-50"
                >
                  {cancelMutation.isPending ? 'Cancelling...' : 'Yes, cancel'}
                </button>
                <button
                  onClick={() => setShowCancelConfirm(false)}
                  className="px-2 py-1 text-xs bg-zinc-700 hover:bg-zinc-600 text-zinc-300 rounded"
                >
                  No
                </button>
              </div>
            ) : (
              <button
                onClick={() => setShowCancelConfirm(true)}
                className="flex items-center gap-1 px-3 py-1.5 text-sm bg-zinc-700 hover:bg-red-600 text-zinc-300 hover:text-white rounded-md transition-colors"
              >
                <StopCircle className="h-3.5 w-3.5" />
                Cancel Batch
              </button>
            )}
          </div>
        )}
      </div>

      {/* Progress bar */}
      <div className="bg-zinc-800 rounded-lg p-5 border border-zinc-700">
        <div className="flex justify-between items-center mb-3">
          <span className="text-sm text-zinc-400">Progress</span>
          <span className="text-sm font-mono text-zinc-300">{completed + failed} / {total} ({pct}%)</span>
        </div>
        <div className="w-full bg-zinc-700 rounded-full h-3 overflow-hidden">
          <div className="h-full flex">
            <div
              className="bg-emerald-500 transition-all duration-500"
              style={{ width: total > 0 ? `${(completed / total) * 100}%` : '0%' }}
            />
            <div
              className="bg-red-500 transition-all duration-500"
              style={{ width: total > 0 ? `${(failed / total) * 100}%` : '0%' }}
            />
          </div>
        </div>

        {/* Counts */}
        <div className="flex gap-6 mt-4">
          <div className="flex items-center gap-2">
            <CheckCircle2 className="h-4 w-4 text-emerald-400" />
            <span className="text-sm text-zinc-300">{completed} completed</span>
          </div>
          <div className="flex items-center gap-2">
            <XCircle className="h-4 w-4 text-red-400" />
            <span className="text-sm text-zinc-300">{failed} failed</span>
          </div>
          <div className="flex items-center gap-2">
            <Loader2 className={`h-4 w-4 text-blue-400 ${creating > 0 ? 'animate-spin' : ''}`} />
            <span className="text-sm text-zinc-300">{creating} creating</span>
          </div>
        </div>

        {/* Terminal status */}
        {isTerminal && (
          <div className={`mt-4 px-3 py-2 rounded text-sm ${
            status === 'Completed' && failed === 0
              ? 'bg-emerald-500/10 border border-emerald-500/30 text-emerald-400'
              : 'bg-amber-500/10 border border-amber-500/30 text-amber-400'
          }`}>
            {status === 'Completed' && failed === 0
              ? `✓ All ${total} instances created successfully`
              : `Batch complete: ${completed} succeeded, ${failed} failed`}
          </div>
        )}
      </div>

      {/* Errors table */}
      {errors.length > 0 && (
        <div className="bg-zinc-800 rounded-lg border border-zinc-700">
          <div className="flex items-center gap-2 px-4 py-3 border-b border-zinc-700">
            <AlertTriangle className="h-4 w-4 text-red-400" />
            <h2 className="text-sm font-medium text-zinc-200">Errors ({errors.length})</h2>
          </div>
          <div className="divide-y divide-zinc-700/50 max-h-96 overflow-y-auto">
            {errors.map((err, i) => (
              <div key={i} className="px-4 py-3">
                <div className="text-sm font-mono text-zinc-300">{err.instance}</div>
                <div className="text-xs text-red-400 mt-1 break-all">{err.error}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Link back */}
      <div className="text-center">
        <Link
          to="/instances"
          className="text-sm text-blue-400 hover:text-blue-300 underline"
        >
          View all instances →
        </Link>
      </div>
    </div>
  );
}
