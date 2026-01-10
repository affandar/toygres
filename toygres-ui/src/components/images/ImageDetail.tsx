import { useParams, useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, HardDrive, Trash2, Database, Clock, Server, Copy, Check, Plus, Terminal, RefreshCw } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/api';
import { useToast } from '@/lib/toast';
import { formatBytes, formatDateTime } from '@/lib/utils';
import { useState } from 'react';

export function ImageDetail() {
  const { name } = useParams<{ name: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { showToast } = useToast();
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [showRestoreModal, setShowRestoreModal] = useState(false);
  const [copiedField, setCopiedField] = useState<string | null>(null);
  const [restoreForm, setRestoreForm] = useState({
    name: '',
    password: '',
  });
  
  const { data: image, isLoading, error } = useQuery({
    queryKey: ['images', name],
    queryFn: () => api.getImage(name!),
    enabled: !!name,
    refetchInterval: (query) => query.state.data?.state === 'creating' ? 3000 : 10000,
  });

  // Fetch job logs - refresh more frequently while creating
  const { data: jobLogs, isLoading: logsLoading, refetch: refetchLogs } = useQuery({
    queryKey: ['image-logs', name],
    queryFn: () => api.getImageJobLogs(name!),
    enabled: !!name,
    refetchInterval: () => image?.state === 'creating' ? 5000 : false,
  });

  const deleteMutation = useMutation({
    mutationFn: () => api.deleteImage(name!),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['images'] });
      showToast('success', data.message);
      navigate('/images');
    },
    onError: (error: Error) => {
      showToast('error', `Failed to delete: ${error.message}`);
    },
  });

  const restoreMutation = useMutation({
    mutationFn: (data: { name: string; password: string; source_image_id: string }) => 
      api.createInstance({
        name: data.name,
        password: data.password,
        source_image_id: data.source_image_id,
      }),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['instances'] });
      showToast('success', `Instance "${data.instance_name}" creation started from image`);
      setShowRestoreModal(false);
      navigate('/instances');
    },
    onError: (error: Error) => {
      showToast('error', `Failed to restore: ${error.message}`);
    },
  });

  const handleRestore = () => {
    if (!image) return;
    restoreMutation.mutate({
      name: restoreForm.name,
      password: restoreForm.password,
      source_image_id: image.id,
    });
  };

  const copyToClipboard = async (text: string, field: string) => {
    await navigator.clipboard.writeText(text);
    setCopiedField(field);
    setTimeout(() => setCopiedField(null), 2000);
  };

  const getStateColor = (state: string) => {
    switch (state) {
      case 'ready':
        return 'text-green-500';
      case 'creating':
        return 'text-blue-500';
      case 'failed':
        return 'text-red-500';
      case 'deleting':
        return 'text-yellow-500';
      default:
        return 'text-gray-500';
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex flex-col items-center gap-4">
          <svg className="animate-spin h-10 w-10 text-blue-500" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" fill="none" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
          <p className="text-slate-400">Loading image details...</p>
        </div>
      </div>
    );
  }

  if (error || !image) {
    return (
      <div className="space-y-6">
        <Button variant="ghost" onClick={() => navigate('/images')}>
          <ArrowLeft className="h-4 w-4 mr-2" />
          Back to Images
        </Button>
        <Card>
          <CardContent className="py-12 text-center">
            <p className="text-red-500">Image not found or error loading details</p>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <Button variant="ghost" onClick={() => navigate('/images')}>
            <ArrowLeft className="h-4 w-4 mr-2" />
            Back
          </Button>
          <div>
            <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
              <HardDrive className="h-8 w-8" />
              {image.name}
            </h1>
            <p className="text-muted-foreground">
              Backup image from {image.source_k8s_name}
            </p>
          </div>
        </div>
        <div className="flex gap-2">
          <Button 
            variant="default" 
            onClick={() => setShowRestoreModal(true)}
            disabled={image.state !== 'ready'}
          >
            <Plus className="h-4 w-4 mr-2" />
            Restore to New Instance
          </Button>
          <Button 
            variant="destructive" 
            onClick={() => setShowDeleteModal(true)}
            disabled={image.state === 'creating' || image.state === 'deleting'}
          >
            <Trash2 className="h-4 w-4 mr-2" />
            Delete Image
          </Button>
        </div>
      </div>

      {/* Status Card */}
      <div className="grid gap-6 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">Status</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-2">
              <span className={`text-2xl font-bold ${getStateColor(image.state)}`}>
                ● {image.state.charAt(0).toUpperCase() + image.state.slice(1)}
              </span>
            </div>
            {image.state === 'creating' && (
              <p className="text-sm text-muted-foreground mt-1">
                Backup in progress...
              </p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">Backup Size</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {image.backup_size_bytes 
                ? formatBytes(image.backup_size_bytes)
                : image.state === 'creating' 
                  ? 'Calculating...' 
                  : '-'
              }
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">PostgreSQL Version</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-2">
              <Database className="h-5 w-5 text-muted-foreground" />
              <span className="text-2xl font-bold">{image.postgres_version}</span>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Details Card */}
      <Card>
        <CardHeader>
          <CardTitle>Image Details</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Image ID</p>
              <p className="text-sm font-mono">{image.id}</p>
            </div>
            
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Image Type</p>
              <p className="text-sm">{image.image_type}</p>
            </div>
            
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Source Instance</p>
              <p className="text-sm">{image.source_k8s_name}</p>
            </div>
            
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Source Namespace</p>
              <p className="text-sm">{image.source_namespace}</p>
            </div>
            
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Storage Size</p>
              <p className="text-sm">{image.storage_size_gb} GB</p>
            </div>
            
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Description</p>
              <p className="text-sm">{image.description || '(none)'}</p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Storage Location Card */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Server className="h-5 w-5" />
            Storage Location
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Blob Storage URL</p>
              <div className="flex items-center gap-2">
                <code className="flex-1 text-sm bg-background px-2 py-1 rounded border overflow-x-auto">
                  {image.blob_storage_url}
                </code>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => copyToClipboard(image.blob_storage_url, 'url')}
                >
                  {copiedField === 'url' ? <Check className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4" />}
                </Button>
              </div>
            </div>
            
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Container</p>
              <p className="text-sm font-mono">{image.blob_container}</p>
            </div>
            
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Blob Path</p>
              <div className="flex items-center gap-2">
                <code className="flex-1 text-sm bg-background px-2 py-1 rounded border overflow-x-auto">
                  {image.blob_path}
                </code>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => copyToClipboard(image.blob_path, 'path')}
                >
                  {copiedField === 'path' ? <Check className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4" />}
                </Button>
              </div>
            </div>
            
            {image.backup_checksum && (
              <div className="space-y-1">
                <p className="text-sm font-medium text-muted-foreground">Checksum (SHA256)</p>
                <code className="text-sm bg-background px-2 py-1 rounded border block overflow-x-auto">
                  {image.backup_checksum}
                </code>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Timestamps Card */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Clock className="h-5 w-5" />
            Timestamps
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Created At</p>
              <p className="text-sm">{formatDateTime(image.created_at)}</p>
            </div>
            
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">Ready At</p>
              <p className="text-sm">{image.ready_at ? formatDateTime(image.ready_at) : '-'}</p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Backup Job Logs Card */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Terminal className="h-5 w-5" />
              Backup Job Logs
            </div>
            <div className="flex items-center gap-2">
              {jobLogs && (
                <span className={`text-sm px-2 py-1 rounded ${
                  jobLogs.job_status === 'completed' ? 'bg-green-500/20 text-green-500' :
                  jobLogs.job_status === 'failed' ? 'bg-red-500/20 text-red-500' :
                  jobLogs.job_status === 'running' ? 'bg-blue-500/20 text-blue-500' :
                  'bg-gray-500/20 text-gray-500'
                }`}>
                  {jobLogs.job_status}
                </span>
              )}
              <Button
                variant="ghost"
                size="sm"
                onClick={() => refetchLogs()}
                disabled={logsLoading}
              >
                <RefreshCw className={`h-4 w-4 ${logsLoading ? 'animate-spin' : ''}`} />
              </Button>
            </div>
          </CardTitle>
        </CardHeader>
        <CardContent>
          {logsLoading && !jobLogs ? (
            <div className="flex items-center justify-center py-8">
              <div className="flex items-center gap-2 text-muted-foreground">
                <RefreshCw className="h-4 w-4 animate-spin" />
                Loading logs...
              </div>
            </div>
          ) : jobLogs ? (
            <div className="space-y-2">
              <p className="text-xs text-muted-foreground font-mono">
                Job: {jobLogs.job_name}
              </p>
              <pre className="text-xs bg-background p-4 rounded border overflow-x-auto max-h-64 overflow-y-auto whitespace-pre-wrap font-mono">
                {jobLogs.logs || '(no logs available)'}
              </pre>
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">Failed to load logs</p>
          )}
        </CardContent>
      </Card>

      {/* Error Card (if failed) */}
      {image.error_message && (
        <Card className="border-red-500/50">
          <CardHeader>
            <CardTitle className="text-red-500">Error</CardTitle>
          </CardHeader>
          <CardContent>
            <pre className="text-sm text-red-400 whitespace-pre-wrap bg-red-500/10 p-4 rounded">
              {image.error_message}
            </pre>
          </CardContent>
        </Card>
      )}

      {/* Delete Confirmation Modal */}
      {showDeleteModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <Card className="w-full max-w-md mx-4">
            <CardHeader>
              <CardTitle>Delete Image</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm text-muted-foreground">
                Are you sure you want to delete the image "{image.name}"? 
                This will also remove the backup data from blob storage.
              </p>
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={() => setShowDeleteModal(false)}>
                  Cancel
                </Button>
                <Button 
                  variant="destructive" 
                  onClick={() => deleteMutation.mutate()}
                  disabled={deleteMutation.isPending}
                >
                  {deleteMutation.isPending ? 'Deleting...' : 'Delete'}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Restore to New Instance Modal */}
      {showRestoreModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <Card className="w-full max-w-md mx-4">
            <CardHeader>
              <CardTitle>Restore to New Instance</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm text-muted-foreground">
                Create a new PostgreSQL instance from the backup image "{image.name}".
                The new instance will have PostgreSQL {image.postgres_version} and {image.storage_size_gb}GB storage.
              </p>
              
              <div className="space-y-2">
                <label className="text-sm font-medium">Instance Name</label>
                <input
                  type="text"
                  value={restoreForm.name}
                  onChange={(e) => setRestoreForm(prev => ({ ...prev, name: e.target.value }))}
                  placeholder="my-restored-db"
                  className="w-full px-3 py-2 border border-input rounded-md bg-background text-foreground"
                />
              </div>
              
              <div className="space-y-2">
                <label className="text-sm font-medium">Password</label>
                <input
                  type="password"
                  value={restoreForm.password}
                  onChange={(e) => setRestoreForm(prev => ({ ...prev, password: e.target.value }))}
                  placeholder="Enter password for new instance"
                  className="w-full px-3 py-2 border border-input rounded-md bg-background text-foreground"
                />
                <p className="text-xs text-muted-foreground">
                  Note: Use the same password as the source instance for authentication to work with existing data.
                </p>
              </div>
              
              <div className="flex justify-end gap-2 pt-2">
                <Button 
                  variant="outline" 
                  onClick={() => {
                    setShowRestoreModal(false);
                    setRestoreForm({ name: '', password: '' });
                  }}
                >
                  Cancel
                </Button>
                <Button 
                  onClick={handleRestore}
                  disabled={restoreMutation.isPending || !restoreForm.name || !restoreForm.password}
                >
                  {restoreMutation.isPending ? 'Creating...' : 'Create Instance'}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}
