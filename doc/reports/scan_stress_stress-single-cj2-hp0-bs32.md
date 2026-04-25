# Scan Stress Report: stress-single-cj2-hp0-bs32

- Passed: `True`
- Profile: `stress`
- Mode: `single`
- Data root: `/home/bucky/.xphoto/test_data`
- Elapsed: `18s`
- Expected photos: `17227`
- API search total: `17227`
- DB active photos: `17227`
- FTS rows: `17227`
- Job failed count sum: `0`
- DB path: `/tmp/xphoto_scan_stress_stress-single-cj2-hp0-bs32.ykPkID/stress.db`
- Log dir: `/tmp/xphoto_scan_stress_stress-single-cj2-hp0-bs32.ykPkID/logs`

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
| `all` | `success` | `17227` | `17227` | `17227` | `0` | `0` |
