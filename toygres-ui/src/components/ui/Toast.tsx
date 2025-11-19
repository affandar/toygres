import { useState, useCallback, ReactNode } from 'react';
import { CheckCircle, XCircle, Info, AlertTriangle } from 'lucide-react';
import { ToastContext, ToastType } from '@/lib/toast';

interface Toast {
  id: string;
  type: ToastType;
  message: string;
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const showToast = useCallback((type: ToastType, message: string) => {
    const id = Math.random().toString(36).substr(2, 9);
    setToasts((prev) => [...prev, { id, type, message }]);

    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 5000);
  }, []);

  return (
    <ToastContext.Provider value={{ showToast }}>
      {children}
      <div className="fixed bottom-4 right-4 z-50 space-y-2">
        {toasts.map((toast) => (
          <ToastItem key={toast.id} toast={toast} onClose={() => {
            setToasts((prev) => prev.filter((t) => t.id !== toast.id));
          }} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

function ToastItem({ toast, onClose }: { toast: Toast; onClose: () => void }) {
  const icons = {
    success: CheckCircle,
    error: XCircle,
    info: Info,
    warning: AlertTriangle,
  };

  const colors = {
    success: 'bg-green-500 text-white',
    error: 'bg-red-500 text-white',
    info: 'bg-blue-500 text-white',
    warning: 'bg-orange-500 text-white',
  };

  const Icon = icons[toast.type];

  return (
    <div
      className={`flex items-center space-x-3 rounded-lg px-4 py-3 shadow-lg ${colors[toast.type]} min-w-[300px] animate-in slide-in-from-right`}
      onClick={onClose}
      style={{ cursor: 'pointer' }}
    >
      <Icon className="h-5 w-5" />
      <p className="text-sm flex-1">{toast.message}</p>
    </div>
  );
}

