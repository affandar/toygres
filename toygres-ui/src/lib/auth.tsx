import { createContext, useContext, useState, useEffect, ReactNode } from 'react';

interface AuthContextType {
  isAuthenticated: boolean;
  isLoading: boolean;
  login: (username: string, password: string) => Promise<boolean>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  // Check if user is already authenticated on mount
  useEffect(() => {
    checkAuth();
  }, []);

  async function checkAuth() {
    try {
      // Try to access a protected endpoint to check if authenticated
      const response = await fetch('/api/instances', {
        credentials: 'include',
      });
      setIsAuthenticated(response.ok);
    } catch {
      setIsAuthenticated(false);
    } finally {
      setIsLoading(false);
    }
  }

  async function login(username: string, password: string): Promise<boolean> {
    try {
      const formData = new URLSearchParams();
      formData.append('username', username);
      formData.append('password', password);

      const response = await fetch('/login', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/x-www-form-urlencoded',
        },
        body: formData,
        credentials: 'include',
        redirect: 'manual', // Don't follow redirects automatically
      });

      // Backend redirects to / on success, /login?error=invalid on failure
      // With redirect: 'manual', we get a 0 status for redirects in some browsers
      // or the redirect response itself
      if (response.type === 'opaqueredirect' || response.status === 200 || response.redirected) {
        // Check if we're actually authenticated now
        const checkResponse = await fetch('/api/instances', {
          credentials: 'include',
        });
        if (checkResponse.ok) {
          setIsAuthenticated(true);
          return true;
        }
      }
      
      setIsAuthenticated(false);
      return false;
    } catch {
      setIsAuthenticated(false);
      return false;
    }
  }

  async function logout(): Promise<void> {
    try {
      await fetch('/logout', {
        method: 'POST',
        credentials: 'include',
      });
    } catch {
      // Ignore errors
    }
    setIsAuthenticated(false);
  }

  return (
    <AuthContext.Provider value={{ isAuthenticated, isLoading, login, logout }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}

