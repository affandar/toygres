import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ToastProvider } from '@/components/ui/Toast';
import { Layout } from '@/components/layout/Layout';
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

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <BrowserRouter>
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
      </BrowserRouter>
      </ToastProvider>
    </QueryClientProvider>
  );
}

export default App;

