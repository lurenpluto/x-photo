# AGENTS

## Scope and Current State

- Repository root: `/home/bucky/work/x_photo`
- Product: local photo management and browsing app.
- Backend: Rust service under `service/`, using Axum, SQLx SQLite, Tokio, tracing, image processing, EXIF parsing, scan/task jobs, album rules, preview/thumbnail cache, and a test data builder.
- Frontend: static web app under `web/` using plain HTML/CSS/JavaScript. No Node package manifest is currently present.
- Docs: project documents live under `doc/`, including `doc/OpenAPI.yaml`, `doc/问题修复跟踪.md`, test data tooling docs, task-system design, and user guides.
- Cursor rules were not found at `.cursor/rules/` or `.cursorrules` when this file was updated.
- Copilot rules were not found at `.github/copilot-instructions.md` when this file was updated.

## Repository Health Check

Run these before deep work:

- `ls -la`
- `git status --short --branch`
- `find . -maxdepth 3 -type f \( -name package.json -o -name Cargo.toml -o -name Makefile \) -print`
- `test -f service/Cargo.toml && echo "rust-service" || true`
- `test -f web/index.html && echo "static-web" || true`
- `test -f doc/OpenAPI.yaml && echo "openapi-doc" || true`

If a new stack or package manager is added, extend this file with exact commands and keep existing entries.

## Command Priority and Defaults

- Rust backend commands run from `service/`.
- Static web files can be opened directly or served by any simple static server when browser verification is needed.
- If a future Node toolchain appears under `web/`, choose package manager by lockfile in this order: `bun.lock`, `pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`.
- Do not introduce a Node build pipeline unless the task explicitly requires it.

## Build / Lint / Test Commands

### Rust Backend (`service/`)

- Format check: `cargo fmt --all -- --check`
- Format write: `cargo fmt --all`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Test: `cargo test`
- Targeted test examples:
  - `cargo test --test scan_workflow manual_rescan_should_not_resume_after_previous_success -- --nocapture`
  - `cargo test --test scan_workflow generated_sample_data_should_scan_exif_and_album -- --nocapture`
  - `cargo test --test error_recovery cancel_scan_should_reach_cancelled -- --nocapture`
  - `cargo test config::tests -- --nocapture`
- Build: `cargo build`
- Optional docs: `cargo doc --no-deps --document-private-items`

### Test Data Builder

- Generate sample data:
  - `cargo run --bin testdata_builder -- /tmp/xphoto_test_data_sample --strategy sample --year 2025 --seed 42`
- Generate larger weekend-family data only when intentionally testing scale; it can take minutes:
  - `cargo run --bin testdata_builder -- /tmp/xphoto_test_data_family --strategy family_us_weekends_2025 --year 2025 --seed 42`

### Smoke Test

- Backend smoke script lives at `service/scripts/smoke_test.sh`.
- Typical invocation with an already running service:
  - `BASE_URL=http://127.0.0.1:8080/rpc/v1 DATA_DIR=/tmp/xphoto_test_data_sample TIMEOUT_SEC=60 ./scripts/smoke_test.sh`

### Static Web (`web/`)

- No package manager or build command is currently defined.
- For browser testing, serve the repo or `web/` directory with a temporary static server and keep backend URL/proxy assumptions explicit.
- If frontend behavior changes, use browser automation when useful and capture the URL/viewport tested.

## Documentation Commands

- OpenAPI source of truth is `service/src/api/mod.rs` plus handler request/response types in `service/src/api/types.rs`.
- When API routes change, update `doc/OpenAPI.yaml` in the same task.
- Track project issues and repair status in `doc/问题修复跟踪.md`.

## Code Style Guidelines

### Rust

- Follow `cargo fmt` and `cargo clippy --all-targets -- -D warnings`.
- Prefer typed request/response structs at API boundaries.
- Keep SQL errors wrapped with operation and identifiers; log meaningful runtime failures with `tracing`.
- Add or update focused tests for behavior changes, especially scan/task state transitions and API contracts.

### JavaScript / Web

- Keep the current static app style unless a larger frontend migration is explicitly requested.
- Avoid introducing dependencies for small UI/API fixes.
- Keep API paths aligned with `/rpc/v1` routes in the Rust service.

### Docs

- Keep Markdown concise and executable: include exact commands and status evidence.
- For `doc/问题修复跟踪.md`, new issues start as `TODO`, active work may use `IN_PROGRESS`, and completed verified items should be marked `VERIFIED` with command evidence.

## PR/Task Verification Checklist

- Run `git diff --check` before handoff.
- For backend code changes, run:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings`
  - targeted tests for changed logic
  - `cargo test` before final handoff when feasible
- For docs-only changes, run at least `git diff --check` and any lightweight structural checks available.
- Keep unrelated generated files and temporary test data out of commits.

## Extending This File

- If Cursor/Copilot rule files are added, append their constraints here immediately.
- If `web/` gains a package manager, add install/lint/typecheck/test/build commands here.
- If deployment scripts or service units are added, document the exact verification and startup commands here.
