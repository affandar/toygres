# Toygres UI

Modern web interface for Toygres PostgreSQL as a Service.

## Features

- 📊 **Dashboard** - Overview of instances and system status
- 🗄️ **Instance Management** - View and manage PostgreSQL instances
- 📈 **System Monitoring** - Stats, configuration, and worker status
- 🔬 **Debug Tools** - Orchestration diagnostics and live logs

## Tech Stack

- **React** + **TypeScript** - Modern UI framework
- **Vite** - Fast build tool and dev server
- **TailwindCSS** - Utility-first CSS framework
- **React Query** - Server state management
- **React Router** - Client-side routing
- **Lucide React** - Beautiful icon library

## Getting Started

1. **Install dependencies:**
   ```bash
   npm install
   ```

2. **Start the development server:**
   ```bash
   npm run dev
   ```

3. **Ensure Toygres server is running:**
   ```bash
   cd .. && ./target/debug/toygres-server server start
   ```

4. **Open in browser:**
   ```
   http://localhost:3000
   ```

## Development

- `npm run dev` - Start dev server (port 3000)
- `npm run build` - Build for production
- `npm run preview` - Preview production build
- `npm run lint` - Run ESLint

## API Proxy

The dev server proxies `/api` and `/health` requests to `http://localhost:8080` (configured in `vite.config.ts`).

## Project Structure

```
src/
├── components/
│   ├── layout/      # Header, Sidebar, Layout
│   ├── dashboard/   # Dashboard page
│   ├── instances/   # Instance management pages
│   ├── system/      # System monitoring pages
│   ├── debug/       # Debug and diagnostics pages
│   └── ui/          # Reusable UI components
├── lib/
│   ├── api.ts       # API client
│   ├── types.ts     # TypeScript types
│   └── utils.ts     # Utility functions
├── App.tsx          # Router setup
└── main.tsx         # Entry point
```

## Notes

- The UI automatically refreshes data every 5 seconds
- Server status indicator in header updates in real-time
- Click on any table row to view details

