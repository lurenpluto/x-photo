#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_DIR="$ROOT_DIR/service"
WEB_DIR="$ROOT_DIR/web"

SERVICE_HOST="${SERVICE_HOST:-127.0.0.1}"
SERVICE_PORT="${SERVICE_PORT:-8080}"
WEB_HOST="${WEB_HOST:-127.0.0.1}"
WEB_PORT="${WEB_PORT:-5174}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[x-photo] cargo not found. Please install Rust toolchain first."
  exit 1
fi

if command -v python3 >/dev/null 2>&1; then
  PYTHON_CMD="python3"
elif command -v python >/dev/null 2>&1; then
  PYTHON_CMD="python"
else
  echo "[x-photo] python/python3 not found. Please install Python first."
  exit 1
fi

echo "[x-photo] Building service..."
cargo build --manifest-path "$SERVICE_DIR/Cargo.toml"

SERVICE_PID=""
WEB_PID=""

cleanup() {
  if [[ -n "$SERVICE_PID" ]] && kill -0 "$SERVICE_PID" >/dev/null 2>&1; then
    kill "$SERVICE_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "$WEB_PID" ]] && kill -0 "$WEB_PID" >/dev/null 2>&1; then
    kill "$WEB_PID" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT INT TERM

echo "[x-photo] Starting service on http://$SERVICE_HOST:$SERVICE_PORT ..."
BIND_ADDR="$SERVICE_HOST:$SERVICE_PORT" cargo run --manifest-path "$SERVICE_DIR/Cargo.toml" >"$ROOT_DIR/.service.log" 2>&1 &
SERVICE_PID=$!

echo "[x-photo] Starting web on http://$WEB_HOST:$WEB_PORT ..."
"$PYTHON_CMD" -m http.server "$WEB_PORT" --bind "$WEB_HOST" --directory "$WEB_DIR" >"$ROOT_DIR/.web.log" 2>&1 &
WEB_PID=$!

echo "[x-photo] Ready"
echo "  - Web: http://$WEB_HOST:$WEB_PORT"
echo "  - API: http://$SERVICE_HOST:$SERVICE_PORT/rpc/v1"
echo "[x-photo] Logs: $ROOT_DIR/.service.log, $ROOT_DIR/.web.log"
echo "[x-photo] Press Ctrl+C to stop both services."

wait "$SERVICE_PID" "$WEB_PID"
