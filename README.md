# wundspray

Tiny HTTP service: give it a URL, get back a cleaned one.

It unwraps tracking redirects and strips tracking parameters — including
referral-marketing tags like Amazon `tag` — then returns a **stable JSON
envelope**.

Built for a small Docker footprint (static musl binary on `scratch`).

## Credits

URL sanitization is powered entirely by
[**url-sanitize**](https://github.com/antonio-orionus/url-sanitize) /
[`url-sanitize-core`](https://crates.io/crates/url-sanitize-core) by
[antonio-orionus](https://github.com/antonio-orionus).

| Piece | License |
| --- | --- |
| `url-sanitize-core` engine | MIT |
| Embedded merged catalog (ClearURLs / Brave / Firefox) | LGPL-3.0 / MPL-2.0 |
| wundspray service code | MIT |

Catalog pin and hashes: [`catalog/PIN.txt`](catalog/PIN.txt).  
Third-party attribution: [`NOTICE`](NOTICE).

AdGuard rules are **not** included (GPL-3.0 upstream data; opt-in in
url-sanitize, out of scope here).

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

## Development

```sh
make fmt          # rustfmt
make lint         # clippy -D warnings
make test
make check        # fmt-check + lint + test
```

## License

MIT for wundspray code — see [`LICENSE`](LICENSE).  
Respect upstream catalog licenses when distributing the binary/image; see
[`NOTICE`](NOTICE).
