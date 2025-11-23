# Toygres Control Plane - Quick Start Guide

## One-Command Startup

Start everything (observability + backend + frontend + logs):

```bash
./scripts/start-control-plane.sh
```

This starts:
- ✅ Docker observability stack (Grafana, Prometheus, Loki, OTLP Collector)
- ✅ Toygres backend server (API on port 8080)
- ✅ Web UI (on port 3000)
- ✅ Log forwarder (pushes logs to Loki)

## One-Command Shutdown

Stop everything:

```bash
./scripts/stop-control-plane.sh
```

Or use **Ctrl+C** in the terminal where you started the control plane - it will gracefully shut down all services.

## Clean Shutdown (Remove Volumes)

To stop and remove all Docker volumes (metrics/logs data):

```bash
./scripts/stop-control-plane.sh --clean
```

## What Gets Started

### Services

| Service | URL | Description |
|---------|-----|-------------|
| **Web UI** | http://localhost:3000 | PostgreSQL instance management |
| **Backend API** | http://localhost:8080 | REST API |
| **Grafana** | http://localhost:3001 | Metrics & logs dashboards (admin/admin) |
| **Prometheus** | http://localhost:9090 | Metrics storage |

### Background Processes

1. **Toygres Server** - Daemon process managing orchestrations
2. **Log Forwarder** - Tails server logs and pushes to Loki
3. **Web UI** - React development server

### Docker Containers

- `toygres-grafana` - Visualization
- `toygres-prometheus` - Metrics database
- `toygres-loki` - Log aggregation
- `toygres-otel-collector` - OpenTelemetry collector

## Useful Commands

### View Logs
```bash
# View backend logs
./toygres server logs

# Follow logs
./toygres server logs -f
```

### Check Status
```bash
# Check observability stack
./scripts/observability-status.sh

# Check toygres server
./toygres server status
```

### Restart Individual Services

**Backend only:**
```bash
./toygres server stop
./toygres server start
```

**Observability only:**
```bash
./scripts/stop-observability.sh
./scripts/start-observability.sh
```

**Web UI only:**
```bash
cd toygres-ui
npm start
```

## Troubleshooting

### "Port already in use"

Check what's running:
```bash
lsof -i :3000  # Web UI
lsof -i :8080  # Backend
lsof -i :3001  # Grafana
```

Kill processes:
```bash
./scripts/stop-control-plane.sh
```

### "Backend failed to start"

Check logs:
```bash
./toygres server logs
```

Common issues:
- DATABASE_URL not set in `.env`
- PostgreSQL not running
- Port 8080 in use

### "Observability stack not responding"

Check Docker:
```bash
docker compose -f docker-compose.observability.yml ps
docker compose -f docker-compose.observability.yml logs
```

Restart:
```bash
./scripts/stop-observability.sh
./scripts/start-observability.sh
```

### "Log forwarder not working"

Check if running:
```bash
cat .pids/log-forwarder.pid
ps aux | grep push-logs-to-loki
```

Manually start:
```bash
./scripts/push-logs-to-loki.sh
```

## Development Workflow

### Daily Development

1. **Start everything:**
   ```bash
   ./scripts/start-control-plane.sh
   ```

2. **Code changes** - Backend rebuilds automatically on restart
   
3. **Restart backend** (if needed):
   ```bash
   ./toygres server stop
   ./toygres server start
   ```

4. **View metrics/logs** in Grafana: http://localhost:3001

5. **End of day:**
   ```bash
   # Press Ctrl+C in the control plane terminal
   # Or run:
   ./scripts/stop-control-plane.sh
   ```

### Quick Testing

**Just run backend + UI (no observability):**
```bash
./scripts/start-dev.sh
```

**Just run observability:**
```bash
./scripts/start-observability.sh
```

### Clean Slate

Remove all data and restart:
```bash
./scripts/stop-control-plane.sh --clean
./scripts/start-control-plane.sh
```

## Signal Handling

The control plane script handles signals gracefully:

- **Ctrl+C (SIGINT)** - Graceful shutdown
- **SIGTERM** - Graceful shutdown
- **Script exit** - Automatic cleanup

All services are stopped in the correct order:
1. Log forwarder
2. Web UI
3. Toygres server
4. Docker containers

## Process Management

PIDs are stored in `.pids/`:
- `.pids/log-forwarder.pid`
- `.pids/ui.pid`

These are automatically created and cleaned up.

## Environment Variables

Required in `.env`:
```bash
DATABASE_URL=postgresql://user:pass@localhost:5432/toygres
```

Loaded automatically from:
- `.env` - Database config
- `observability/env.local.example` - Observability config

## Logs Location

- **Backend logs**: In-memory daemon, view with `./toygres server logs`
- **Log forwarder**: Writes to Loki (view in Grafana)
- **Docker logs**: `docker compose logs`

## Health Checks

The startup script checks:
- ✅ Grafana responds at `/api/health`
- ✅ Prometheus responds at `/-/healthy`
- ✅ Loki responds at `/ready`
- ✅ Backend responds at `/health`
- ✅ Web UI responds at `/`

If any fail, you'll see warnings but the script continues.

## Comparison: Control Plane vs Individual Scripts

### `start-control-plane.sh`
- ✅ Everything in one command
- ✅ Automatic log forwarding
- ✅ Graceful Ctrl+C shutdown
- ✅ Production-like local setup
- **Use for:** Full development experience

### `start-dev.sh`
- ✅ Backend + UI only
- ❌ No observability
- **Use for:** Quick iteration on features

### `start-observability.sh`
- ✅ Just monitoring stack
- **Use for:** When backend already running

## Next Steps

- **Create an instance:** Go to http://localhost:3000
- **View metrics:** Go to http://localhost:3001 → Dashboards → Toygres Simple
- **View logs:** Go to http://localhost:3001 → Dashboards → Toygres Logs
- **Query Prometheus:** Go to http://localhost:9090

## Production Deployment

The control plane design mirrors production:
- Local: Docker Compose
- Production: Kubernetes (AKS)

Same configs, different infrastructure!

See `docs/observability-quickstart.md` for production deployment.

---

**Quick Reference:**
```bash
# Start everything
./scripts/start-control-plane.sh

# Stop everything
./scripts/stop-control-plane.sh

# Stop and clean
./scripts/stop-control-plane.sh --clean

# View logs
./toygres server logs -f
```

**That's it! Happy developing! 🚀**


