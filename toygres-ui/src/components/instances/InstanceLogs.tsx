import { useState, useEffect, useRef } from 'react';
import { useQuery } from '@tanstack/react-query';
import { RefreshCw, Download, Maximize2, Minimize2 } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/api';

interface InstanceLogsProps {
  instanceName: string;
}

export function InstanceLogs({ instanceName }: InstanceLogsProps) {
  const [tailLines, setTailLines] = useState(200);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [isExpanded, setIsExpanded] = useState(false);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const logsContainerRef = useRef<HTMLDivElement>(null);

  const { data, isLoading, error, refetch, isFetching } = useQuery({
    queryKey: ['instance-logs', instanceName, tailLines],
    queryFn: () => api.getInstanceLogs(instanceName, tailLines),
    refetchInterval: autoRefresh ? 5000 : false,
    staleTime: 2000,
  });

  // Auto-scroll to bottom when new logs arrive
  useEffect(() => {
    if (data && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [data]);

  const handleDownload = () => {
    if (!data) return;
    const content = data.logs.join('\n');
    const blob = new Blob([content], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${instanceName}-logs.txt`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  const formatLogLine = (line: string, index: number) => {
    // Parse timestamp from the beginning of the line (format: 2024-01-01T12:00:00.000Z)
    const timestampMatch = line.match(/^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)\s+(.*)$/);
    
    if (timestampMatch) {
      const [, timestamp, message] = timestampMatch;
      const date = new Date(timestamp);
      const formattedTime = date.toLocaleTimeString();
      
      // Color code based on log level
      let levelColor = 'text-slate-400';
      if (message.includes('ERROR') || message.includes('FATAL')) {
        levelColor = 'text-red-400';
      } else if (message.includes('WARNING') || message.includes('WARN')) {
        levelColor = 'text-yellow-400';
      } else if (message.includes('LOG') || message.includes('INFO')) {
        levelColor = 'text-blue-400';
      }
      
      return (
        <div key={index} className="flex gap-2 hover:bg-slate-800/50 px-2 py-0.5">
          <span className="text-slate-500 text-xs shrink-0 font-mono">{formattedTime}</span>
          <span className={`text-xs font-mono ${levelColor} break-all`}>{message}</span>
        </div>
      );
    }
    
    // Non-timestamped line
    return (
      <div key={index} className="px-2 py-0.5 hover:bg-slate-800/50">
        <span className="text-xs font-mono text-slate-300 break-all">{line}</span>
      </div>
    );
  };

  return (
    <Card className={isExpanded ? 'fixed inset-4 z-50 flex flex-col' : ''}>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-lg flex items-center gap-2">
          PostgreSQL Logs
          {data && (
            <span className="text-xs text-muted-foreground font-normal">
              ({data.pod_name})
            </span>
          )}
        </CardTitle>
        <div className="flex items-center gap-2">
          <select
            value={tailLines}
            onChange={(e) => setTailLines(Number(e.target.value))}
            className="text-xs bg-background border rounded px-2 py-1"
          >
            <option value={100}>Last 100 lines</option>
            <option value={200}>Last 200 lines</option>
            <option value={500}>Last 500 lines</option>
            <option value={1000}>Last 1000 lines</option>
          </select>
          <Button
            variant={autoRefresh ? 'default' : 'outline'}
            size="sm"
            onClick={() => setAutoRefresh(!autoRefresh)}
            className="text-xs"
          >
            <RefreshCw className={`h-3 w-3 mr-1 ${autoRefresh && isFetching ? 'animate-spin' : ''}`} />
            {autoRefresh ? 'Live' : 'Paused'}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => refetch()}
            disabled={isFetching}
          >
            <RefreshCw className={`h-3 w-3 ${isFetching ? 'animate-spin' : ''}`} />
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleDownload}
            disabled={!data}
          >
            <Download className="h-3 w-3" />
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setIsExpanded(!isExpanded)}
          >
            {isExpanded ? <Minimize2 className="h-3 w-3" /> : <Maximize2 className="h-3 w-3" />}
          </Button>
        </div>
      </CardHeader>
      <CardContent className={`${isExpanded ? 'flex-1 overflow-hidden' : ''}`}>
        {isLoading ? (
          <div className="flex items-center justify-center h-48">
            <RefreshCw className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : error ? (
          <div className="flex flex-col items-center justify-center h-48 text-center">
            <p className="text-sm text-destructive mb-2">Failed to load logs</p>
            <p className="text-xs text-muted-foreground">
              {error instanceof Error ? error.message : 'Unknown error'}
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => refetch()}
              className="mt-4"
            >
              Retry
            </Button>
          </div>
        ) : data && data.logs.length > 0 ? (
          <div
            ref={logsContainerRef}
            className={`bg-slate-900 rounded-lg overflow-auto ${
              isExpanded ? 'h-full' : 'h-96'
            }`}
          >
            <div className="py-2">
              {data.logs.map((line, index) => formatLogLine(line, index))}
              <div ref={logsEndRef} />
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center h-48 text-muted-foreground">
            <p className="text-sm">No logs available</p>
          </div>
        )}
        {data && (
          <div className="flex justify-between items-center mt-2 text-xs text-muted-foreground">
            <span>Namespace: {data.namespace}</span>
            <span>{data.log_count} lines</span>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

