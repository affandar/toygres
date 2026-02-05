import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Package, Plus } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/api';
import { useToast } from '@/lib/toast';

export function RuntimeImageCatalog() {
  const queryClient = useQueryClient();
  const { showToast } = useToast();

  const { data: images, isLoading } = useQuery({
    queryKey: ['runtime-images'],
    queryFn: () => api.listRuntimeImages(),
    refetchInterval: 5000,
  });

  const [form, setForm] = useState({
    name: '',
    description: '',
    acr_ref: 'toygresacr.azurecr.io/',
    digest: 'sha256:',
  });

  const registerMutation = useMutation({
    mutationFn: () =>
      api.registerRuntimeImage({
        name: form.name,
        description: form.description || undefined,
        acr_ref: form.acr_ref,
        digest: form.digest,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['runtime-images'] });
      showToast('success', 'Runtime image registered');
      setForm((prev) => ({ ...prev, name: '', description: '' }));
    },
    onError: (error: Error) => {
      showToast('error', `Failed to register: ${error.message}`);
    },
  });

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Runtime Images</h1>
        <p className="text-muted-foreground">
          Register existing OCI images in the Toygres ACR for use during instance creation
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Register Image</CardTitle>
        </CardHeader>
        <CardContent>
          <form
            className="grid gap-4 md:grid-cols-2"
            onSubmit={(e) => {
              e.preventDefault();
              registerMutation.mutate();
            }}
          >
            <div className="space-y-2">
              <label className="text-sm font-medium">Name *</label>
              <input
                className="w-full h-10 rounded-md border border-input bg-background px-3 text-sm"
                placeholder="my-pg-build"
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
              />
              <p className="text-xs text-muted-foreground">Used in the UI dropdown.</p>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">Deployment Mode</label>
              <input
                className="w-full h-10 rounded-md border border-input bg-background px-3 text-sm"
                value="stock"
                disabled
              />
              <p className="text-xs text-muted-foreground">
                Runtime images are always treated as stock PostgreSQL. `pg_durable` is a built-in special image.
              </p>
            </div>

            <div className="space-y-2 md:col-span-2">
              <label className="text-sm font-medium">ACR Ref *</label>
              <input
                className="w-full h-10 rounded-md border border-input bg-background px-3 text-sm font-mono"
                placeholder="toygresacr.azurecr.io/repo:tag"
                value={form.acr_ref}
                onChange={(e) => setForm({ ...form, acr_ref: e.target.value })}
              />
              <p className="text-xs text-muted-foreground">
                Must start with the Toygres ACR host (server validates `TOYGRES_ACR_HOST`).
              </p>
            </div>

            <div className="space-y-2 md:col-span-2">
              <label className="text-sm font-medium">Digest *</label>
              <input
                className="w-full h-10 rounded-md border border-input bg-background px-3 text-sm font-mono"
                placeholder="sha256:..."
                value={form.digest}
                onChange={(e) => setForm({ ...form, digest: e.target.value })}
              />
              <p className="text-xs text-muted-foreground">Phase 1 requires a digest-pinned ref for safety.</p>
            </div>

            <div className="space-y-2 md:col-span-2">
              <label className="text-sm font-medium">Description</label>
              <input
                className="w-full h-10 rounded-md border border-input bg-background px-3 text-sm"
                placeholder="Optional notes"
                value={form.description}
                onChange={(e) => setForm({ ...form, description: e.target.value })}
              />
            </div>

            <div className="md:col-span-2 flex justify-end">
              <Button type="submit" disabled={registerMutation.isPending}>
                <Plus className="mr-2 h-4 w-4" />
                {registerMutation.isPending ? 'Registering...' : 'Register'}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Catalog</CardTitle>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex items-center justify-center h-40">
              <p className="text-sm text-muted-foreground">Loading runtime images...</p>
            </div>
          ) : !images || images.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-10">
              <Package className="h-10 w-10 text-muted-foreground mb-3" />
              <p className="text-sm text-muted-foreground">No runtime images registered.</p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-sm text-muted-foreground">
                    <th className="pb-3 px-2 font-medium">Name</th>
                    <th className="pb-3 px-2 font-medium whitespace-nowrap">Type</th>
                    <th className="pb-3 px-2 font-medium">ACR Ref</th>
                    <th className="pb-3 px-2 font-medium">Digest</th>
                    <th className="pb-3 px-2 font-medium whitespace-nowrap">Created</th>
                  </tr>
                </thead>
                <tbody>
                  {images.map((img) => (
                    <tr key={img.id} className="border-b">
                      <td className="py-3 px-2 font-medium break-words">{img.name}</td>
                      <td className="py-3 px-2 whitespace-nowrap">{img.suggested_image_type}</td>
                      <td className="py-3 px-2 font-mono text-xs break-all">{img.acr_ref}</td>
                      <td className="py-3 px-2 font-mono text-xs break-all">{img.digest}</td>
                      <td className="py-3 px-2 whitespace-nowrap">{new Date(img.created_at).toLocaleDateString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
