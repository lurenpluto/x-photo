# AGENTS

## Scope and Current State

- Repository root: `/home/bucky/work/x_photo`
- `AGENTS.md` had not existed before this update and has now been created.
- No source files were found in the repository at analysis time.
- No Cursor rules were found at `.cursor/rules/` or `.cursorrules`.
- No Copilot rules were found at `.github/copilot-instructions.md`.
- The repo appears empty, so this document is intentionally stack-aware and uses project-detection fallbacks.

## Repository Health Check

Run these before deep work:

- `ls -la`
- `ls -la .*`
- `git status` (if this is a git checkout)
- `test -f package.json && echo "node" || true`
- `test -f pyproject.toml && echo "python" || true`
- `test -f Cargo.toml && echo "rust" || true`
- `test -f go.mod && echo "go" || true`
- `test -f Makefile && echo "make" || true`

If any new stack is detected, extend this file with the exact checks and keep existing entries.

## Command Priority and Defaults

When multiple toolchains are present, choose in this order:

1. `bun.lock` -> bun
2. `pnpm-lock.yaml` -> pnpm
3. `package-lock.json` -> npm
4. `yarn.lock` -> yarn
5. `pyproject.toml` + `uv.lock` -> uv
6. `Cargo.toml` -> cargo
7. `go.mod` -> go
8. `Makefile` -> make

## Build / Lint / Test Commands

### Node / TypeScript / JavaScript

- Install: `bun/pnpm/npm/yarn install`
- Lint: `bun run lint`, `pnpm lint`, `npm run lint`, `yarn lint`
- Format: `bun run format`, `pnpm format`, `npm run format`
- Type check: `bun run typecheck`, `pnpm tsc --noEmit`, `npm run typecheck`
- Test: `bun test`, `pnpm test`, `npm test`, `yarn test`
- Single test: `bun test path/to/file.test.ts`, `pnpm test path/to/file.test.ts`, `npm test -- path/to/file.test.ts`, `npx jest path/to/file.test.ts`, `npx vitest run path/to/file.test.ts`
- Build: `bun run build`, `pnpm build`, `npm run build`, `yarn build`

### Python

- Install: `python -m pip install -r requirements.txt`, `uv sync` if `uv.lock` exists
- Lint: `ruff check .`, `python -m flake8 .`
- Type check: `mypy .`, `pyright .`
- Format: `ruff format .`
- Test: `pytest`, `pytest -q`
- Single test: `pytest tests/test_file.py::test_name -q`, `pytest tests/test_file.py -k "name" -q`
- Build/package: `python -m pip wheel .`

### Rust

- Build: `cargo build`, `cargo build --release`
- Lint/format: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
- Test: `cargo test`
- Single test: `cargo test test_name -- --nocapture`, `cargo test module::tests::name -- --exact`, `cargo test --test integration_name`
- Docs: `cargo doc --no-deps --document-private-items`

### Go

- Build: `go build ./...`
- Lint: `gofmt -w .`, `golangci-lint run`
- Test: `go test ./...`
- Single test: `go test ./... -run TestName`, `go test ./pkg/name -run TestName/Case -v`

### Make-driven projects

- Typical commands: `make fmt`, `make lint`, `make test`
- Single test examples: `make test TEST=path/to/test`, `make test-file FILE=path/to/test.py`

If no recognized stack is detected, inspect `package.json`, `pyproject.toml`, `Cargo.toml`, `go.mod`, or Make targets before adding new tooling.

## Code Style Guidelines

### Imports

- Keep import grouping deterministic and predictable: standard library, third-party, local project.
- Sort imports within groups alphabetically.
- Prefer explicit imports over wildcard imports when available.
- Keep side-effect imports isolated and uncommon.

### Formatting

- Respect existing formatter config as authoritative (`.editorconfig`, prettier, ruff, gofmt, rustfmt, clang-format, etc.).
- Avoid mixed style choices inside a file.
- Prefer formatting tools over hand alignment.
- Keep one statement per line unless formatting rules require a different form.

### Typing

- Enable strict type checks when project tooling supports it.
- Add type annotations for public and boundary-facing functions.
- Use typed DTOs/models for data contracts across modules and services.
- Avoid permissive escape hatches (`any`, broad `object`, `interface{}`) unless justified by boundary constraints.

### Naming

- Use descriptive, intention-driven names (no unnecessary abbreviations).
- Match project separator conventions for filenames (`snake_case`, `kebab-case`, etc.).
- Keep variables and functions readable; use full words when possible.
- Use stable names at API boundaries unless a migration requires rename.
- Prefer noun phrases for types/classes, verbs for functions/methods.

### Error Handling

- Fail fast and surface a clear action path in errors.
- Wrap errors with context (`operation`, `resource`, `identifiers`).
- Avoid broad `catch`/`except` that swallows every failure.
- In async code, propagate cancellation and timeout behavior.
- Map internal failures to user-safe messages for API boundaries.

### Testing Guidance

- Add/adjust tests for new behavior and each failure branch touched.
- Keep assertions explicit and descriptive.
- Prefer deterministic fixtures and avoid timing-based randomness.
- Prioritize targeted tests for critical logic before broad refactors.

### PR/Task Verification Checklist

- Run stack-matched lint or format command for modified code.
- Run at least one targeted single test for changed logic.
- Run the relevant full test command before handing work over.
- Re-run repository health checks when build tooling changes.

## Extending This File

- Any new stack should add its install/lint/format/test/single-test/build commands to this file.
- If Cursor/Copilot rule files are added, append their constraints here immediately.
