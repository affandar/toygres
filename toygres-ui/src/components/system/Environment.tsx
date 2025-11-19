import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { CheckCircle, XCircle } from 'lucide-react';

export function Environment() {
  const envVars = [
    { name: 'DATABASE_URL', status: true, description: 'PostgreSQL connection for CMS and Duroxide' },
    { name: 'AKS_CLUSTER_NAME', status: true, description: 'Target AKS cluster name' },
    { name: 'AKS_RESOURCE_GROUP', status: true, description: 'Azure resource group' },
    { name: 'AKS_REGION', status: true, description: 'Azure region (westus3)' },
    { name: 'TOYGRES_API_URL', status: false, description: 'Override API URL (optional)' },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Environment Variables</h1>
        <p className="text-muted-foreground">
          Server environment configuration status
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Required Environment Variables</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            {envVars.map((env) => (
              <div
                key={env.name}
                className="flex items-start justify-between p-3 rounded-md border"
              >
                <div className="flex-1">
                  <div className="flex items-center space-x-2">
                    <code className="text-sm font-mono">{env.name}</code>
                    {env.status ? (
                      <CheckCircle className="h-4 w-4 text-green-600" />
                    ) : (
                      <XCircle className="h-4 w-4 text-red-600" />
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground mt-1">{env.description}</p>
                </div>
                <span className={`text-xs px-2 py-1 rounded ${env.status ? 'bg-green-100 text-green-700' : 'bg-gray-100 text-gray-700'}`}>
                  {env.status ? 'Set' : 'Not set'}
                </span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Security Note</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            Actual secret values are not exposed via the Web UI for security reasons.
            Use the CLI to view with caution:
          </p>
          <code className="block mt-4 p-4 bg-muted rounded-md text-sm">
            ./toygres-server server env --show-secrets
          </code>
        </CardContent>
      </Card>
    </div>
  );
}

