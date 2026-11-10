#!/usr/bin/env bash
# Sync vendored catalog + Cargo pin from a published url-sanitize release tag.
# Usage: scripts/sync-catalog.sh vX.Y.Z
#        make sync-catalog VERSION=vX.Y.Z
# Does not modify catalog/overlay.json.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ $# -ne 1 ]]; then
  echo "usage: $0 vX.Y.Z" >&2
  exit 2
fi

VERSION_RAW="$1"
if [[ "$VERSION_RAW" != v* ]]; then
  echo "VERSION must look like vX.Y.Z (got: $VERSION_RAW)" >&2
  exit 2
fi

TAG="$VERSION_RAW"
CRATE_VERSION="${TAG#v}"
UPSTREAM_REPO="antonio-orionus/url-sanitize"
CATALOG_PATH="crates/url-sanitize/catalog/catalog.json"
UA="wundspray-sync-catalog (https://github.com/nonninz/wundspray)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Resolving commit for ${UPSTREAM_REPO}@${TAG}..."
ref_json="$(curl -fsSL \
  -H "User-Agent: ${UA}" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/${UPSTREAM_REPO}/git/ref/tags/${TAG}")"

obj_type="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["object"]["type"])' <<<"$ref_json")"
obj_sha="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["object"]["sha"])' <<<"$ref_json")"

if [[ "$obj_type" == "commit" ]]; then
  commit="$obj_sha"
else
  # Annotated tag: resolve the pointed-to commit object.
  commit="$(curl -fsSL \
    -H "User-Agent: ${UA}" \
    -H "Accept: application/vnd.github+json" \
    "https://api.github.com/repos/${UPSTREAM_REPO}/git/tags/${obj_sha}" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["object"]["sha"])')"
fi

if [[ -z "$commit" || ${#commit} -lt 7 ]]; then
  echo "Could not resolve commit for tag ${TAG}" >&2
  exit 1
fi

echo "Fetching catalog from ${TAG} (${commit:0:12})..."
curl -fsSL \
  -H "User-Agent: ${UA}" \
  "https://raw.githubusercontent.com/${UPSTREAM_REPO}/${TAG}/${CATALOG_PATH}" \
  -o "${tmp}/catalog.json"

python3 - "${tmp}/catalog.json" <<'PY'
import json, sys
path = sys.argv[1]
with open(path, encoding="utf-8") as f:
    data = json.load(f)
assert "rules" in data and isinstance(data["rules"], list), "catalog missing rules"
assert data["rules"], "catalog rules empty"
PY

# Rewrite Cargo.toml dependency pin (crate version without leading v).
python3 - "$ROOT/Cargo.toml" "$CRATE_VERSION" <<'PY'
from pathlib import Path
import re, sys
path = Path(sys.argv[1])
ver = sys.argv[2]
text = path.read_text(encoding="utf-8")
new, n = re.subn(
    r'(?m)^url-sanitize-core\s*=\s*"[^"]+"\s*$',
    f'url-sanitize-core = "{ver}"',
    text,
    count=1,
)
if n != 1:
    raise SystemExit("failed to rewrite url-sanitize-core in Cargo.toml")
path.write_text(new, encoding="utf-8")
PY

cp "${tmp}/catalog.json" "$ROOT/catalog/catalog.json"

# Derive PIN metadata from the downloaded catalog.
python3 - "$ROOT/catalog/catalog.json" "$ROOT/catalog/PIN.txt" "$TAG" "$commit" "$CRATE_VERSION" "$CATALOG_PATH" <<'PY'
import json, sys
from pathlib import Path

catalog_path, pin_path, tag, commit, crate_ver, rel_path = sys.argv[1:7]
data = json.loads(Path(catalog_path).read_text(encoding="utf-8"))
sources = data.get("sources") or []
short = []
license_lines = []
for src in sources:
    h = (src.get("hash") or "")[:12]
    if h:
        short.append(h)
    name = src.get("name") or "unknown"
    lic = src.get("license") or "UNKNOWN"
    license_lines.append(f"  {name:<10} {lic}")

identity = data.get("version") or "unknown"
if isinstance(identity, str) and identity.startswith("merged("):
    pass
elif short:
    identity = "merged(" + ",".join(short) + ")"

generated = data.get("generatedAt") or data.get("generated_at") or "unknown"

pin = f"""Upstream: https://github.com/antonio-orionus/url-sanitize
Tag:      {tag}
Commit:   {commit}
Path:     {rel_path}
Crate:    url-sanitize-core {crate_ver}

Catalog identity: {identity}
generatedAt:      {generated}

Licenses (rule data):
{chr(10).join(license_lines) if license_lines else "  (none listed)"}
"""
Path(pin_path).write_text(pin, encoding="utf-8")
print(pin)
PY

echo "Updating Cargo.lock..."
cargo update -p url-sanitize-core

echo "Done. overlay.json was not modified."
echo "Next: make check"
