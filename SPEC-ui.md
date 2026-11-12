# Spec: embedded clean-tool UI

## Objective

Ship a browser clean-tool page for wundspray: paste a URL, call the existing
`POST /clean` API same-origin, and show `url`, `action`, `changed`, and
`stripped_params`. Served from the same Axum process and scratch Docker image
as the API (assets compiled into the binary).

**Users:** people who want a quick web form instead of curling the API.

**Success:** visiting `/` cleans a known UTM example end-to-end; `make check`
passes; CSP + nosniff on HTML; `/clean` JSON schema unchanged.

## Tech Stack

- Axum 0.8 HTTP routes
- Vanilla HTML / CSS / JS embedded via `include_str!`
- Existing `POST /clean` JSON envelope (no new clean endpoint)

## Commands

```sh
make fmt
make fmt-check
make lint
make test
make check
cargo run   # local server on PORT (default 8080); open http://localhost:8080/
```

## Project Structure

```
static/index.html   → tool page markup
static/app.css      → styles
static/app.js       → form → POST /clean → safe DOM
src/ui.rs           → embed + handlers + security headers
src/routes.rs       → mounts GET /, /assets/app.css, /assets/app.js
tests/api.rs        → integration tests for UI routes
SPEC-ui.md          → this spec
```

## Code Style

Match existing Rust: snake_case modules, Axum handlers returning `Response`,
structured tracing on API routes. Frontend: no frameworks, no CDNs, no
`innerHTML` for API data — use `textContent` / `createTextNode`.

## Testing Strategy

- Integration tests in `tests/api.rs` via `tower::ServiceExt::oneshot`
- Assert `GET /` → 200, `text/html`, body contains `wundspray`, CSP + nosniff
- Assert asset routes return expected content types and 200
- Existing `/clean` / `/health` / `/sanitize` tests remain green

## Boundaries

- **Always:** `make check` before done; XSS-safe rendering; CSP without
  `unsafe-inline`; keep `/clean` contract unchanged
- **Ask first:** rate limiting, auth, new frontend toolchain, changing `/`
  away from the tool page
- **Never:** third-party CDNs; `innerHTML` of response fields; commit secrets;
  wipe `catalog/overlay.json`; AdGuard GPL rules

## Success Criteria

- [ ] `GET /` serves HTML with brand **wundspray**, form, and links to CSS/JS
- [ ] `GET /assets/app.css` and `GET /assets/app.js` return 200 with correct types
- [ ] Form posts to `/clean` and renders result/error safely
- [ ] HTML responses include CSP (`default-src 'self'; frame-ancestors 'none'`),
      `X-Content-Type-Options: nosniff`, `Referrer-Policy`, `X-Frame-Options: DENY`
- [ ] OpenAPI + README document the UI
- [ ] Docker builder copies `static/` so embed compiles; scratch image unchanged
- [ ] `make check` passes

## Open Questions

None — decisions locked in the approved plan (split assets, usable-minimal polish).
