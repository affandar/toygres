import { useQuery } from '@tanstack/react-query';
import { Server } from 'lucide-react';
import { api } from '@/lib/api';

export function Header() {
  const { data: status } = useQuery({
    queryKey: ['server-status'],
    queryFn: () => api.getServerStatus(),
    refetchInterval: 5000, // Check every 5 seconds
  });

  return (
    <header className="sticky top-0 z-50 w-full border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="container flex h-14 items-center px-4">
        <div className="flex items-center space-x-2">
          <Server className="h-6 w-6 text-primary" />
          <span className="text-xl font-bold">TOYGRES</span>
        </div>
        
        <div className="ml-auto flex items-center space-x-4">
          <div className="flex items-center space-x-2">
            {status?.serverRunning ? (
              <>
                <div className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
                <span className="text-sm text-muted-foreground">
                  Server Online {status.version && `(v${status.version})`}
                </span>
              </>
            ) : (
              <>
                <div className="h-2 w-2 rounded-full bg-red-500" />
                <span className="text-sm text-muted-foreground">Server Offline</span>
              </>
            )}
          </div>
        </div>
      </div>
    </header>
  );
}

