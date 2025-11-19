import { useState, useEffect, useRef } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/api';

export function Logs() {
  const [filter, setFilter] = useState('');
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [limit, setLimit] = useState(500);
  const logContainerRef = useRef<HTMLDivElement>(null);

  const { data: logs } = useQuery({
    queryKey: ['server-logs', limit, filter],
    queryFn: () => api.getLogs(limit, filter || undefined),
    refetchInterval: autoRefresh ? 2000 : false,
  });

  // Auto-scroll to bottom when new logs arrive
  useEffect(() => {
    if (logContainerRef.current && autoRefresh) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs, autoRefresh]);

  const stripAnsi = (str: string) => {
    // Remove ANSI color codes (both with and without ESC character)
    // Also handle the case where logs contain duplicate content with ANSI codes
    // eslint-disable-next-line no-control-regex
    const withoutEsc = str.replace(/\x1b\[[0-9;]*m/g, ''); // Standard ANSI codes with ESC
    return withoutEsc
      .replace(/\[[\d;]*m/g, '')      // ANSI codes without ESC character (using * not + to handle [m too)
      .replace(/\[\d+m/g, '');        // Additional cleanup for any remaining codes
  };

  const parseLogLine = (line: string) => {
    // Strip ANSI codes first
    const clean = stripAnsi(line);
    
    // Try to extract structured log components
    // Format typically: "2024-11-18T00:36:31.162Z INFO duroxide::runtime [instance_id] message"
    const timestampMatch = clean.match(/^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}[Z+\-\d:]+)/);
    const levelMatch = clean.match(/\s+(TRACE|DEBUG|INFO|WARN|ERROR)\s+/);
    const targetMatch = clean.match(/\s+(duroxide::\w+|toygres[_\w]*::\w+)/);
    
    return {
      timestamp: timestampMatch?.[1],
      level: levelMatch?.[1],
      target: targetMatch?.[1],
      raw: clean,
    };
  };

  const getLevelColor = (level?: string) => {
    switch (level) {
      case 'ERROR': return 'text-red-400';
      case 'WARN': return 'text-yellow-400';
      case 'INFO': return 'text-blue-400';
      case 'DEBUG': return 'text-gray-400';
      case 'TRACE': return 'text-gray-500';
      default: return 'text-green-400';
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Server Logs</h1>
          <p className="text-muted-foreground">
            Real-time server logs and traces
          </p>
        </div>
        <div className="flex items-center space-x-2">
          <Button
            size="sm"
            variant={autoRefresh ? 'default' : 'outline'}
            onClick={() => setAutoRefresh(!autoRefresh)}
          >
            {autoRefresh ? '⏸ Pause' : '▶ Resume'}
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle>Filters & Settings</CardTitle>
            <div className="flex items-center space-x-2 text-sm text-muted-foreground">
              <span>Showing:</span>
              <select
                className="rounded border border-input bg-background px-2 py-1 text-sm"
                value={limit}
                onChange={(e) => setLimit(parseInt(e.target.value))}
              >
                <option value="100">Last 100</option>
                <option value="200">Last 200</option>
                <option value="500">Last 500</option>
                <option value="1000">Last 1000</option>
              </select>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <input
            type="text"
            placeholder="Filter logs (e.g., orchestration ID, instance name, ERROR, etc.)..."
            className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle>
              Log Output {logs && `(${logs.length} lines)`}
            </CardTitle>
            {autoRefresh && (
              <span className="text-xs text-muted-foreground animate-pulse">
                ● Live (refreshing every 2s)
              </span>
            )}
          </div>
        </CardHeader>
        <CardContent>
          <div 
            ref={logContainerRef}
            className="bg-black text-green-400 rounded-md p-4 font-mono text-xs h-[600px] overflow-y-auto overflow-x-auto"
          >
            {!logs || logs.length === 0 ? (
              <p className="text-gray-500">
                {filter ? 'No matching log entries found' : 'No logs available yet'}
              </p>
            ) : (
              logs.map((line, idx) => {
                const parsed = parseLogLine(line);
                // Remove timestamp, level, and target from the cleaned message
                const message = parsed.raw
                  .replace(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}[Z+\-\d:]+/, '')
                  .replace(/\s+(TRACE|DEBUG|INFO|WARN|ERROR)\s+/, '')
                  .replace(/\s+(duroxide::\w+|toygres[_\w]*::\w+)\s*:\s*/, '');
                
                return (
                  <div key={idx} className="whitespace-pre-wrap break-all mb-0.5">
                    {parsed.timestamp && (
                      <span className="text-gray-500">{parsed.timestamp} </span>
                    )}
                    {parsed.level && (
                      <span className={getLevelColor(parsed.level)}>{parsed.level} </span>
                    )}
                    {parsed.target && (
                      <span className="text-purple-400">{parsed.target}: </span>
                    )}
                    <span className="text-green-400">{message}</span>
                  </div>
                );
              })
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Tips</CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground space-y-2">
          <p>• Logs auto-refresh every 2 seconds when not paused</p>
          <p>• Use the filter to search for specific orchestration IDs, instance names, or log levels</p>
          <p>• Logs automatically scroll to bottom in live mode</p>
          <p>• Click "Pause" to stop auto-scroll and inspect older logs</p>
          <p>• Increase the limit dropdown to see more history</p>
        </CardContent>
      </Card>
    </div>
  );
}
