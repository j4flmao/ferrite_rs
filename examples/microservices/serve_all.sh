#!/usr/bin/env bash
set -e

DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$DIR/../.." && pwd)"
LOG_DIR="$DIR/.run-logs"
PID_DIR="$DIR/.run-pids"
SERVICES="micro-gateway micro-sqlx-products micro-sqlx-auth micro-sqlx-users micro-sqlx-orders micro-sqlx-carts"

mkdir -p "$LOG_DIR" "$PID_DIR"

echo "[serve-all] building all 6 microservices (if needed)..."
cargo build --manifest-path "$ROOT/Cargo.toml" --workspace

for svc in $SERVICES; do
  svc_dir="$DIR/$svc"
  if [ ! -f "$svc_dir/.env" ]; then
    cp "$svc_dir/.env.example" "$svc_dir/.env" 2>/dev/null || true
  fi

  bin="$ROOT/target/debug/$svc"
  if [ ! -x "$bin" ]; then
    echo "[serve-all] ERROR: $bin does not exist — build failed for $svc"
    exit 1
  fi

  (
    cd "$svc_dir"
    setsid "$bin" >> "$LOG_DIR/$svc.log" 2>&1 &
    echo $! > "$PID_DIR/$svc.pid"
  )
  echo "[serve-all] started $svc (pid $(cat "$PID_DIR/$svc.pid")) -> .run-logs/$svc.log"
done

sleep 2
echo ""
echo "[serve-all] all services launched"
echo "[serve-all] gateway :8080  products :3004  auth :3005  users :3006  orders :3007  carts :3008"
echo "[serve-all] logs: .run-logs/    stop: make stop-all"