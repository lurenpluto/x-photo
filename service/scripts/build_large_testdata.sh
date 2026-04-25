#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

OUTPUT_DIR="${XPHOTO_TESTDATA_DIR:-/tmp/xphoto_large_test_data}"
JOBS="${XPHOTO_TESTDATA_JOBS:-0}"
PROFILE="large"
CLEAN=0
FORCE=0

usage() {
  cat <<'EOF'
Usage: service/scripts/build_large_testdata.sh [options]

Options:
  --output <dir>       Output dataset root. Default: /tmp/xphoto_large_test_data
  --profile <name>     sample | medium | large | stress. Default: large
  --jobs <n>           Per-scenario image generation threads. 0 = auto. Default: 0
  --clean              Remove output dir before generating.
  --force              Regenerate scenario dirs even when manifests already exist.
  -h, --help           Show this help.

Profiles:
  sample   1 photo, fastest smoke fixture.
  medium   sample + small/medium family datasets, suitable for quick local checks.
  large    multi-year family datasets, suitable for reusable manual stress runs.
  stress   larger multi-year dataset for scan concurrency and stability testing.

Generated layout:
  <output>/
    _manifests/
      <scenario>.json
      _summary_<profile>.json
    <scenario>/
      <album-dir>/
        *.jpg
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      [[ $# -ge 2 ]] || { echo "--output requires a value" >&2; exit 1; }
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --profile)
      [[ $# -ge 2 ]] || { echo "--profile requires a value" >&2; exit 1; }
      PROFILE="$2"
      shift 2
      ;;
    --jobs)
      [[ $# -ge 2 ]] || { echo "--jobs requires a value" >&2; exit 1; }
      JOBS="$2"
      shift 2
      ;;
    --clean)
      CLEAN=1
      shift
      ;;
    --force)
      FORCE=1
      shift
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

case "$PROFILE" in
  sample|medium|large|stress) ;;
  *)
    echo "unknown profile: $PROFILE" >&2
    usage >&2
    exit 1
    ;;
esac

if [[ "$CLEAN" == "1" ]]; then
  rm -rf "$OUTPUT_DIR"
fi

mkdir -p "$OUTPUT_DIR/_manifests"

scenario_lines=()
case "$PROFILE" in
  sample)
    scenario_lines+=("sample|sample|2025|42")
    ;;
  medium)
    scenario_lines+=("sample|sample|2025|42")
    scenario_lines+=("family-small-2025|family_us_weekends_small|2025|42")
    scenario_lines+=("family-medium-2024|family_us_weekends_medium|2024|4242")
    scenario_lines+=("family-medium-2025|family_us_weekends_medium|2025|5252")
    ;;
  large)
    scenario_lines+=("sample|sample|2025|42")
    scenario_lines+=("family-small-2025|family_us_weekends_small|2025|42")
    scenario_lines+=("family-medium-2024|family_us_weekends_medium|2024|4242")
    scenario_lines+=("family-full-2023|family_us_weekends_2025|2023|23042")
    scenario_lines+=("family-full-2024|family_us_weekends_2025|2024|24042")
    scenario_lines+=("family-full-2025|family_us_weekends_2025|2025|25042")
    ;;
  stress)
    scenario_lines+=("sample|sample|2025|42")
    scenario_lines+=("family-medium-2024|family_us_weekends_medium|2024|4242")
    scenario_lines+=("family-full-2020|family_us_weekends_2025|2020|20042")
    scenario_lines+=("family-full-2021|family_us_weekends_2025|2021|21042")
    scenario_lines+=("family-full-2022|family_us_weekends_2025|2022|22042")
    scenario_lines+=("family-full-2023|family_us_weekends_2025|2023|23042")
    scenario_lines+=("family-full-2024|family_us_weekends_2025|2024|24042")
    scenario_lines+=("family-full-2025|family_us_weekends_2025|2025|25042")
    ;;
esac

manifest_paths=()
for line in "${scenario_lines[@]}"; do
  IFS='|' read -r scenario strategy year seed <<<"$line"
  scenario_dir="$OUTPUT_DIR/$scenario"
  manifest_path="$OUTPUT_DIR/_manifests/$scenario.json"
  manifest_paths+=("$manifest_path")

  if [[ "$FORCE" == "0" && -f "$manifest_path" && -d "$scenario_dir" ]]; then
    echo "skip existing scenario: $scenario ($manifest_path)"
    continue
  fi

  args=(
    run --bin testdata_builder --
    "$scenario_dir"
    --strategy "$strategy"
    --year "$year"
    --seed "$seed"
    --jobs "$JOBS"
    --manifest "$manifest_path"
  )
  if [[ "$FORCE" == "1" ]]; then
    args+=(--clean)
  fi

  echo "generate scenario: $scenario strategy=$strategy year=$year seed=$seed"
  (cd "$SERVICE_DIR" && cargo "${args[@]}")
done

summary_path="$OUTPUT_DIR/_manifests/_summary_${PROFILE}.json"
python3 - "$OUTPUT_DIR" "$PROFILE" "$JOBS" "$summary_path" "${manifest_paths[@]}" <<'PY'
import json
import os
import sys
from datetime import datetime, timezone

output_dir, profile, jobs, summary_path, *manifest_paths = sys.argv[1:]
scenarios = []
total_photos = 0
total_albums = 0
missing = []

for path in manifest_paths:
    if not os.path.exists(path):
        missing.append(path)
        continue
    with open(path, "r", encoding="utf-8") as f:
        manifest = json.load(f)
    scenarios.append({
        "name": os.path.splitext(os.path.basename(path))[0],
        "strategy": manifest["strategy"],
        "year": manifest["year"],
        "seed": manifest["seed"],
        "total_photos": manifest["total_photos"],
        "album_count": len(manifest.get("albums", [])),
        "manifest": path,
    })
    total_photos += manifest["total_photos"]
    total_albums += len(manifest.get("albums", []))

summary = {
    "profile": profile,
    "jobs": jobs,
    "output_dir": output_dir,
    "generated_at": datetime.now(timezone.utc).isoformat(),
    "total_photos": total_photos,
    "total_albums": total_albums,
    "scenario_count": len(scenarios),
    "missing_manifests": missing,
    "scenarios": scenarios,
}

with open(summary_path, "w", encoding="utf-8") as f:
    json.dump(summary, f, ensure_ascii=False, indent=2)

print(json.dumps({
    "profile": profile,
    "jobs": jobs,
    "output_dir": output_dir,
    "total_photos": total_photos,
    "total_albums": total_albums,
    "summary": summary_path,
}, ensure_ascii=False, indent=2))
PY

cat <<EOF

Dataset ready.

Scan source root:
  $OUTPUT_DIR

Manifest summary:
  $summary_path

Example smoke run against an already running service:
  BASE_URL=http://127.0.0.1:8080/rpc/v1 DATA_DIR="$OUTPUT_DIR" TIMEOUT_SEC=1800 ./service/scripts/smoke_test.sh
EOF
