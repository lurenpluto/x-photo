# Scan Stress Report: pilot-scenarios-2

- Passed: `True`
- Profile: `stress`
- Mode: `scenarios`
- Data root: `/home/bucky/.xphoto/test_data`
- Elapsed: `2s`
- Expected photos: `152`
- API search total: `152`
- DB active photos: `152`
- FTS rows: `152`
- Job failed count sum: `0`
- DB path: `/tmp/xphoto_scan_stress_pilot-scenarios-2.7i4kiq/stress.db`
- Log dir: `/tmp/xphoto_scan_stress_pilot-scenarios-2.7i4kiq/logs`

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
