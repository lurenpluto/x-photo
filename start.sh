#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_DIR="$ROOT_DIR/service"
WEB_DIR="$ROOT_DIR/web"

SERVICE_HOST="${SERVICE_HOST:-127.0.0.1}"
SERVICE_PORT="${SERVICE_PORT:-55080}"
WEB_HOST="${WEB_HOST:-127.0.0.1}"
WEB_PORT="${WEB_PORT:-55081}"

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

SERVICE_PID=""
WEB_PID=""
SERVICE_PID_FILE="$ROOT_DIR/.service.pid"
WEB_PID_FILE="$ROOT_DIR/.web.pid"
SERVICE_LOG="$ROOT_DIR/.service.log"
WEB_LOG="$ROOT_DIR/.web.log"

show_port_usage() {
  local port="$1"
  if command -v ss >/dev/null 2>&1; then
    ss -ltnp "( sport = :$port )" 2>/dev/null || true
  elif command -v lsof >/dev/null 2>&1; then
    lsof -nP -iTCP:"$port" -sTCP:LISTEN 2>/dev/null || true
  else
    echo "[x-photo] install ss or lsof to inspect the process using port $port"
  fi
}

ensure_port_available() {
  local name="$1"
  local host="$2"
  local port="$3"

  if ! "$PYTHON_CMD" - "$host" "$port" <<'PY'
import socket
import sys

host = sys.argv[1]
port = int(sys.argv[2])
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
try:
    sock.bind((host, port))
except OSError:
    sys.exit(1)
finally:
    sock.close()
PY
  then
    echo "[x-photo] $name port is already in use: $host:$port"
    echo "[x-photo] Either stop that process or choose another port, for example:"
    if [[ "$name" == "service" ]]; then
      echo "  SERVICE_PORT=55082 ./start.sh"
    else
      echo "  WEB_PORT=55083 ./start.sh"
    fi
    echo "[x-photo] ---- listeners on port $port ----"
    show_port_usage "$port"
    exit 1
  fi
}

show_log_tail() {
  local log_file="$1"
  local label="$2"
  if [[ -f "$log_file" ]]; then
    echo "[x-photo] ---- $label (last 25 lines) ----"
    tail -n 25 "$log_file"
  fi
}

assert_alive() {
  local pid="$1"
  local name="$2"
  local log_file="$3"
  if ! kill -0 "$pid" >/dev/null 2>&1; then
    echo "[x-photo] $name failed shortly after startup."
    echo "[x-photo] Common reason: port already in use."
    show_log_tail "$log_file" "$name log"
    exit 1
  fi
}

cleanup() {
  if [[ -n "$SERVICE_PID" ]] && kill -0 "$SERVICE_PID" >/dev/null 2>&1; then
    kill "$SERVICE_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "$WEB_PID" ]] && kill -0 "$WEB_PID" >/dev/null 2>&1; then
    kill "$WEB_PID" >/dev/null 2>&1 || true
  fi
  rm -f "$SERVICE_PID_FILE" "$WEB_PID_FILE"
}

trap cleanup EXIT INT TERM

ensure_port_available "service" "$SERVICE_HOST" "$SERVICE_PORT"
ensure_port_available "web" "$WEB_HOST" "$WEB_PORT"

echo "[x-photo] Building service..."
cargo build --manifest-path "$SERVICE_DIR/Cargo.toml"

echo "[x-photo] Starting service on http://$SERVICE_HOST:$SERVICE_PORT ..."
BIND_ADDR="$SERVICE_HOST:$SERVICE_PORT" cargo run --manifest-path "$SERVICE_DIR/Cargo.toml" --bin service >"$SERVICE_LOG" 2>&1 &
SERVICE_PID=$!
printf "%s" "$SERVICE_PID" >"$SERVICE_PID_FILE"

echo "[x-photo] Starting web on http://$WEB_HOST:$WEB_PORT ..."
"$PYTHON_CMD" -m http.server "$WEB_PORT" --bind "$WEB_HOST" --directory "$WEB_DIR" >"$WEB_LOG" 2>&1 &
WEB_PID=$!
printf "%s" "$WEB_PID" >"$WEB_PID_FILE"

sleep 1
assert_alive "$SERVICE_PID" "service" "$SERVICE_LOG"
assert_alive "$WEB_PID" "web" "$WEB_LOG"

echo "[x-photo] Ready"
echo "  - Web: http://$WEB_HOST:$WEB_PORT"
echo "  - API: http://$SERVICE_HOST:$SERVICE_PORT/rpc/v1"
echo "[x-photo] Logs: $SERVICE_LOG, $WEB_LOG"
echo "[x-photo] Pid files: $SERVICE_PID_FILE, $WEB_PID_FILE"
echo "[x-photo] Press Ctrl+C to stop both services."

wait "$SERVICE_PID" "$WEB_PID"
