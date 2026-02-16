#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_PID_FILE="$ROOT_DIR/.service.pid"
WEB_PID_FILE="$ROOT_DIR/.web.pid"

stop_by_pid_file() {
  local name="$1"
  local pid_file="$2"

  if [[ ! -f "$pid_file" ]]; then
    echo "[x-photo] $name pid file not found: $pid_file"
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
  fi

  rm -f "$pid_file"
}

stop_by_pid_file "service" "$SERVICE_PID_FILE"
stop_by_pid_file "web" "$WEB_PID_FILE"

echo "[x-photo] stop completed"
