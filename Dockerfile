# syntax=docker/dockerfile:1

# Build a fully static musl binary, then ship it on scratch with license files.
FROM rust:1.99-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app

# Dependency layer cache
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && printf 'fn main() {}\n' > src/main.rs \
    && printf '// placeholder\n' > src/lib.rs \
    && cargo build --release \
    && rm -rf src

COPY catalog ./catalog
COPY src ./src

# Force rebuild of our crate after sources land
RUN touch src/main.rs src/lib.rs \
    && RUSTFLAGS="-C target-feature=+crt-static" cargo build --release \
    && strip target/release/wundspray

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
