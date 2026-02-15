#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8080/rpc/v1}"
DATA_DIR="${DATA_DIR:-/tmp/xphoto_test_data}"
TIMEOUT_SEC="${TIMEOUT_SEC:-120}"

json_get() {
  local path="$1"
  python3 - "$path" <<'PY'
import json
import sys

path = sys.argv[1]
obj = json.load(sys.stdin)
cur = obj
for p in path.split('.'):
    if p == '':
        continue
    if p.isdigit():
        cur = cur[int(p)]
    else:
        cur = cur[p]
if isinstance(cur, (dict, list)):
    print(json.dumps(cur, ensure_ascii=False))
else:
    print(cur)
PY
}

call_api() {
  local method="$1"
  local url="$2"
  local body="${3:-}"
  if [[ -n "$body" ]]; then
    curl -sS -X "$method" "$url" -H "content-type: application/json" -d "$body"
  else
    curl -sS -X "$method" "$url"
  fi
}

echo "[1/8] health check"
health_resp="$(call_api GET "$BASE_URL/health")"
health_code="$(printf '%s' "$health_resp" | json_get code)"
if [[ "$health_code" != "0" ]]; then
  echo "health failed: $health_resp"
  exit 1
fi

echo "[2/8] create source"
create_source_body="{\"name\":\"smoke-local\",\"root_path\":\"$DATA_DIR\",\"source_type\":\"local_fs\"}"
create_source_resp="$(call_api POST "$BASE_URL/sources" "$create_source_body")"
create_source_code="$(printf '%s' "$create_source_resp" | json_get code)"
if [[ "$create_source_code" != "0" ]]; then
  echo "create source failed: $create_source_resp"
  exit 1
fi
source_id="$(printf '%s' "$create_source_resp" | json_get data.id)"

echo "[3/8] trigger scan"
scan_resp="$(call_api POST "$BASE_URL/sources/$source_id/scan" "{}")"
scan_code="$(printf '%s' "$scan_resp" | json_get code)"
if [[ "$scan_code" != "0" ]]; then
  echo "trigger scan failed: $scan_resp"
  exit 1
fi
scan_job_id="$(printf '%s' "$scan_resp" | json_get data.job_id)"

echo "[4/8] poll scan job status"
start_ts="$(date +%s)"
while true; do
  status_resp="$(call_api GET "$BASE_URL/scan-jobs/$scan_job_id")"
  status="$(printf '%s' "$status_resp" | json_get data.status)"
  if [[ "$status" == "success" ]]; then
    break
  fi
  if [[ "$status" == "failed" || "$status" == "cancelled" ]]; then
    echo "scan job ended with $status: $status_resp"
    exit 1
  fi
  now_ts="$(date +%s)"
  if (( now_ts - start_ts > TIMEOUT_SEC )); then
    echo "scan polling timeout"
    exit 1
  fi
  sleep 1
done

echo "[5/8] search photos"
search_resp="$(call_api POST "$BASE_URL/photos/search" '{"page":1,"page_size":20}')"
search_code="$(printf '%s' "$search_resp" | json_get code)"
if [[ "$search_code" != "0" ]]; then
  echo "search failed: $search_resp"
  exit 1
fi
total="$(printf '%s' "$search_resp" | json_get data.total)"
if (( total <= 0 )); then
  echo "search returned empty photos: $search_resp"
  exit 1
fi

echo "[6/8] check task overview"
overview_resp="$(call_api GET "$BASE_URL/task-jobs/overview")"
overview_code="$(printf '%s' "$overview_resp" | json_get code)"
if [[ "$overview_code" != "0" ]]; then
  echo "task overview failed: $overview_resp"
  exit 1
fi

echo "[7/8] trigger fs_watch scan"
fs_watch_resp="$(call_api POST "$BASE_URL/sources/$source_id/scan:fs-watch" '{"changed_paths":["/tmp/xphoto_test_data"]}')"
fs_watch_code="$(printf '%s' "$fs_watch_resp" | json_get code)"
if [[ "$fs_watch_code" != "0" ]]; then
  echo "fs_watch trigger failed: $fs_watch_resp"
  exit 1
fi

echo "[8/8] query albums"
albums_resp="$(call_api GET "$BASE_URL/albums")"
albums_code="$(printf '%s' "$albums_resp" | json_get code)"
if [[ "$albums_code" != "0" ]]; then
  echo "albums query failed: $albums_resp"
  exit 1
fi

echo "smoke test passed: source_id=$source_id scan_job_id=$scan_job_id photos_total=$total"
