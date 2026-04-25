# Scan Stress Report: stress-scenarios-cj8-hp4-bs64-r1

- Passed: `True`
- Profile: `stress`
- Mode: `scenarios`
- Data root: `/home/bucky/.xphoto/test_data`
- Elapsed: `15s`
- Expected photos: `17227`
- API search total: `17227`
- DB active photos: `17227`
- FTS rows: `17227`
- Job failed count sum: `0`
- DB path: `/tmp/xphoto_scan_stress_stress-scenarios-cj8-hp4-bs64-r1.HcrX2K/stress.db`
- Log dir: `/tmp/xphoto_scan_stress_stress-scenarios-cj8-hp4-bs64-r1.HcrX2K/logs`

## Correctness Checks

| Check | Result |
| --- | --- |
| `all_jobs_success` | `True` |
| `job_failed_count_zero` | `True` |
| `api_search_total_matches_expected` | `True` |
| `db_active_photo_count_matches_expected` | `True` |
| `fts_rows_match_active_photos` | `True` |
| `duplicate_storage_groups_zero` | `True` |

## Jobs

| Scenario | Status | Total | Processed | New | Updated | Failed |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `sample` | `success` | `1` | `1` | `1` | `0` | `0` |
| `family-medium-2024` | `success` | `151` | `151` | `151` | `0` | `0` |
| `family-full-2020` | `success` | `2931` | `2931` | `2931` | `0` | `0` |
| `family-full-2021` | `success` | `3018` | `3018` | `3018` | `0` | `0` |
| `family-full-2022` | `success` | `2872` | `2872` | `2872` | `0` | `0` |
| `family-full-2023` | `success` | `2943` | `2943` | `2943` | `0` | `0` |
| `family-full-2024` | `success` | `2509` | `2509` | `2509` | `0` | `0` |
| `family-full-2025` | `success` | `2802` | `2802` | `2802` | `0` | `0` |
