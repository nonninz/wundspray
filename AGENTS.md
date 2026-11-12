# wundspray — agent guide

Tiny Rust HTTP service: unwrap tracking redirects and strip tracking params,
return a stable JSON envelope. Stack: Axum + Tokio, OpenAPI in `openapi.yaml`,
static musl Docker image on `scratch`.

Skills live under `.agents/skills/` (shared by OpenCode and Cursor). Shared
checklists live under `.agents/references/`.

## Core rules

- If a task matches an installed skill, load it with the `skill` tool (or read
  `.agents/skills/<name>/SKILL.md`) **before** acting, then follow it fully.
- Prefer thin vertical slices: implement → test → verify → commit (when asked).
- Do not invent APIs: keep `openapi.yaml` and handlers in sync.
- Do not commit secrets, `.env`, or build artifacts.
- Only create git commits when the user explicitly asks.
- Never bump or sync `url-sanitize-core` / `catalog/` casually — use
  `make sync-catalog` (or `VERSION=vX.Y.Z`) and keep `Cargo.toml` pin aligned
  with `catalog/PIN.txt`.

## Project commands

```sh
make fmt                 # rustfmt
make fmt-check
make lint                # clippy -D warnings
make test                # cargo test
make build               # cargo build --release
make check-catalog-pin   # Cargo.toml pin == catalog/PIN.txt
make check               # pin + fmt-check + lint + test  (CI gate)
make sync-catalog        # or VERSION=vX.Y.Z
cargo run                # local server on PORT (default 8080)
```

Toolchain: asdf + `.tool-versions`, or Rust with `rustfmt` + `clippy`.

Before calling a change done, run `make check` (or the subset that still applies)
and fix failures.

## Layout (orientation)

| Path | Role |
| --- | --- |
| `src/routes.rs` | HTTP routes (`/health`, `/clean`, deprecated `/sanitize`) |
| `src/sanitize.rs` | Cleaning / mapping to response fields |
| `src/map.rs` | Response mapping helpers |
| `src/logging.rs` / `src/client_ip.rs` | Request logging, trusted proxies |
| `openapi.yaml` | Public API contract — update with behavior changes |
| `catalog/` | Vendored rule data + `PIN.txt` + local `overlay.json` |
| `tests/` | Integration / HTTP tests |
| `scripts/` | Catalog pin check + sync |

## Landmines

- Catalog is **LGPL/MPL** embedded data — do not add AdGuard (GPL) rules.
- `catalog/overlay.json` is local hotfixes; `make sync-catalog` must not wipe it.
- `POST /sanitize` is deprecated (308 → `/clean`); do not build new clients on it.
- Domain blocking is **off** by default; keep that unless the task says otherwise.
- Release version must stay aligned across `Cargo.toml`, `openapi.yaml`
  `info.version`, and GitHub release tag `vX.Y.Z`.

## Intent → skill mapping

| Intent | Skill |
| --- | --- |
| Which skill applies / session start | `using-agent-skills` |
| Underspecified ask / new significant feature | `spec-driven-development` |
| Break a spec into tasks | `planning-and-task-breakdown` |
| Implement a multi-file change | `incremental-implementation` |
| Any logic / bug / behavior change | `test-driven-development` |
| Tests fail, build breaks, unexpected behavior | `debugging-and-error-recovery` |
| API / OpenAPI / module boundary | `api-and-interface-design` |
| Auth, untrusted input, deps, SSRF-ish URL handling | `security-and-hardening` |
| Before merge / review a diff or PR | `code-review-and-quality` |
| Commits, PR sizing, worktrees, changelog | `git-workflow-and-versioning` |
| Context quality drops / switch tasks | `context-engineering` |

## Brownfield default

This is an established codebase. Prefer:

1. Reproduce / characterize with tests before changing behavior.
2. Small atomic diffs; do not drive-by refactor unrelated files.
3. Full `/spec → /plan → /build → /review` only for **new** features or
   significant changes — not for every typo fix.

## Definition of done (project)

A change is done when:

- [ ] Behavior matches the task (and OpenAPI if the HTTP surface moved)
- [ ] `make check` passes
- [ ] No unrelated files touched
- [ ] License / catalog pin rules respected when those paths change
