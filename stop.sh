#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_PID_FILE="$ROOT_DIR/.service.pid"
WEB_PID_FILE="$ROOT_DIR/.web.pid"
SERVICE_PORT="${SERVICE_PORT:-55080}"
WEB_PORT="${WEB_PORT:-55081}"

show_port_usage() {
  local name="$1"
  local port="$2"

  if command -v ss >/dev/null 2>&1; then
    local output
    output="$(ss -ltnp "( sport = :$port )" 2>/dev/null || true)"
    if printf '%s\n' "$output" | awk 'NR > 1 { found = 1 } END { exit !found }'; then
      echo "[x-photo] $name port $port is still in use:"
      printf '%s\n' "$output"
      return 0
    fi
  elif command -v lsof >/dev/null 2>&1; then
    local output
    output="$(lsof -nP -iTCP:"$port" -sTCP:LISTEN 2>/dev/null || true)"
    if [[ -n "$output" ]]; then
      echo "[x-photo] $name port $port is still in use:"
      printf '%s\n' "$output"
      return 0
    fi
  fi

  return 1
}

stop_by_pid_file() {
  local name="$1"
  local pid_file="$2"
  local port="$3"

  if [[ ! -f "$pid_file" ]]; then
    echo "[x-photo] $name pid file not found: $pid_file"
    if show_port_usage "$name" "$port"; then
      echo "[x-photo] not stopping port $port without a pid file proving x-photo ownership"
    fi
    return
  fi

  local pid
  pid="$(cat "$pid_file" 2>/dev/null || true)"
  if [[ -z "$pid" ]]; then
    echo "[x-photo] $name pid file is empty: $pid_file"
    rm -f "$pid_file"
    return
  fi

  if kill -0 "$pid" >/dev/null 2>&1; then
    kill "$pid" >/dev/null 2>&1 || true
    echo "[x-photo] stopped $name (pid=$pid)"
  else
    echo "[x-photo] $name already stopped (pid=$pid)"
    show_port_usage "$name" "$port" || true
  fi

  rm -f "$pid_file"
}

stop_by_pid_file "service" "$SERVICE_PID_FILE" "$SERVICE_PORT"
stop_by_pid_file "web" "$WEB_PID_FILE" "$WEB_PORT"

echo "[x-photo] stop completed"
