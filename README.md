# wundspray

Tiny HTTP service that takes a URL, unwraps tracking redirects, strips tracking
parameters (including referral-marketing tags), and returns a stable JSON
envelope.

## Credits

URL sanitization is provided by
[**url-sanitize**](https://github.com/antonio-orionus/url-sanitize) /
[`url-sanitize-core`](https://crates.io/crates/url-sanitize-core) by
[antonio-orionus](https://github.com/antonio-orionus).

- Engine: MIT
- Default rule catalog (ClearURLs / Brave / Firefox): LGPL-3.0 / MPL-2.0

This service embeds a pinned copy of the upstream merged catalog. Full license
notes will live in `NOTICE` / `LICENSES/` as the project grows.

## Status

Work in progress.
