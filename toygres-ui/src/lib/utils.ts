import { type ClassValue, clsx } from 'clsx';

export function cn(...inputs: ClassValue[]) {
  return clsx(inputs);
}

export function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSecs < 60) return `${diffSecs}s ago`;
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  return `${diffDays}d ago`;
}

export function formatDateTime(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleString();
}

export function getStateColor(state: string): string {
  switch (state) {
    case 'running':
      return 'text-green-600 dark:text-green-400';
    case 'creating':
      return 'text-blue-600 dark:text-blue-400';
    case 'stopped':
      return 'text-yellow-600 dark:text-yellow-400';
    case 'deleting':
      return 'text-orange-600 dark:text-orange-400';
    case 'failed':
      return 'text-red-600 dark:text-red-400';
    case 'deleted':
      return 'text-gray-600 dark:text-gray-400';
    default:
      return 'text-gray-600 dark:text-gray-400';
  }
}

export function getHealthColor(health: string): string {
  switch (health) {
    case 'healthy':
      return 'text-green-600 dark:text-green-400';
    case 'unhealthy':
      return 'text-red-600 dark:text-red-400';
    case 'unknown':
      return 'text-gray-600 dark:text-gray-400';
    default:
      return 'text-gray-600 dark:text-gray-400';
  }
}

export function getStatusIcon(status: string): string {
  switch (status) {
    case 'Running':
      return '●';
    case 'Completed':
      return '✓';
    case 'Failed':
      return '✗';
    default:
      return '○';
  }
}

export function copyToClipboard(text: string): Promise<void> {
  return navigator.clipboard.writeText(text);
}

export function formatBytes(bytes: number, decimals: number = 2): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}

