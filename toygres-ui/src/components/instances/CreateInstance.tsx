import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, RefreshCw, Eye, EyeOff, Copy, Check } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useToast } from '@/lib/toast';
import { api } from '@/lib/api';

// Generate a secure password that meets all requirements:
// - At least 12 characters (we use 16 for extra security)
// - Contains uppercase letters
// - Contains lowercase letters
// - Contains numbers
// - Contains special characters
function generateSecurePassword(): string {
  const uppercase = 'ABCDEFGHJKLMNPQRSTUVWXYZ'; // Removed I, O to avoid confusion
  const lowercase = 'abcdefghjkmnpqrstuvwxyz'; // Removed i, l, o to avoid confusion
  const numbers = '23456789'; // Removed 0, 1 to avoid confusion
  const special = '!@#$%^&*';
  
  // Ensure at least one of each type
  let password = '';
  password += uppercase[Math.floor(Math.random() * uppercase.length)];
  password += lowercase[Math.floor(Math.random() * lowercase.length)];
  password += numbers[Math.floor(Math.random() * numbers.length)];
  password += special[Math.floor(Math.random() * special.length)];
  
  // Fill the rest with random characters from all sets
  const allChars = uppercase + lowercase + numbers + special;
  for (let i = password.length; i < 16; i++) {
    password += allChars[Math.floor(Math.random() * allChars.length)];
  }
  
  // Shuffle the password to randomize position of required characters
  return password.split('').sort(() => Math.random() - 0.5).join('');
}

export function CreateInstance() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { showToast } = useToast();

  const [formData, setFormData] = useState({
    name: '',
    password: generateSecurePassword(), // Auto-generate on initial load
    postgres_version: '17',
    storage_size_gb: 10,
    internal: false,
    image_type: 'pg_durable' as 'stock' | 'pg_durable',
  });

  const [showPassword, setShowPassword] = useState(true); // Show by default so user can see generated password
  const [copied, setCopied] = useState(false);

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
                placeholder="my-postgres-db"
                value={formData.name}
                onChange={(e) => setFormData({ ...formData, name: e.target.value.toLowerCase() })}
                autoComplete="off"
                name="instance-name"
              />
              {errors.name && (
                <p className="text-sm text-destructive">{errors.name}</p>
              )}
              <p className="text-xs text-muted-foreground">
                Lowercase letters, numbers, and hyphens only. Will become: {formData.name || 'my-postgres-db'}.westus3.cloudapp.azure.com
              </p>
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <label className="text-sm font-medium">
                  Password *
                </label>
                <div className="flex items-center gap-1">
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      navigator.clipboard.writeText(formData.password);
                      setCopied(true);
                      setTimeout(() => setCopied(false), 2000);
                    }}
                    title="Copy password"
                  >
                    {copied ? <Check className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4" />}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => setFormData({ ...formData, password: generateSecurePassword() })}
                    title="Generate new password"
                  >
                    <RefreshCw className="h-4 w-4" />
                  </Button>
                </div>
              </div>
              <div className="relative">
                <input
                  type={showPassword ? 'text' : 'password'}
                  className="w-full rounded-md border border-input bg-background px-3 py-2 pr-10 text-sm font-mono"
                  placeholder="••••••••"
                  value={formData.password}
                  onChange={(e) => setFormData({ ...formData, password: e.target.value })}
                  autoComplete="new-password"
                  name="instance-password"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="absolute right-0 top-0 h-full px-3"
                  onClick={() => setShowPassword(!showPassword)}
                  title={showPassword ? 'Hide password' : 'Show password'}
                >
                  {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </Button>
              </div>
              {errors.password && (
                <p className="text-sm text-destructive">{errors.password}</p>
              )}
              <p className="text-xs text-muted-foreground">
                Auto-generated secure password (16 chars with uppercase, lowercase, numbers, and symbols). Copy it before creating!
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

            <div className="space-y-2">
              <label className="text-sm font-medium">
                Image Type
              </label>
              <div className="grid gap-3 md:grid-cols-2">
                <div
                  className={`relative rounded-lg border-2 p-4 cursor-pointer transition-colors ${
                    formData.image_type === 'stock'
                      ? 'border-primary bg-primary/5'
                      : 'border-input hover:border-muted-foreground/50'
                  }`}
                  onClick={() => setFormData({ ...formData, image_type: 'stock' })}
                >
                  <div className="flex items-center gap-2">
                    <div className={`h-4 w-4 rounded-full border-2 ${
                      formData.image_type === 'stock' ? 'border-primary bg-primary' : 'border-muted-foreground'
                    }`}>
                      {formData.image_type === 'stock' && (
                        <div className="h-full w-full rounded-full bg-primary" />
                      )}
                    </div>
                    <span className="font-medium">Stock PostgreSQL</span>
                  </div>
                  <p className="mt-2 text-xs text-muted-foreground">
                    Standard PostgreSQL image. Best for typical database workloads.
                  </p>
                </div>
                <div
                  className={`relative rounded-lg border-2 p-4 cursor-pointer transition-colors ${
                    formData.image_type === 'pg_durable'
                      ? 'border-primary bg-primary/5'
                      : 'border-input hover:border-muted-foreground/50'
                  }`}
                  onClick={() => setFormData({ ...formData, image_type: 'pg_durable' })}
                >
                  <div className="flex items-center gap-2">
                    <div className={`h-4 w-4 rounded-full border-2 ${
                      formData.image_type === 'pg_durable' ? 'border-primary bg-primary' : 'border-muted-foreground'
                    }`}>
                      {formData.image_type === 'pg_durable' && (
                        <div className="h-full w-full rounded-full bg-primary" />
                      )}
                    </div>
                    <span className="font-medium">pg_durable</span>
                    <span className="ml-auto text-xs bg-blue-500/20 text-blue-500 px-2 py-0.5 rounded">Durable SQL</span>
                  </div>
                  <p className="mt-2 text-xs text-muted-foreground">
                    PostgreSQL with Duroxide extension for durable SQL functions and orchestrations.
                  </p>
                </div>
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
