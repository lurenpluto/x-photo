#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
SERVICE_DIR="$REPO_DIR/service"

DATA_ROOT="${XPHOTO_STRESS_DATA_ROOT:-/home/bucky/.xphoto/test_data}"
PROFILE="${XPHOTO_STRESS_PROFILE:-stress}"
MODE="single"
REPORT_DIR="$REPO_DIR/doc/reports"
LABEL="$(date +%Y%m%d-%H%M%S)"
BASE_URL="${BASE_URL:-http://127.0.0.1:58090/rpc/v1}"
BIND_ADDR="${BIND_ADDR:-127.0.0.1:58090}"
TIMEOUT_SEC="${TIMEOUT_SEC:-3600}"
START_SERVICE=1
MAX_SCENARIOS=0

usage() {
  cat <<'EOF'
Usage: service/scripts/scan_stress_test.sh [options]

Options:
  --data-root <dir>       Test dataset root. Default: /home/bucky/.xphoto/test_data
  --profile <name>        Dataset summary profile. Default: stress
  --mode <name>           single | scenarios. Default: single
  --report-dir <dir>      Report output dir. Default: doc/reports
  --label <name>          Report/run label. Default: timestamp
  --timeout-sec <n>       Scan polling timeout. Default: 3600
  --max-scenarios <n>     In scenarios mode, only scan first n scenarios. 0 = all.
  --start-service         Start isolated local service. Default.
  --no-start-service      Use BASE_URL instead of starting a service.
  --bind-addr <addr>      Bind addr when starting service. Default: 127.0.0.1:58090
  --base-url <url>        API base URL. Default: http://127.0.0.1:58090/rpc/v1
  -h, --help              Show this help.

Useful env overrides for started service:
  SCAN_MAX_CONCURRENT_JOBS, SCAN_HASH_PARALLELISM, SCAN_HASH_BATCH_SIZE,
  SCAN_CHECKPOINT_EVERY, SCAN_SEARCH_INDEX_SYNC_EVERY
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --data-root)
      [[ $# -ge 2 ]] || { echo "--data-root requires a value" >&2; exit 1; }
      DATA_ROOT="$2"
      shift 2
      ;;
    --profile)
      [[ $# -ge 2 ]] || { echo "--profile requires a value" >&2; exit 1; }
      PROFILE="$2"
      shift 2
      ;;
    --mode)
      [[ $# -ge 2 ]] || { echo "--mode requires a value" >&2; exit 1; }
      MODE="$2"
      shift 2
      ;;
    --report-dir)
      [[ $# -ge 2 ]] || { echo "--report-dir requires a value" >&2; exit 1; }
      REPORT_DIR="$2"
      shift 2
      ;;
    --label)
      [[ $# -ge 2 ]] || { echo "--label requires a value" >&2; exit 1; }
      LABEL="$2"
      shift 2
      ;;
    --timeout-sec)
      [[ $# -ge 2 ]] || { echo "--timeout-sec requires a value" >&2; exit 1; }
      TIMEOUT_SEC="$2"
      shift 2
      ;;
    --max-scenarios)
      [[ $# -ge 2 ]] || { echo "--max-scenarios requires a value" >&2; exit 1; }
      MAX_SCENARIOS="$2"
      shift 2
      ;;
    --start-service)
      START_SERVICE=1
      shift
      ;;
    --no-start-service)
      START_SERVICE=0
      shift
      ;;
    --bind-addr)
      [[ $# -ge 2 ]] || { echo "--bind-addr requires a value" >&2; exit 1; }
      BIND_ADDR="$2"
      BASE_URL="http://$BIND_ADDR/rpc/v1"
      shift 2
      ;;
    --base-url)
      [[ $# -ge 2 ]] || { echo "--base-url requires a value" >&2; exit 1; }
      BASE_URL="$2"
      START_SERVICE=0
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

case "$MODE" in
  single|scenarios) ;;
  *)
    echo "unknown mode: $MODE" >&2
    usage >&2
    exit 1
    ;;
esac

SUMMARY_PATH="$DATA_ROOT/_manifests/_summary_${PROFILE}.json"
if [[ ! -f "$SUMMARY_PATH" ]]; then
  echo "dataset summary not found: $SUMMARY_PATH" >&2
  exit 1
fi

mkdir -p "$REPORT_DIR"
RUN_DIR="$(mktemp -d "/tmp/xphoto_scan_stress_${LABEL}.XXXXXX")"
RESP_DIR="$RUN_DIR/responses"
mkdir -p "$RESP_DIR"
SERVICE_PID=""
DB_PATH=""
LOG_DIR=""

cleanup() {
  if [[ -n "$SERVICE_PID" ]] && kill -0 "$SERVICE_PID" 2>/dev/null; then
    kill "$SERVICE_PID" 2>/dev/null || true
    wait "$SERVICE_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

json_get() {
  local path="$1"
  python3 -c 'import json,sys
path=sys.argv[1]
obj=json.load(sys.stdin)
cur=obj
for p in path.split("."):
    if not p:
        continue
    cur = cur[int(p)] if p.isdigit() else cur[p]
print(json.dumps(cur, ensure_ascii=False) if isinstance(cur, (dict, list)) else cur)' "$path"
}

call_api() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  local url="$BASE_URL$path"
  if [[ -n "$body" ]]; then
    curl -sS -X "$method" "$url" -H "content-type: application/json" -d "$body"
  else
    curl -sS -X "$method" "$url"
  fi
}

json_source_body() {
  python3 - "$1" "$2" <<'PY'
import json
import sys
print(json.dumps({"name": sys.argv[1], "root_path": sys.argv[2], "source_type": "local_fs"}, ensure_ascii=False))
PY
}

start_service_if_needed() {
  if [[ "$START_SERVICE" != "1" ]]; then
    return
  fi

  DB_PATH="$RUN_DIR/stress.db"
  LOG_DIR="$RUN_DIR/logs"
  mkdir -p "$LOG_DIR"
  cp "$REPO_DIR/doc/config.example.toml" "$RUN_DIR/config.toml"

  echo "starting isolated service at $BIND_ADDR"
  (
    cd "$SERVICE_DIR"
    CONFIG_PATH="$RUN_DIR/config.toml" \
    BIND_ADDR="$BIND_ADDR" \
    DATABASE_URL="sqlite://$DB_PATH" \
    LOG_DIR="$LOG_DIR" \
    SCAN_SOURCE_CHANGE_DETECT_ENABLED=false \
    cargo run --bin service
  ) >"$RUN_DIR/service.stdout.log" 2>"$RUN_DIR/service.stderr.log" &
  SERVICE_PID="$!"

  for _ in $(seq 1 60); do
    if call_api GET "/health" >/dev/null 2>&1; then
      local health
      health="$(call_api GET "/health")"
      if [[ "$(printf '%s' "$health" | json_get code)" == "0" ]]; then
        return
      fi
    fi
    sleep 1
  done

  echo "service did not become healthy; logs: $RUN_DIR" >&2
  exit 1
}

build_source_plan() {
  python3 - "$SUMMARY_PATH" "$DATA_ROOT" "$MODE" "$MAX_SCENARIOS" <<'PY'
import json
import os
import sys

summary_path, data_root, mode, max_scenarios = sys.argv[1:]
max_scenarios = int(max_scenarios)
with open(summary_path, encoding="utf-8") as f:
    summary = json.load(f)

if mode == "single":
    print("|".join(["all", data_root, str(summary["total_photos"]), str(summary["total_albums"])]))
else:
    scenarios = summary["scenarios"]
    if max_scenarios > 0:
        scenarios = scenarios[:max_scenarios]
    for item in scenarios:
        print("|".join([
            item["name"],
            os.path.join(data_root, item["name"]),
            str(item["total_photos"]),
            str(item["album_count"]),
        ]))
PY
}

start_service_if_needed

mapfile -t source_plan < <(build_source_plan)
if [[ "${#source_plan[@]}" -eq 0 ]]; then
  echo "empty source plan" >&2
  exit 1
fi

expected_photos=0
expected_albums=0
source_names=()
source_ids=()
job_ids=()
source_expected_photos=()

run_started_epoch="$(date +%s)"

for line in "${source_plan[@]}"; do
  IFS='|' read -r scenario root expected_photo_count expected_album_count <<<"$line"
  expected_photos=$((expected_photos + expected_photo_count))
  expected_albums=$((expected_albums + expected_album_count))
  source_names+=("$scenario")
  source_expected_photos+=("$expected_photo_count")

  body="$(json_source_body "stress-${LABEL}-${scenario}" "$root")"
  resp="$(call_api POST "/sources" "$body")"
  printf '%s' "$resp" >"$RESP_DIR/create_source_${scenario}.json"
  code="$(printf '%s' "$resp" | json_get code)"
  if [[ "$code" != "0" ]]; then
    echo "create source failed for $scenario: $resp" >&2
    exit 1
  fi
  source_id="$(printf '%s' "$resp" | json_get data.id)"
  source_ids+=("$source_id")

  trigger_resp="$(call_api POST "/sources/$source_id/scan" "{}")"
  printf '%s' "$trigger_resp" >"$RESP_DIR/trigger_${scenario}.json"
  trigger_code="$(printf '%s' "$trigger_resp" | json_get code)"
  if [[ "$trigger_code" != "0" ]]; then
    echo "trigger scan failed for $scenario: $trigger_resp" >&2
    exit 1
  fi
  job_ids+=("$(printf '%s' "$trigger_resp" | json_get data.job_id)")
done

echo "triggered ${#job_ids[@]} scan job(s), expected_photos=$expected_photos"

declare -A job_done=()
start_ts="$(date +%s)"
while true; do
  done_count=0
  for idx in "${!job_ids[@]}"; do
    job_id="${job_ids[$idx]}"
    if [[ "${job_done[$job_id]:-0}" == "1" ]]; then
      done_count=$((done_count + 1))
      continue
    fi
    status_resp="$(call_api GET "/scan-jobs/$job_id")"
    printf '%s' "$status_resp" >"$RESP_DIR/job_${source_names[$idx]}.json"
    status="$(printf '%s' "$status_resp" | json_get data.status)"
    if [[ "$status" == "success" ]]; then
      job_done[$job_id]=1
      done_count=$((done_count + 1))
    elif [[ "$status" == "failed" || "$status" == "cancelled" ]]; then
      echo "job $job_id ended with $status: $status_resp" >&2
      job_done[$job_id]=1
      done_count=$((done_count + 1))
    fi
  done

  if [[ "$done_count" -eq "${#job_ids[@]}" ]]; then
    break
  fi

  now_ts="$(date +%s)"
  if (( now_ts - start_ts > TIMEOUT_SEC )); then
    echo "scan stress timeout after ${TIMEOUT_SEC}s" >&2
    break
  fi
  sleep 2
done

run_finished_epoch="$(date +%s)"
elapsed_sec=$((run_finished_epoch - run_started_epoch))

search_resp="$(call_api POST "/photos/search" '{"page":1,"page_size":1}')"
printf '%s' "$search_resp" >"$RESP_DIR/search_total.json"

db_metrics_path="$RESP_DIR/db_metrics.json"
if [[ -n "$DB_PATH" && -f "$DB_PATH" ]]; then
  python3 - "$DB_PATH" >"$db_metrics_path" <<'PY'
import json
import sqlite3
import sys

db_path = sys.argv[1]
conn = sqlite3.connect(db_path)
cur = conn.cursor()

def scalar(sql):
    return cur.execute(sql).fetchone()[0]

metrics = {
    "sources": scalar("SELECT COUNT(1) FROM sources"),
    "active_photos": scalar("SELECT COUNT(1) FROM photos WHERE deleted_at IS NULL"),
    "deleted_photos": scalar("SELECT COUNT(1) FROM photos WHERE deleted_at IS NOT NULL"),
    "albums": scalar("SELECT COUNT(1) FROM albums"),
    "photo_albums": scalar("SELECT COUNT(1) FROM photo_albums"),
    "favorites": scalar("SELECT COUNT(1) FROM photo_favorites"),
    "fts_rows": scalar("SELECT COUNT(1) FROM photo_search_fts"),
    "duplicate_storage_groups": scalar("""
        SELECT COUNT(1)
        FROM (
            SELECT source_id, storage_file_id, COUNT(1) AS c
            FROM photos
            GROUP BY source_id, storage_file_id
            HAVING c > 1
        )
    """),
}
print(json.dumps(metrics, ensure_ascii=False, indent=2))
PY
else
  printf '{}\n' >"$db_metrics_path"
fi

report_json="$REPORT_DIR/scan_stress_${LABEL}.json"
report_md="$REPORT_DIR/scan_stress_${LABEL}.md"

python3 - \
  "$report_json" \
  "$report_md" \
  "$SUMMARY_PATH" \
  "$RESP_DIR" \
  "$db_metrics_path" \
  "$DATA_ROOT" \
  "$PROFILE" \
  "$MODE" \
  "$LABEL" \
  "$BASE_URL" \
  "$expected_photos" \
  "$expected_albums" \
  "$elapsed_sec" \
  "$DB_PATH" \
  "$LOG_DIR" \
  "${source_names[@]}" \
  -- \
  "${source_ids[@]}" \
  -- \
  "${job_ids[@]}" <<'PY'
import json
import os
import sys
from datetime import datetime, timezone

args = sys.argv[1:]
sep1 = args.index("--")
sep2 = args.index("--", sep1 + 1)
fixed = args[:sep1]
source_names = fixed[15:]
source_ids = args[sep1 + 1:sep2]
job_ids = args[sep2 + 1:]
(
    report_json,
    report_md,
    summary_path,
    resp_dir,
    db_metrics_path,
    data_root,
    profile,
    mode,
    label,
    base_url,
    expected_photos,
    expected_albums,
    elapsed_sec,
    db_path,
    log_dir,
) = fixed[:15]
expected_photos = int(expected_photos)
expected_albums = int(expected_albums)
elapsed_sec = int(elapsed_sec)

def read_json(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)

summary = read_json(summary_path)
db_metrics = read_json(db_metrics_path)
search_total = read_json(os.path.join(resp_dir, "search_total.json"))

jobs = []
all_jobs_success = True
failed_count_sum = 0
new_count_sum = 0
updated_count_sum = 0
processed_count_sum = 0
for name, source_id, job_id in zip(source_names, source_ids, job_ids):
    path = os.path.join(resp_dir, f"job_{name}.json")
    job_resp = read_json(path)
    data = job_resp.get("data", {})
    status = data.get("status")
    if status != "success":
        all_jobs_success = False
    failed_count_sum += data.get("failed_count") or 0
    new_count_sum += data.get("new_count") or 0
    updated_count_sum += data.get("updated_count") or 0
    processed_count_sum += data.get("processed_count") or 0
    jobs.append({
        "scenario": name,
        "source_id": source_id,
        "job_id": job_id,
        "status": status,
        "processed_count": data.get("processed_count"),
        "total_count": data.get("total_count"),
        "new_count": data.get("new_count"),
        "updated_count": data.get("updated_count"),
        "failed_count": data.get("failed_count"),
        "error_message": data.get("error_message"),
        "started_at": data.get("started_at"),
        "finished_at": data.get("finished_at"),
    })

api_total = search_total.get("data", {}).get("total")
active_photos = db_metrics.get("active_photos")
fts_rows = db_metrics.get("fts_rows")

checks = {
    "all_jobs_success": all_jobs_success,
    "job_failed_count_zero": failed_count_sum == 0,
    "api_search_total_matches_expected": api_total == expected_photos,
    "db_active_photo_count_matches_expected": active_photos in (None, expected_photos),
    "fts_rows_match_active_photos": (
        fts_rows is None or active_photos is None or fts_rows == active_photos
    ),
    "duplicate_storage_groups_zero": db_metrics.get("duplicate_storage_groups", 0) == 0,
}
passed = all(checks.values())

report = {
    "label": label,
    "generated_at": datetime.now(timezone.utc).isoformat(),
    "data_root": data_root,
    "profile": profile,
    "mode": mode,
    "base_url": base_url,
    "elapsed_sec": elapsed_sec,
    "expected": {
        "photos": expected_photos,
        "albums_manifest_sum": expected_albums,
        "summary": summary_path,
    },
    "actual": {
        "api_search_total": api_total,
        "job_failed_count_sum": failed_count_sum,
        "job_new_count_sum": new_count_sum,
        "job_updated_count_sum": updated_count_sum,
        "job_processed_count_sum": processed_count_sum,
        "db": db_metrics,
    },
    "checks": checks,
    "passed": passed,
    "db_path": db_path or None,
    "log_dir": log_dir or None,
    "jobs": jobs,
}

with open(report_json, "w", encoding="utf-8") as f:
    json.dump(report, f, ensure_ascii=False, indent=2)

lines = [
    f"# Scan Stress Report: {label}",
    "",
    f"- Passed: `{passed}`",
    f"- Profile: `{profile}`",
    f"- Mode: `{mode}`",
    f"- Data root: `{data_root}`",
    f"- Elapsed: `{elapsed_sec}s`",
    f"- Expected photos: `{expected_photos}`",
    f"- API search total: `{api_total}`",
    f"- DB active photos: `{active_photos}`",
    f"- FTS rows: `{fts_rows}`",
    f"- Job failed count sum: `{failed_count_sum}`",
    f"- DB path: `{db_path or ''}`",
    f"- Log dir: `{log_dir or ''}`",
    "",
    "## Correctness Checks",
    "",
    "| Check | Result |",
    "| --- | --- |",
]
for key, value in checks.items():
    lines.append(f"| `{key}` | `{value}` |")
lines.extend([
    "",
    "## Jobs",
    "",
    "| Scenario | Status | Total | Processed | New | Updated | Failed |",
    "| --- | --- | ---: | ---: | ---: | ---: | ---: |",
])
for job in jobs:
    lines.append(
        f"| `{job['scenario']}` | `{job['status']}` | `{job['total_count']}` | "
        f"`{job['processed_count']}` | `{job['new_count']}` | "
        f"`{job['updated_count']}` | `{job['failed_count']}` |"
    )

with open(report_md, "w", encoding="utf-8") as f:
    f.write("\n".join(lines) + "\n")

print(json.dumps({"passed": passed, "report_json": report_json, "report_md": report_md}, ensure_ascii=False, indent=2))
if not passed:
    sys.exit(2)
PY

