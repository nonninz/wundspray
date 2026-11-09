# wundspray

[![ci](https://github.com/nonninz/wundspray/actions/workflows/ci.yml/badge.svg)](https://github.com/nonninz/wundspray/actions/workflows/ci.yml)

Tiny HTTP service: give it a URL, get back a cleaned one.

It unwraps tracking redirects and strips tracking parameters — including
referral-marketing tags like Amazon `tag` — then returns a **stable JSON
envelope**.

Built for a small Docker footprint (static musl binary on `scratch`).

## Credits & licenses

URL sanitization is powered entirely by
[**url-sanitize**](https://github.com/antonio-orionus/url-sanitize) /
[`url-sanitize-core`](https://crates.io/crates/url-sanitize-core) by
[antonio-orionus](https://github.com/antonio-orionus).

| Piece | License | Where |
| --- | --- | --- |
| wundspray service code | MIT | [`LICENSE`](LICENSE), [`LICENSES/MIT.txt`](LICENSES/MIT.txt) |
| `url-sanitize-core` engine | MIT | [`LICENSES/url-sanitize-MIT.txt`](LICENSES/url-sanitize-MIT.txt) |
| ClearURLs rule data | LGPL-3.0-only | [`LICENSES/LGPL-3.0-only.txt`](LICENSES/LGPL-3.0-only.txt) |
| Brave / Firefox rule data | MPL-2.0 | [`LICENSES/MPL-2.0.txt`](LICENSES/MPL-2.0.txt) |

Because the release **binary and Docker image embed the catalog**, the
distributed artifact is **MIT AND LGPL-3.0-only AND MPL-2.0**.

- Attribution + how to rebuild with modified rules: [`NOTICE`](NOTICE)
- Catalog pin / hashes: [`catalog/PIN.txt`](catalog/PIN.txt)
- Image ships the same texts under `/licenses/`

AdGuard rules are **not** included (GPL-3.0 upstream data; opt-in in
url-sanitize, out of scope here).

This project is not affiliated with or endorsed by antonio-orionus, ClearURLs,
Brave, Mozilla, or AdGuard.

## API

OpenAPI 3.1 contract: [`openapi.yaml`](openapi.yaml).

### `GET /health`

```json
{ "status": "ok" }
```

### `POST /sanitize`

Request:

```json
{ "url": "https://example.com/a?utm_source=x&tag=aff-20" }
```

Response (always the same shape):

```json
{
  "input": "https://example.com/a?utm_source=x&tag=aff-20",
  "url": "https://example.com/a",
  "changed": true,
  "action": "cleaned",
  "stripped_params": ["utm_source", "tag"],
  "blocked": false
}
```

| Field | Meaning |
| --- | --- |
| `action` | `unchanged` \| `cleaned` \| `redirected` \| `blocked` |
| `url` | Result URL (`null` only if `blocked`) |
| `stripped_params` | Removed query/fragment param names when cleaned |
| `blocked` | Domain-block hit (disabled by default in this service) |

Errors (`400`):

```json
{ "error": "invalid_url", "message": "url must be a non-empty string" }
```

Matched rules / redirect `via` are **not** returned. Enable `LOG_LEVEL=debug`
to see them in process logs.

## Behaviour defaults

| Option | wundspray | upstream url-sanitize default |
| --- | --- | --- |
| Strip normal tracking (`utm_*`, `fbclid`, …) | on | on |
| Strip referral marketing (`tag`, `ref`, …) | **on** | off |
| Unwrap redirects | on | on |
| Domain blocking | **off** | off |

## Quick start (local)

Requires [asdf](https://asdf-vm.com/) with the `rust` plugin (see
`.tool-versions`), or any Rust **1.99.1** toolchain with `rustfmt` + `clippy`.

```sh
asdf install          # from .tool-versions
make test
cargo run
```

```sh
curl -s localhost:8080/health
curl -s localhost:8080/sanitize \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com/?utm_source=x"}'
```

## Configuration

| Variable | Default | Description |
| --- | --- | --- |
| `PORT` | `8080` | Listen port (`0.0.0.0`) |
| `LOG_LEVEL` | `info` | `trace` / `debug` / `info` / `warn` / `error`, or a full `tracing` filter expression containing `=` |

At `info`, each `/sanitize` logs action / changed / latency.  
At `debug`+, also logs input/output, stripped params, matched rules, and `via`.

## Docker

Multi-stage build: Alpine Rust → static musl binary → `scratch`.

```sh
docker build -t wundspray .
docker run --rm -p 8080:8080 -e LOG_LEVEL=info wundspray
```

No outbound network is required at runtime (catalog is embedded).

### Releasing / GHCR

Version lives in two places that must stay aligned on a release:

| Source | Role |
| --- | --- |
| `Cargo.toml` `version` (and `openapi.yaml` `info.version`) | In-repo package / API version; embedded in the binary via `CARGO_PKG_VERSION` |
| GitHub Release tag `vX.Y.Z` | User-facing release; drives GHCR image tags |

The `image` workflow (`.github/workflows/image.yml`) runs **only when a GitHub
Release is published**. It first runs the same fmt/clippy/test checks as `ci`
(via reusable `.github/workflows/rust-check.yml`), then fails if `Cargo.toml`’s
package version does not exactly match the release tag without the leading `v`
(so `v1.2.3` requires `version = "1.2.3"`). GHCR is pushed only if both pass.

Checklist:

1. Bump `Cargo.toml` `version` and `openapi.yaml` `info.version` to `X.Y.Z`.
2. Commit that bump to `main`.
3. Create a GitHub Release with tag `vX.Y.Z` pointing at that commit.
4. Wait for the `image` workflow; it pushes:

```sh
docker pull ghcr.io/nonninz/wundspray:X.Y.Z   # also :X.Y, :X, and :latest
```

First-time GHCR notes:
- Repo **Settings → Actions → General**: workflows permitted; GITHUB_TOKEN can
  create/read/write packages (default for `packages: write` permission).
- After the first push, if the package is private, grant access under
  **Packages** or make it public.
- Optional: add package README / link the package back to this repo in GHCR.

## Development

```sh
make fmt          # rustfmt
make lint         # clippy -D warnings
make test
make check        # fmt-check + lint + test
```

## License

- Source (this repo’s Rust code): MIT — [`LICENSE`](LICENSE)
- Binary / container (embeds catalog): MIT AND LGPL-3.0-only AND MPL-2.0 —
  see [`NOTICE`](NOTICE) and [`LICENSES/`](LICENSES/)
