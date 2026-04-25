# Scan Stress Matrix Report: 2026-04-25

- Data root: `/home/bucky/.xphoto/test_data`
- Profile: `stress`
- Expected photos: `17227`
- Expected album manifest sum: `326`
- Report files: `doc/reports/scan_stress_stress-*.json|md`

## Summary

The first full stress matrix passed all correctness checks. Every run produced:

- API search total: `17227`
- DB active photos: `17227`
- FTS rows: `17227`
- Job failed count sum: `0`
- Duplicate storage groups: `0`

## Matrix Results

| Label | Mode | Scan concurrency | Hash parallelism | Hash batch | Jobs | Elapsed | Throughput | Passed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `stress-single-cj2-hp0-bs32` | `single` | 2 | 0 | 32 | 1 | 18s | 957.1 photos/s | yes |
| `stress-single-cj2-hp4-bs64` | `single` | 2 | 4 | 64 | 1 | 26s | 662.6 photos/s | yes |
| `stress-single-cj2-hp8-bs128` | `single` | 2 | 8 | 128 | 1 | 26s | 662.6 photos/s | yes |
| `stress-scenarios-cj2-hp4-bs64` | `scenarios` | 2 | 4 | 64 | 8 | 18s | 957.1 photos/s | yes |
| `stress-scenarios-cj4-hp4-bs64` | `scenarios` | 4 | 4 | 64 | 8 | 15s | 1148.5 photos/s | yes |
| `stress-scenarios-cj4-hp8-bs128` | `scenarios` | 4 | 8 | 128 | 8 | 17s | 1013.4 photos/s | yes |
| `stress-scenarios-cj8-hp4-bs64-r1` | `scenarios` | 8 | 4 | 64 | 8 | 15s | 1148.5 photos/s | yes |
| `stress-scenarios-cj8-hp4-bs64-r2` | `scenarios` | 8 | 4 | 64 | 8 | 15s | 1148.5 photos/s | yes |
| `stress-scenarios-cj8-hp4-bs64-r3` | `scenarios` | 8 | 4 | 64 | 8 | 16s | 1076.7 photos/s | yes |

## Observations

- `scenarios` mode with scan concurrency `4` and hash parallelism `4` was the fastest run in this matrix.
- Repeating `scenarios` with scan concurrency `8` passed three times, with elapsed time between 15s and 16s.
- Increasing hash parallelism from `4` to `8` did not improve this dataset in either `single` or `scenarios` mode.
- The default/auto hash parallelism case (`hp0`) was fastest in `single` mode, but this should be rechecked with repeated runs before treating it as a tuning conclusion.
- No correctness failure appeared in the matrix: job status, API count, DB active count, FTS count, and duplicate storage checks all matched.

## Mutation Correctness

Report: `doc/reports/scan_mutation_stress_mutation-stress-full2021.md`

| Case | Result |
| --- | --- |
| Initial scan of `family-full-2021` | 3018 API / DB active / FTS rows |
| Delete 25 files and rescan | 2993 active, 25 deleted, 2993 FTS rows |
| Append 10 files and rescan | 3003 active, 25 deleted, 3003 FTS rows |
| Cancel full stress scan under load | terminal status `cancelled` |
| Retry after missing-root failure | first job `failed`, retry job `success`, 1 active photo |

## Follow-Up

1. Add a dedicated script for repeating selected matrix cases and computing min/mean/max throughput.
2. Add true in-scan mutation cases: append/delete files while the scan is running, then verify the next full rescan converges.
3. Preserve per-run service summary logs when a case fails or regresses, because the JSON report currently keeps the temp log path but the script cleans the process only after the run.
