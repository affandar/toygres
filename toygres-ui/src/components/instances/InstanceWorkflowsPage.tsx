import { useParams, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/api';
import { PgDurableOrchestrations } from './PgDurableOrchestrations';

export function InstanceWorkflowsPage() {
  const { name } = useParams<{ name: string }>();
  const navigate = useNavigate();

  const { data: instance, isLoading } = useQuery({
    queryKey: ['instance', name],
    queryFn: () => api.getInstance(name!),
    enabled: !!name,
  });

  if (!name) {
    return <div>Instance name required</div>;
  }

  if (isLoading) {
    return <div className="flex items-center justify-center h-64 text-muted-foreground">Loading...</div>;
  }

  if (!instance) {
    return <div>Instance not found</div>;
  }

  if (instance.image_type !== 'pg_durable') {
    return (
      <div className="space-y-6">
        <div className="flex items-center space-x-4">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => navigate(`/instances/${name}`)}
          >
            <ArrowLeft className="h-4 w-4 mr-2" />
            Back to Instance
          </Button>
        </div>
        <div className="flex items-center justify-center h-64">
          <div className="text-center">
            <p className="text-lg font-medium">Workflows not available</p>
            <p className="text-sm text-muted-foreground mt-2">
              Durable SQL workflows are only available for pg_durable instances.
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center space-x-4">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => navigate(`/instances/${name}`)}
        >
          <ArrowLeft className="h-4 w-4 mr-2" />
          Back to Instance
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Durable SQL Functions</h1>
          <p className="text-sm text-muted-foreground">
            Instance: {name} · Manage, monitor, and create durable functions
          </p>
        </div>
      </div>

      <PgDurableOrchestrations instanceName={name} />
    </div>
  );
}

