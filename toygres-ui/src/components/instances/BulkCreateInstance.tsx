import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft, Plus } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/api';
import { useToast } from '@/lib/toast';

export function BulkCreateInstance() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { showToast } = useToast();

  const [baseName, setBaseName] = useState('');
  const [count, setCount] = useState(3);
  const [password, setPassword] = useState('');
  const [postgresVersion, setPostgresVersion] = useState('18');
  const [storageSize, setStorageSize] = useState(10);
  const [internal, setInternal] = useState(false);

  const createMutation = useMutation({
    mutationFn: (data: {
      base_name: string;
      count: number;
      password: string;
      postgres_version: string;
      storage_size_gb: number;
      internal: boolean;
      namespace: string;
    }) => api.bulkCreateInstances(data),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['instances'] });
      showToast('success', `Created ${data.count} instances successfully`);
      navigate('/instances');
    },
    onError: (error: Error) => {
      showToast('error', `Failed to create instances: ${error.message}`);
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    if (!baseName.trim()) {
      showToast('error', 'Please enter a base name');
      return;
    }

    if (count < 1 || count > 50) {
      showToast('error', 'Count must be between 1 and 50');
      return;
    }

    if (password.length < 8) {
      showToast('error', 'Password must be at least 8 characters');
      return;
    }

    createMutation.mutate({
      base_name: baseName,
      count,
      password,
      postgres_version: postgresVersion,
      storage_size_gb: storageSize,
      internal,
      namespace: 'toygres',
    });
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="outline" onClick={() => navigate('/instances')}>
          <ArrowLeft className="mr-2 h-4 w-4" />
          Back
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Bulk Create Instances</h1>
          <p className="text-muted-foreground">
            Create multiple instances with the same configuration
          </p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Bulk Creation Settings</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="block text-sm font-medium mb-1">
                Base Name
              </label>
              <input
                type="text"
                value={baseName}
                onChange={(e) => setBaseName(e.target.value)}
                placeholder="e.g., 'db' will create db1, db2, db3..."
                className="w-full border rounded-md px-3 py-2"
                required
              />
              <p className="text-xs text-muted-foreground mt-1">
                Instances will be named: {baseName}1, {baseName}2, {baseName}3, etc.
              </p>
            </div>

            <div>
              <label className="block text-sm font-medium mb-1">
                Count
              </label>
              <input
                type="number"
                value={count}
                onChange={(e) => setCount(parseInt(e.target.value) || 1)}
                min="1"
                max="50"
                className="w-full border rounded-md px-3 py-2"
                required
              />
              <p className="text-xs text-muted-foreground mt-1">
                Number of instances to create (1-50)
              </p>
            </div>

            <div>
              <label className="block text-sm font-medium mb-1">
                Password (shared for all instances)
              </label>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="w-full border rounded-md px-3 py-2"
                required
                minLength={8}
              />
              <p className="text-xs text-muted-foreground mt-1">
                Minimum 8 characters
              </p>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium mb-1">
                  PostgreSQL Version
                </label>
                <select
                  value={postgresVersion}
                  onChange={(e) => setPostgresVersion(e.target.value)}
                  className="w-full border rounded-md px-3 py-2"
                >
                  <option value="18">PostgreSQL 18</option>
                  <option value="17">PostgreSQL 17</option>
                  <option value="16">PostgreSQL 16</option>
                  <option value="15">PostgreSQL 15</option>
                </select>
              </div>

              <div>
                <label className="block text-sm font-medium mb-1">
                  Storage Size (GB)
                </label>
                <input
                  type="number"
                  value={storageSize}
                  onChange={(e) => setStorageSize(parseInt(e.target.value) || 10)}
                  min="10"
                  max="1000"
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
            </div>

            <div className="flex items-center gap-2">
              <input
                type="checkbox"
                id="internal"
                checked={internal}
                onChange={(e) => setInternal(e.target.checked)}
                className="cursor-pointer"
              />
              <label htmlFor="internal" className="text-sm cursor-pointer">
                Internal only (no LoadBalancer)
              </label>
            </div>

            <div className="flex gap-3 pt-4">
              <Button
                type="button"
                variant="outline"
                onClick={() => navigate('/instances')}
                className="flex-1"
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={createMutation.isPending}
                className="flex-1"
              >
                <Plus className="mr-2 h-4 w-4" />
                {createMutation.isPending ? `Creating ${count} instances...` : `Create ${count} Instances`}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}


