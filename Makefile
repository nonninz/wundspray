.PHONY: fmt fmt-check lint test build check sync-catalog check-catalog-pin

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test

build:
	cargo build --release

check-catalog-pin:
	./scripts/check-catalog-pin.sh

check: check-catalog-pin fmt-check lint test

# Usage: make sync-catalog
#        make sync-catalog VERSION=vX.Y.Z
# With no VERSION, syncs to the newest url-sanitize-core on crates.io.
sync-catalog:
	@if [ -n "$(VERSION)" ]; then ./scripts/sync-catalog.sh "$(VERSION)"; else ./scripts/sync-catalog.sh; fi
