import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useToast } from '@/lib/toast';
import { api } from '@/lib/api';

export function CreateInstance() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { showToast } = useToast();

  const [formData, setFormData] = useState({
    name: '',
    password: '',
    postgres_version: '18',
    storage_size_gb: 10,
    internal: false,
  });

  const [errors, setErrors] = useState<Record<string, string>>({});

  const createMutation = useMutation({
    mutationFn: (data: typeof formData) => api.createInstance(data),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['instances'] });
      showToast('success', `Instance '${data.instance_name}' creation started! DNS: ${data.dns_name}`);
      navigate('/instances');
    },
    onError: (error: Error) => {
      showToast('error', `Failed to create instance: ${error.message}`);
    },
  });

  const validateForm = (): boolean => {
    const newErrors: Record<string, string> = {};

    if (!formData.name) {
      newErrors.name = 'Instance name is required';
    } else if (!/^[a-z0-9-]+$/.test(formData.name)) {
      newErrors.name = 'Name must contain only lowercase letters, numbers, and hyphens';
    } else if (formData.name.length < 3) {
      newErrors.name = 'Name must be at least 3 characters';
    }

    if (!formData.password) {
      newErrors.password = 'Password is required';
    } else if (formData.password.length < 8) {
      newErrors.password = 'Password must be at least 8 characters';
    }

    if (formData.storage_size_gb < 1 || formData.storage_size_gb > 1000) {
      newErrors.storage_size_gb = 'Storage must be between 1 and 1000 GB';
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (validateForm()) {
      createMutation.mutate(formData);
    }
  };

  return (
    <div className="space-y-6">
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
          <h1 className="text-3xl font-bold tracking-tight">Create New Instance</h1>
          <p className="text-muted-foreground">
            Deploy a new PostgreSQL database instance
          </p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Instance Configuration</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-6">
            <div className="space-y-2">
              <label className="text-sm font-medium">
                Instance Name *
              </label>
              <input
                type="text"
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                placeholder="mydb"
                value={formData.name}
                onChange={(e) => setFormData({ ...formData, name: e.target.value.toLowerCase() })}
              />
              {errors.name && (
                <p className="text-sm text-destructive">{errors.name}</p>
              )}
              <p className="text-xs text-muted-foreground">
                Lowercase letters, numbers, and hyphens only. Will become: {formData.name || 'name'}.westus3.cloudapp.azure.com
              </p>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">
                Password *
              </label>
              <input
                type="password"
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                placeholder="••••••••"
                value={formData.password}
                onChange={(e) => setFormData({ ...formData, password: e.target.value })}
              />
              {errors.password && (
                <p className="text-sm text-destructive">{errors.password}</p>
              )}
              <p className="text-xs text-muted-foreground">
                Minimum 8 characters
              </p>
            </div>

            <div className="grid gap-4 md:grid-cols-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">
                  PostgreSQL Version
                </label>
                <select
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  value={formData.postgres_version}
                  onChange={(e) => setFormData({ ...formData, postgres_version: e.target.value })}
                >
                  <option value="18">18 (Latest)</option>
                  <option value="17">17</option>
                  <option value="16">16</option>
                  <option value="15">15</option>
                </select>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">
                  Storage Size (GB)
                </label>
                <input
                  type="number"
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  value={formData.storage_size_gb}
                  onChange={(e) => setFormData({ ...formData, storage_size_gb: parseInt(e.target.value) || 10 })}
                  min="1"
                  max="1000"
                />
                {errors.storage_size_gb && (
                  <p className="text-sm text-destructive">{errors.storage_size_gb}</p>
                )}
              </div>
            </div>

            <div className="flex items-center space-x-2">
              <input
                type="checkbox"
                id="internal"
                className="h-4 w-4 rounded border-input"
                checked={formData.internal}
                onChange={(e) => setFormData({ ...formData, internal: e.target.checked })}
              />
              <label htmlFor="internal" className="text-sm font-medium cursor-pointer">
                Internal only (no public IP)
              </label>
            </div>

            <div className="flex justify-end space-x-3 pt-4">
              <Button
                type="button"
                variant="outline"
                onClick={() => navigate('/instances')}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={createMutation.isPending}
              >
                {createMutation.isPending ? 'Creating...' : 'Create Instance'}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>What happens next?</CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground space-y-2">
          <p>1. A durable orchestration will be created</p>
          <p>2. Kubernetes resources are deployed (StatefulSet, Service, PVC)</p>
          <p>3. Pod will start and PostgreSQL initializes (~30-60 seconds)</p>
          <p>4. DNS name is configured automatically</p>
          <p>5. Instance actor starts monitoring health every 30 seconds</p>
          <p className="pt-2">You can track progress in the instance list or orchestrations page.</p>
        </CardContent>
      </Card>
    </div>
  );
}
