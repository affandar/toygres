import { NavLink } from 'react-router-dom';
import { 
  Database, 
  LayoutDashboard, 
  Settings, 
  Bug,
  BarChart3,
  FileText,
  Users,
  GitBranch,
  FileCode
} from 'lucide-react';
import { cn } from '@/lib/utils';

interface NavItem {
  title: string;
  href: string;
  icon: React.ComponentType<{ className?: string }>;
  children?: NavItem[];
}

const navigation: NavItem[] = [
  {
    title: 'Dashboard',
    href: '/',
    icon: LayoutDashboard,
  },
  {
    title: 'DB Instances',
    href: '/instances',
    icon: Database,
  },
  {
    title: 'System',
    href: '/system',
    icon: Settings,
    children: [
      { title: 'Stats', href: '/system/stats', icon: BarChart3 },
      { title: 'Config', href: '/system/config', icon: FileText },
      { title: 'Environment', href: '/system/env', icon: FileCode },
      { title: 'Workers', href: '/system/workers', icon: Users },
    ],
  },
  {
    title: 'Debug',
    href: '/debug',
    icon: Bug,
    children: [
      { title: 'Orchestrations', href: '/debug/orchestrations', icon: GitBranch },
      { title: 'Logs', href: '/debug/logs', icon: FileText },
    ],
  },
];

export function Sidebar() {
  return (
    <div className="flex h-full w-64 flex-col border-r bg-card">
      <div className="flex-1 overflow-auto py-4">
        <nav className="space-y-1 px-3">
          {navigation.map((item) => (
            <div key={item.href}>
              <NavLink
                to={item.href}
                end={item.href === '/'}
                className={({ isActive }) =>
                  cn(
                    'group flex items-center rounded-md px-3 py-2 text-sm font-medium',
                    isActive
                      ? 'bg-primary text-primary-foreground'
                      : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                  )
                }
              >
                <item.icon className="mr-3 h-5 w-5" />
                {item.title}
              </NavLink>
              
              {item.children && (
                <div className="ml-6 mt-1 space-y-1">
                  {item.children.map((child) => (
                    <NavLink
                      key={child.href}
                      to={child.href}
                      className={({ isActive }) =>
                        cn(
                          'group flex items-center rounded-md px-3 py-2 text-sm',
                          isActive
                            ? 'bg-primary/10 text-primary'
                            : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                        )
                      }
                    >
                      <child.icon className="mr-3 h-4 w-4" />
                      {child.title}
                    </NavLink>
                  ))}
                </div>
              )}
            </div>
          ))}
        </nav>
      </div>
    </div>
  );
}

