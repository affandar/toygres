import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { HardDrive, Plus, Trash2, X } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/api';
import { useToast } from '@/lib/toast';
import { formatBytes } from '@/lib/utils';

export function ImageList() {
  const navigate = useNavigate();
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const { showToast } = useToast();
  
  const { data: images, isLoading } = useQuery({
    queryKey: ['images'],
    queryFn: () => api.listImages(),
    refetchInterval: 5000,
  });

  const deleteMutation = useMutation({
    mutationFn: (name: string) => api.deleteImage(name),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['images'] });
      showToast('success', data.message);
      setShowDeleteModal(false);
      setDeleteTarget(null);
    },
    onError: (error: Error) => {
      showToast('error', `Failed to delete: ${error.message}`);
    },
  });

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

  const handleDelete = (name: string, event: React.MouseEvent) => {
    event.stopPropagation();
    setDeleteTarget(name);
    setShowDeleteModal(true);
  };

  const confirmDelete = () => {
    if (deleteTarget) {
      deleteMutation.mutate(deleteTarget);
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
          <p className="text-slate-400">Loading images...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Backup Images</h1>
          <p className="text-muted-foreground">
            Manage PostgreSQL backup images for instance cloning
          </p>
        </div>
        <div className="flex gap-2">
          <Button onClick={() => navigate('/images/create')}>
            <Plus className="mr-2 h-4 w-4" />
            Create Image
          </Button>
        </div>
      </div>

      {!images || images.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <HardDrive className="h-12 w-12 text-muted-foreground mb-4" />
            <h3 className="text-lg font-semibold mb-2">No images found</h3>
            <p className="text-sm text-muted-foreground mb-4">
              Create a backup image from a running PostgreSQL instance
            </p>
            <Button onClick={() => navigate('/images/create')}>
              <Plus className="mr-2 h-4 w-4" />
              Create Image
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle>All Images ({images.length})</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b text-left text-sm text-muted-foreground">
                    <th className="pb-3 font-medium">Name</th>
                    <th className="pb-3 font-medium">Status</th>
                    <th className="pb-3 font-medium">Source Instance</th>
                    <th className="pb-3 font-medium">PG Version</th>
                    <th className="pb-3 font-medium">Size</th>
                    <th className="pb-3 font-medium">Created</th>
                    <th className="pb-3 font-medium text-right">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {images.map((image) => (
                    <tr
                      key={image.id}
                      className="border-b hover:bg-accent/50 transition-colors cursor-pointer"
                      onClick={() => navigate(`/images/${image.name}`)}
                    >
                      <td className="py-3 font-medium">{image.name}</td>
                      <td className="py-3">
                        <span className={getStateColor(image.state)}>
                          ● {image.state}
                        </span>
                      </td>
                      <td className="py-3">{image.source_k8s_name}</td>
                      <td className="py-3">PostgreSQL {image.postgres_version}</td>
                      <td className="py-3">
                        {image.backup_size_bytes 
                          ? formatBytes(image.backup_size_bytes)
                          : image.state === 'creating' 
                            ? '...' 
                            : '-'
                        }
                      </td>
                      <td className="py-3">
                        {new Date(image.created_at).toLocaleDateString()}
                      </td>
                      <td className="py-3 text-right">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={(e) => handleDelete(image.name, e)}
                          disabled={image.state === 'creating' || image.state === 'deleting'}
                        >
                          <Trash2 className="h-4 w-4 text-red-500" />
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Delete Confirmation Modal */}
      {showDeleteModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <Card className="w-full max-w-md mx-4">
            <CardHeader className="flex flex-row items-center justify-between">
              <CardTitle>Delete Image</CardTitle>
              <Button variant="ghost" size="sm" onClick={() => setShowDeleteModal(false)}>
                <X className="h-4 w-4" />
              </Button>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm text-muted-foreground">
                Are you sure you want to delete the image "{deleteTarget}"? 
                This will also remove the backup data from blob storage.
              </p>
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={() => setShowDeleteModal(false)}>
                  Cancel
                </Button>
                <Button 
                  variant="destructive" 
                  onClick={confirmDelete}
                  disabled={deleteMutation.isPending}
                >
                  {deleteMutation.isPending ? 'Deleting...' : 'Delete'}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}
