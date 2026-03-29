#!/bin/bash
set -e

ROOT="$(cd "$(dirname "$0")" && pwd)"

echo "→ Starting Redis..."
redis-server --daemonize yes 2>/dev/null || echo "  Redis already running"

echo "→ Starting backend..."
cd "$ROOT/backend"
~/.cargo/bin/cargo run &
BACKEND_PID=$!

echo "→ Starting frontend..."
cd "$ROOT/frontend"
npm run dev &
FRONTEND_PID=$!

echo "→ Starting Cloudflare tunnel..."
cloudflared tunnel --config ~/.cloudflared/config.yml run hlv &
TUNNEL_PID=$!

echo ""
echo "  local:   http://localhost:5173"
echo "  network: https://hlv.bavardage.org"
echo ""
echo "Press Ctrl+C to stop all services."

trap "echo ''; echo 'Stopping...'; kill $BACKEND_PID $FRONTEND_PID $TUNNEL_PID 2>/dev/null; redis-cli shutdown 2>/dev/null; exit 0" INT

wait
