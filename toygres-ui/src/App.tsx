import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ToastProvider } from '@/components/ui/Toast';
import { AuthProvider, useAuth } from '@/lib/auth';
import { Layout } from '@/components/layout/Layout';
import { Login } from '@/components/auth/Login';
import { Dashboard } from '@/components/dashboard/Dashboard';
import { InstanceList } from '@/components/instances/InstanceList';
import { InstanceDetail } from '@/components/instances/InstanceDetail';
import { CreateInstance } from '@/components/instances/CreateInstance';
import { BulkCreateInstance } from '@/components/instances/BulkCreateInstance';
import { Stats } from '@/components/system/Stats';
import { Config } from '@/components/system/Config';
import { Workers } from '@/components/system/Workers';
import { Environment } from '@/components/system/Environment';
import { Orchestrations } from '@/components/debug/Orchestrations';
import { Logs } from '@/components/debug/Logs';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

function AppRoutes() {
  const { isAuthenticated, isLoading } = useAuth();

  // Show loading spinner while checking auth
  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-slate-900 via-purple-950 to-slate-900">
        <div className="flex flex-col items-center gap-4">
          <svg className="animate-spin h-10 w-10 text-blue-500" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" fill="none" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
          <p className="text-slate-400">Loading...</p>
        </div>
      </div>
    );
  }

  // Show login page if not authenticated
  if (!isAuthenticated) {
    return <Login />;
  }

  // Show main app if authenticated
  return (
    <Routes>
      <Route path="/" element={<Layout />}>
        <Route index element={<Dashboard />} />
        
        {/* Instances */}
        <Route path="instances">
          <Route index element={<InstanceList />} />
          <Route path="create" element={<CreateInstance />} />
          <Route path="bulk-create" element={<BulkCreateInstance />} />
          <Route path=":name" element={<InstanceDetail />} />
        </Route>
        
        {/* System */}
        <Route path="system">
          <Route path="stats" element={<Stats />} />
          <Route path="config" element={<Config />} />
          <Route path="workers" element={<Workers />} />
          <Route path="env" element={<Environment />} />
        </Route>
        
        {/* Debug */}
        <Route path="debug">
          <Route path="orchestrations" element={<Orchestrations />} />
          <Route path="logs" element={<Logs />} />
        </Route>
      </Route>
    </Routes>
  );
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <AuthProvider>
          <BrowserRouter>
            <AppRoutes />
          </BrowserRouter>
        </AuthProvider>
      </ToastProvider>
    </QueryClientProvider>
  );
}

export default App;
