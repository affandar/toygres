import { useQuery } from '@tanstack/react-query';
import { Server, LogOut } from 'lucide-react';
import { api } from '@/lib/api';
import { useAuth } from '@/lib/auth';

export function Header() {
  const { logout } = useAuth();
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
          
          <button
            onClick={logout}
            className="flex items-center gap-2 px-3 py-1.5 text-sm text-muted-foreground hover:text-foreground hover:bg-accent rounded-md transition-colors"
            title="Sign out"
          >
            <LogOut className="h-4 w-4" />
            <span className="hidden sm:inline">Sign Out</span>
          </button>
        </div>
      </div>
    </header>
  );
}

