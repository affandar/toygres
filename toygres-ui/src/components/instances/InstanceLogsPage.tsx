import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { InstanceLogs } from './InstanceLogs';

export function InstanceLogsPage() {
  const { name } = useParams<{ name: string }>();
  const navigate = useNavigate();

  if (!name) {
    return <div>Instance name required</div>;
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
          <h1 className="text-3xl font-bold tracking-tight">PostgreSQL Logs</h1>
          <p className="text-sm text-muted-foreground">Instance: {name}</p>
        </div>
      </div>

      <InstanceLogs instanceName={name} />
    </div>
  );
}

