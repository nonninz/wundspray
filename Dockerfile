# syntax=docker/dockerfile:1

# Build a fully static musl binary, then ship it on scratch with license files.
FROM rust:1.99-alpine AS builder

RUN apk add --no-cache musl-dev file

WORKDIR /app

# Dependency layer cache (dynamic musl build — proc-macros need this)
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && printf 'fn main() {}\n' > src/main.rs \
    && printf '// placeholder\n' > src/lib.rs \
    && cargo build --release \
    && rm -rf src

COPY catalog ./catalog
COPY static ./static
COPY src ./src

# Rebuild our crate, then re-link only the final binary as fully static so
# proc-macro deps stay usable while the shipped binary runs on scratch.
RUN touch src/main.rs src/lib.rs \
    && cargo build --release \
    && cargo rustc --release --bin wundspray -- -C target-feature=+crt-static \
    && strip target/release/wundspray \
    && file -b target/release/wundspray \
    && test "$(file -b target/release/wundspray | grep -Ec 'statically linked|static-pie linked')" -ge 1

FROM scratch

COPY --from=builder /app/target/release/wundspray /wundspray
COPY LICENSE NOTICE /licenses/
COPY LICENSES/ /licenses/LICENSES/
COPY catalog/PIN.txt /licenses/catalog-PIN.txt

ENV PORT=8080 \
    LOG_LEVEL=info

EXPOSE 8080

USER 65532:65532

ENTRYPOINT ["/wundspray"]
