import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { ArrowLeft, HardDrive, ShieldCheck } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/api';
import { useToast } from '@/lib/toast';

export function CreateImage() {
  const navigate = useNavigate();
  const { showToast } = useToast();
  
  const [imageName, setImageName] = useState('');
  const [description, setDescription] = useState('');
  const [sourceInstance, setSourceInstance] = useState('');
  
  // Fetch running instances to select from
  const { data: instances, isLoading: instancesLoading } = useQuery({
    queryKey: ['instances'],
    queryFn: () => api.listInstances(),
  });
  
  const runningInstances = instances?.filter(i => i.state === 'running') || [];

  const createMutation = useMutation({
    mutationFn: (data: { name: string; source_k8s_name: string; description?: string }) => 
      api.createImage(data),
    onSuccess: (data) => {
      showToast('success', `Image creation started: ${data.image_name}`);
      navigate('/images');
    },
    onError: (error: Error) => {
      showToast('error', `Failed to create image: ${error.message}`);
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!imageName.trim()) {
      showToast('error', 'Please enter an image name');
      return;
    }
    
    if (!sourceInstance) {
      showToast('error', 'Please select a source instance');
      return;
    }
    
    const selectedInstance = runningInstances.find(i => i.user_name === sourceInstance);
    if (!selectedInstance) {
      showToast('error', 'Selected instance not found');
      return;
    }
    
    createMutation.mutate({
      name: imageName.trim(),
      source_k8s_name: selectedInstance.k8s_name,
      description: description.trim() || undefined,
    });
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" onClick={() => navigate('/images')}>
          <ArrowLeft className="h-4 w-4 mr-2" />
          Back
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Create Backup Image</h1>
          <p className="text-muted-foreground">
            Create a point-in-time backup image from a running PostgreSQL instance
          </p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <HardDrive className="h-5 w-5" />
            New Backup Image
          </CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-6">
            {/* Image Name */}
            <div className="space-y-2">
              <label className="text-sm font-medium" htmlFor="imageName">
                Image Name
              </label>
              <input
                id="imageName"
                type="text"
                value={imageName}
                onChange={(e) => setImageName(e.target.value)}
                placeholder="e.g., prod-backup-jan-2026"
                className="w-full rounded-md border bg-background px-3 py-2 text-sm"
                pattern="[a-zA-Z0-9\-_]+"
                title="Only alphanumeric characters, hyphens, and underscores"
              />
              <p className="text-xs text-muted-foreground">
                Use only letters, numbers, hyphens, and underscores
              </p>
            </div>

            {/* Description */}
            <div className="space-y-2">
              <label className="text-sm font-medium" htmlFor="description">
                Description (optional)
              </label>
              <textarea
                id="description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="e.g., Production database backup before major update"
                className="w-full rounded-md border bg-background px-3 py-2 text-sm min-h-[80px]"
              />
            </div>

            {/* Source Instance */}
            <div className="space-y-2">
              <label className="text-sm font-medium" htmlFor="sourceInstance">
                Source Instance
              </label>
              {instancesLoading ? (
                <p className="text-sm text-muted-foreground">Loading instances...</p>
              ) : runningInstances.length === 0 ? (
                <div className="p-4 rounded-md border border-yellow-500/50 bg-yellow-500/10">
                  <p className="text-sm text-yellow-500">
                    No running instances available. Create and start an instance first.
                  </p>
                </div>
              ) : (
                <select
                  id="sourceInstance"
                  value={sourceInstance}
                  onChange={(e) => setSourceInstance(e.target.value)}
                  className="w-full rounded-md border bg-background px-3 py-2 text-sm"
                >
                  <option value="">Select an instance...</option>
                  {runningInstances.map((instance) => (
                    <option key={instance.k8s_name} value={instance.user_name}>
                      {instance.user_name} (PostgreSQL {instance.postgres_version}, {instance.storage_size_gb}GB)
                    </option>
                  ))}
                </select>
              )}
            </div>

            {/* Password (Auto-detected) */}
            <div className="space-y-2">
              <label className="text-sm font-medium">
                Instance Password
              </label>
              <div className="flex items-center gap-2 p-3 rounded-md border bg-muted/50">
                <ShieldCheck className="h-4 w-4 text-green-500" />
                <span className="text-sm text-muted-foreground">
                  Automatically detected from instance configuration
                </span>
              </div>
            </div>

            {/* Info Box */}
            <div className="p-4 rounded-md border border-blue-500/50 bg-blue-500/10">
              <h4 className="text-sm font-medium text-blue-500 mb-2">How it works</h4>
              <ul className="text-sm text-muted-foreground space-y-1">
                <li>• Uses <code className="bg-background px-1 rounded">pg_basebackup</code> for consistent physical backup</li>
                <li>• Zero downtime - backup runs while instance is live</li>
                <li>• Backup is stored in Azure Blob Storage</li>
                <li>• Can be used to create new instances quickly</li>
              </ul>
            </div>

            {/* Submit */}
            <div className="flex justify-end gap-2">
              <Button type="button" variant="outline" onClick={() => navigate('/images')}>
                Cancel
              </Button>
              <Button 
                type="submit" 
                disabled={createMutation.isPending || runningInstances.length === 0}
              >
                {createMutation.isPending ? 'Creating...' : 'Create Image'}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
