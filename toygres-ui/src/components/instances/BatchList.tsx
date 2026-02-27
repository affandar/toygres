import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { api } from '../../lib/api';
import { Loader2, CheckCircle2, XCircle, Clock } from 'lucide-react';

export function BatchList() {
  const { data, isLoading, error } = useQuery({
    queryKey: ['batches'],
    queryFn: () => api.listBatches(),
    refetchInterval: 10000,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <Loader2 className="h-8 w-8 animate-spin text-blue-500" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6">
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4 text-red-400">
          Failed to load batches: {(error as Error)?.message}
        </div>
      </div>
    );
  }

  const batches = data?.batches || [];

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold text-zinc-100">Batch Creates</h1>
        <Link
          to="/instances/bulk-create"
          className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 text-white text-sm rounded-md transition-colors"
        >
          New Batch
        </Link>
      </div>

      {batches.length === 0 ? (
        <div className="bg-zinc-800 rounded-lg border border-zinc-700 p-8 text-center text-zinc-400">
          No batch creates yet
        </div>
      ) : (
        <div className="space-y-3">
          {batches.map((batch) => {
            const p = batch.progress;
            const total = p?.total || 0;
            const completed = p?.completed || 0;
            const failed = p?.failed || 0;
            const creating = p?.creating || 0;
            const done = completed + failed;
            const pct = total > 0 ? Math.round((done / total) * 100) : 0;

            return (
              <Link
                key={batch.batch_id}
                to={`/batches/${batch.batch_id}`}
                className="block bg-zinc-800 rounded-lg border border-zinc-700 p-4 hover:border-zinc-500 transition-colors"
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <StatusIcon status={batch.status} />
                    <code className="text-sm text-zinc-200">{batch.batch_id}</code>
                  </div>
                  <span className="text-xs text-zinc-500">
                    {new Date(batch.created_at).toLocaleString()}
                  </span>
                </div>

                {p && total > 0 && (
                  <>
                    <div className="w-full bg-zinc-700 rounded-full h-2 overflow-hidden mb-2">
                      <div className="h-full flex">
                        <div
                          className="bg-emerald-500 transition-all"
                          style={{ width: `${(completed / total) * 100}%` }}
                        />
                        <div
                          className="bg-red-500 transition-all"
                          style={{ width: `${(failed / total) * 100}%` }}
                        />
                      </div>
                    </div>
                    <div className="flex gap-4 text-xs text-zinc-400">
                      <span>{completed} ✓</span>
                      <span>{failed} ✗</span>
                      <span>{creating} creating</span>
                      <span className="ml-auto">{done}/{total} ({pct}%)</span>
                    </div>
                  </>
                )}
              </Link>
            );
          })}
        </div>
      )}
    </div>
  );
}

function StatusIcon({ status }: { status: string }) {
  switch (status) {
    case 'Running':
      return <Loader2 className="h-4 w-4 animate-spin text-blue-400" />;
    case 'Completed':
      return <CheckCircle2 className="h-4 w-4 text-emerald-400" />;
    case 'Failed':
      return <XCircle className="h-4 w-4 text-red-400" />;
    default:
      return <Clock className="h-4 w-4 text-zinc-400" />;
  }
}
