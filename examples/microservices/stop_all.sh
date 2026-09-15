#!/usr/bin/env bash
set -e

DIR="$(cd "$(dirname "$0")" && pwd)"
PID_DIR="$DIR/.run-pids"

stopped=0
for f in "$PID_DIR"/*.pid; do
  [ -e "$f" ] || continue
  pid="$(cat "$f")"
  if kill "$pid" 2>/dev/null; then
    stopped=$((stopped + 1))
  fi
  rm -f "$f"
done

echo "[stop-all] stopped $stopped service(s)."