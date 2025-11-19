import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { Database, Plus, Trash2, X } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/api';
import { useToast } from '@/lib/toast';
import { getStateColor, getHealthColor } from '@/lib/utils';

export function InstanceList() {
  const navigate = useNavigate();
  const [selectedInstances, setSelectedInstances] = useState<Set<string>>(new Set());
  const [showBulkDeleteModal, setShowBulkDeleteModal] = useState(false);
  const queryClient = useQueryClient();
  const { showToast } = useToast();
  
  const { data: instances, isLoading } = useQuery({
    queryKey: ['instances'],
    queryFn: () => api.listInstances(),
    refetchInterval: 5000,
  });

  const bulkDeleteMutation = useMutation({
    mutationFn: (names: string[]) => api.bulkDeleteInstances(names),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['instances'] });
      showToast('success', `Deleted ${data.deleted} instances${data.errors > 0 ? ` (${data.errors} errors)` : ''}`);
      setSelectedInstances(new Set());
      setShowBulkDeleteModal(false);
    },
    onError: (error: Error) => {
      showToast('error', `Failed to delete: ${error.message}`);
    },
  });

  const toggleInstance = (name: string, event: React.MouseEvent) => {
    event.stopPropagation();
    const newSelected = new Set(selectedInstances);
    if (newSelected.has(name)) {
      newSelected.delete(name);
    } else {
      newSelected.add(name);
    }
    setSelectedInstances(newSelected);
  };

  const toggleAll = (event: React.ChangeEvent<HTMLInputElement>) => {
    if (event.target.checked && instances) {
      setSelectedInstances(new Set(instances.map(i => i.user_name)));
    } else {
      setSelectedInstances(new Set());
    }
  };

  const handleBulkDelete = () => {
    bulkDeleteMutation.mutate(Array.from(selectedInstances));
  };

  if (isLoading) {
    return <div>Loading...</div>;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">PostgreSQL Instances</h1>
          <p className="text-muted-foreground">
            Manage your PostgreSQL database instances
          </p>
        </div>
        <div className="flex gap-2">
          {selectedInstances.size > 0 && (
            <Button
              variant="destructive"
              onClick={() => setShowBulkDeleteModal(true)}
            >
              <Trash2 className="mr-2 h-4 w-4" />
              Delete Selected ({selectedInstances.size})
            </Button>
          )}
          <Button onClick={() => navigate('/instances/create')}>
            <Plus className="mr-2 h-4 w-4" />
            Create New
          </Button>
          <Button onClick={() => navigate('/instances/bulk-create')}>
            <Plus className="mr-2 h-4 w-4" />
            Bulk Create
          </Button>
        </div>
      </div>

      {!instances || instances.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Database className="h-12 w-12 text-muted-foreground mb-4" />
            <h3 className="text-lg font-semibold mb-2">No instances found</h3>
            <p className="text-sm text-muted-foreground mb-4">
              Get started by creating your first PostgreSQL instance
            </p>
            <Button onClick={() => navigate('/instances/create')}>
              <Plus className="mr-2 h-4 w-4" />
              Create Instance
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle>All Instances ({instances.length})</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b text-left text-sm text-muted-foreground">
                    <th className="pb-3 font-medium w-12">
                      <input
                        type="checkbox"
                        checked={instances.length > 0 && selectedInstances.size === instances.length}
                        onChange={toggleAll}
                        className="cursor-pointer"
                      />
                    </th>
                    <th className="pb-3 font-medium">Name</th>
                    <th className="pb-3 font-medium">Status</th>
                    <th className="pb-3 font-medium">Health</th>
                    <th className="pb-3 font-medium">Version</th>
                    <th className="pb-3 font-medium">Storage</th>
                    <th className="pb-3 font-medium">DNS</th>
                  </tr>
                </thead>
                <tbody>
                  {instances.map((instance) => (
                    <tr
                      key={instance.k8s_name}
                      className="border-b hover:bg-accent/50 transition-colors"
                    >
                      <td className="py-3">
                        <input
                          type="checkbox"
                          checked={selectedInstances.has(instance.user_name)}
                          onChange={(e) => toggleInstance(instance.user_name, e as unknown as React.MouseEvent)}
                          onClick={(e) => e.stopPropagation()}
                          className="cursor-pointer"
                        />
                      </td>
                      <td
                        className="py-3 font-medium cursor-pointer"
                        onClick={() => navigate(`/instances/${instance.user_name}`)}
                      >
                        {instance.user_name}
                      </td>
                      <td
                        className="py-3 cursor-pointer"
                        onClick={() => navigate(`/instances/${instance.user_name}`)}
                      >
                        <span className={getStateColor(instance.state)}>
                          ● {instance.state}
                        </span>
                      </td>
                      <td
                        className="py-3 cursor-pointer"
                        onClick={() => navigate(`/instances/${instance.user_name}`)}
                      >
                        <span className={getHealthColor(instance.health_status)}>
                          {instance.health_status === 'healthy' && '✓'}
                          {instance.health_status === 'unhealthy' && '✗'}
                          {instance.health_status === 'unknown' && '○'}
                          {' '}
                          {instance.health_status}
                        </span>
                      </td>
                      <td
                        className="py-3 text-muted-foreground cursor-pointer"
                        onClick={() => navigate(`/instances/${instance.user_name}`)}
                      >
                        {instance.postgres_version}
                      </td>
                      <td
                        className="py-3 text-muted-foreground cursor-pointer"
                        onClick={() => navigate(`/instances/${instance.user_name}`)}
                      >
                        {instance.storage_size_gb} GB
                      </td>
                      <td
                        className="py-3 text-muted-foreground text-sm cursor-pointer"
                        onClick={() => navigate(`/instances/${instance.user_name}`)}
                      >
                        {instance.dns_name || '-'}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Bulk Delete Confirmation Modal */}
      {showBulkDeleteModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-background border rounded-lg p-6 max-w-md w-full mx-4">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold">Confirm Bulk Deletion</h3>
              <button
                onClick={() => setShowBulkDeleteModal(false)}
                className="text-muted-foreground hover:text-foreground"
              >
                <X className="h-5 w-5" />
              </button>
            </div>
            <p className="text-sm text-muted-foreground mb-4">
              Are you sure you want to delete {selectedInstances.size} instance{selectedInstances.size !== 1 ? 's' : ''}?
            </p>
            <div className="max-h-48 overflow-y-auto mb-4 p-3 bg-muted/30 rounded text-sm">
              {Array.from(selectedInstances).map((name) => (
                <div key={name} className="py-1">• {name}</div>
              ))}
            </div>
            <div className="flex gap-2 justify-end">
              <Button
                variant="outline"
                onClick={() => setShowBulkDeleteModal(false)}
              >
                Cancel
              </Button>
              <Button
                variant="destructive"
                onClick={handleBulkDelete}
                disabled={bulkDeleteMutation.isPending}
              >
                {bulkDeleteMutation.isPending ? 'Deleting...' : 'Delete All'}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

