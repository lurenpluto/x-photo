# Scan Mutation Stress Report: mutation-in-scan-full2021

- Passed: `True`
- Profile: `stress`
- Scenario: `family-full-2021`
- Data root: `/home/bucky/.xphoto/test_data`
- Baseline photos: `3018`
- Deleted files: `25`
- Appended files: `10`
- DB path: `/tmp/xphoto_scan_mutation_mutation-in-scan-full2021.6a4nr8zz/mutation.db`
- Log dir: `/tmp/xphoto_scan_mutation_mutation-in-scan-full2021.6a4nr8zz/logs`

## Correctness Checks

| Check | Result |
| --- | --- |
| `initial_scan_success` | `True` |
| `initial_counts_match` | `True` |
| `delete_rescan_success` | `True` |
| `delete_counts_match` | `True` |
| `append_rescan_success` | `True` |
| `append_counts_match` | `True` |
| `cancel_under_load_reaches_cancelled` | `True` |
| `retry_initial_failure` | `True` |
| `retry_after_fix_success` | `True` |
| `retry_counts_match` | `True` |
| `in_scan_baseline_counts_match` | `True` |
| `in_scan_mutated_while_running` | `True` |
| `in_scan_mutation_job_success` | `True` |
| `in_scan_converge_success` | `True` |
| `in_scan_converge_counts_match` | `True` |
| `duplicate_storage_groups_zero` | `True` |

## Source Counts

| Phase | API Total | Active | Deleted | FTS Rows |
| --- | ---: | ---: | ---: | ---: |
| `initial` | 3018 | 3018 | 0 | 3018 |
| `after_delete` | 2993 | 2993 | 25 | 2993 |
| `after_append` | 3003 | 3003 | 25 | 3003 |
| `retry_after_fix` | 1 | 1 | 0 | 1 |
| `in_scan_baseline` | 17227 | 17227 | 0 | 17227 |
| `in_scan_after_mutation_job` | 17202 | 17202 | 25 | 17202 |
| `in_scan_converged` | 17212 | 17212 | 25 | 17212 |

## In-Scan Mutation

- Pre-mutation observed job status: `running`
- Baseline photos: `17227`
- Deleted while scan was active: `25`
- Appended while scan was active: `10`
- Expected after convergence: `17212`

## Jobs

| Label | Status | Total | Processed | New | Updated | Failed |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `mutation_initial` | `success` | 3018 | 3018 | 3018 | 0 | 0 |
| `mutation_delete` | `success` | 2993 | 2993 | 0 | 25 | 0 |
| `mutation_append` | `success` | 3003 | 3003 | 10 | 0 | 0 |
| `cancel_load` | `cancelled` | 17227 | 0 | None | None | 0 |
| `retry_missing_initial` | `failed` | None | 0 | None | None | 1 |
| `retry_missing` | `success` | 1 | 1 | 1 | 0 | 0 |
| `in_scan_baseline` | `success` | 17227 | 17227 | 17227 | 0 | 0 |
| `in_scan_mutation` | `success` | 17227 | 17227 | 0 | 25 | 25 |
| `in_scan_converge` | `success` | 17212 | 17212 | 10 | 0 | 0 |
