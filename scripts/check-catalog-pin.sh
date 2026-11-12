#!/usr/bin/env bash
# Fail if Cargo.toml url-sanitize-core version does not match catalog/PIN.txt.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo_ver="$(sed -nE 's/^url-sanitize-core = "([^"]+)"/\1/p' Cargo.toml | head -n1)"
pin_ver="$(sed -nE 's/^Crate:[[:space:]]+url-sanitize-core[[:space:]]+(.+)$/\1/p' catalog/PIN.txt | head -n1)"
pin_tag="$(sed -nE 's/^Tag:[[:space:]]+(.+)$/\1/p' catalog/PIN.txt | head -n1)"

if [[ -z "$cargo_ver" ]]; then
  echo "check-catalog-pin: could not read url-sanitize-core from Cargo.toml" >&2
  exit 1
fi
if [[ -z "$pin_ver" || -z "$pin_tag" ]]; then
  echo "check-catalog-pin: could not read Crate/Tag from catalog/PIN.txt" >&2
  exit 1
fi

expected_tag="v${cargo_ver}"
if [[ "$cargo_ver" != "$pin_ver" ]]; then
  echo "check-catalog-pin: Cargo.toml url-sanitize-core=${cargo_ver} != PIN Crate ${pin_ver}" >&2
  exit 1
fi
if [[ "$pin_tag" != "$expected_tag" ]]; then
  echo "check-catalog-pin: PIN Tag ${pin_tag} != expected ${expected_tag}" >&2
  exit 1
fi

if [[ ! -f catalog/catalog.json ]]; then
  echo "check-catalog-pin: missing catalog/catalog.json" >&2
  exit 1
fi
if [[ ! -f catalog/overlay.json ]]; then
  echo "check-catalog-pin: missing catalog/overlay.json" >&2
  exit 1
fi

echo "check-catalog-pin: ok (url-sanitize-core ${cargo_ver}, tag ${pin_tag})"
