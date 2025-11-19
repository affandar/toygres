import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft, Copy, Trash2, AlertTriangle } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useToast } from '@/lib/toast';
import { api } from '@/lib/api';
import { copyToClipboard, getStateColor, getHealthColor, formatRelativeTime } from '@/lib/utils';

export function InstanceDetail() {
  const { name } = useParams<{ name: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { showToast } = useToast();
  
  const [showDeleteModal, setShowDeleteModal] = useState(false);

  const { data: instance, isLoading } = useQuery({
    queryKey: ['instance', name],
    queryFn: () => api.getInstance(name!),
    enabled: !!name,
    refetchInterval: 5000,
  });

  const deleteMutation = useMutation({
    mutationFn: (instanceName: string) => api.deleteInstance(instanceName),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['instances'] });
      showToast('success', `Instance '${data.instance_name}' deletion started!`);
      navigate('/instances');
    },
    onError: (error: Error) => {
      showToast('error', `Failed to delete instance: ${error.message}`);
    },
  });

  const handleDelete = () => {
    if (name) {
      deleteMutation.mutate(name);
      setShowDeleteModal(false);
    }
  };

  if (isLoading) {
    return <div>Loading...</div>;
  }

  if (!instance) {
    return <div>Instance not found</div>;
  }

  const connectionString = instance.dns_connection_string || instance.ip_connection_string;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center space-x-4">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => navigate('/instances')}
          >
            <ArrowLeft className="h-4 w-4 mr-2" />
            Back
          </Button>
          <div>
            <h1 className="text-3xl font-bold tracking-tight">{instance.user_name}</h1>
            <p className="text-sm text-muted-foreground">K8s: {instance.k8s_name}</p>
          </div>
        </div>
        <Button 
          variant="destructive"
          onClick={() => setShowDeleteModal(true)}
          disabled={instance.state === 'deleting' || instance.state === 'deleted'}
        >
          <Trash2 className="h-4 w-4 mr-2" />
          Delete Instance
        </Button>
      </div>

      {/* Delete Confirmation Modal */}
      {showDeleteModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <Card className="w-full max-w-md mx-4">
            <CardHeader>
              <CardTitle className="flex items-center space-x-2">
                <AlertTriangle className="h-5 w-5 text-destructive" />
                <span>Delete Instance</span>
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm">
                Are you sure you want to delete <strong>{instance.user_name}</strong>?
              </p>
              <div className="bg-destructive/10 border border-destructive/20 rounded-md p-3">
                <p className="text-sm text-destructive font-medium">Warning:</p>
                <ul className="text-sm text-destructive/80 mt-2 space-y-1 list-disc list-inside">
                  <li>All data will be permanently deleted</li>
                  <li>The instance cannot be recovered</li>
                  <li>Connection strings will stop working</li>
                </ul>
              </div>
              <div className="flex justify-end space-x-3 pt-2">
                <Button
                  variant="outline"
                  onClick={() => setShowDeleteModal(false)}
                  disabled={deleteMutation.isPending}
                >
                  Cancel
                </Button>
                <Button
                  variant="destructive"
                  onClick={handleDelete}
                  disabled={deleteMutation.isPending}
                >
                  {deleteMutation.isPending ? 'Deleting...' : 'Delete Instance'}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Status</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">State</span>
              <span className={`text-sm font-medium ${getStateColor(instance.state)}`}>
                {instance.state}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Health</span>
              <span className={`text-sm font-medium ${getHealthColor(instance.health_status)}`}>
                {instance.health_status}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Version</span>
              <span className="text-sm font-medium">{instance.postgres_version}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Storage</span>
              <span className="text-sm font-medium">{instance.storage_size_gb} GB</span>
            </div>
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Created</span>
              <span className="text-sm font-medium">{formatRelativeTime(instance.created_at)}</span>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Connection</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {instance.dns_name && (
              <div>
                <span className="text-sm text-muted-foreground">DNS</span>
                <p className="text-sm font-mono mt-1">{instance.dns_name}</p>
              </div>
            )}
            {instance.external_ip && (
              <div>
                <span className="text-sm text-muted-foreground">IP Address</span>
                <p className="text-sm font-mono mt-1">{instance.external_ip}</p>
              </div>
            )}
            {connectionString && (
              <div>
                <span className="text-sm text-muted-foreground">Connection String</span>
                <div className="flex items-center space-x-2 mt-1">
                  <code className="flex-1 rounded bg-muted px-2 py-1 text-xs break-all">
                    {connectionString}
                  </code>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => {
                      copyToClipboard(connectionString);
                      showToast('success', 'Connection string copied to clipboard');
                    }}
                  >
                    <Copy className="h-3 w-3" />
                  </Button>
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {(instance.create_orchestration_id || instance.instance_actor_orchestration_id) && (
        <Card>
          <CardHeader>
            <CardTitle>Related Orchestrations</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            {instance.create_orchestration_id && (
              <div 
                className="flex items-center justify-between p-3 rounded-md hover:bg-accent cursor-pointer"
                onClick={() => navigate(`/debug/orchestrations/${instance.create_orchestration_id}`)}
              >
                <div>
                  <p className="text-sm font-medium">Create Instance</p>
                  <p className="text-xs text-muted-foreground">{instance.create_orchestration_id}</p>
                </div>
                <span className="text-xs text-green-600">Completed</span>
              </div>
            )}
            {instance.instance_actor_orchestration_id && (
              <div 
                className="flex items-center justify-between p-3 rounded-md hover:bg-accent cursor-pointer"
                onClick={() => navigate(`/debug/orchestrations/${instance.instance_actor_orchestration_id}`)}
              >
                <div>
                  <p className="text-sm font-medium">Instance Actor</p>
                  <p className="text-xs text-muted-foreground">{instance.instance_actor_orchestration_id}</p>
                </div>
                <span className="text-xs text-blue-600">Running</span>
              </div>
            )}
          </CardContent>
        </Card>
      )}
    </div>
  );
}

