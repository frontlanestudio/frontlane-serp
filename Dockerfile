# Multi-stage Rust build
FROM rust:1.85-bookworm AS builder

WORKDIR /build

# Build dependencies for the vendored BoringSSL used by the browser TLS
# fingerprint impersonation client (wreq/boring-sys): cmake and a C/C++
# toolchain to build it, pkg-config so it's discoverable, clang/libclang for
# bindgen's FFI binding generation, and git since boring-sys fetches its
# BoringSSL source as a submodule-style checkout.
RUN apt-get update \
  && apt-get install -y --no-install-recommends cmake perl pkg-config clang libclang-dev git \
  && rm -rf /var/lib/apt/lists/*

# Copy source code and build release binary
COPY . .
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

WORKDIR /usr/src/app

# Install runtime dependencies for HTTPS and health checks
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --create-home --uid 1001 --shell /bin/bash serp \
  && chown serp:serp /usr/src/app

# Copy binary and symlink legacy name
COPY --from=builder /build/target/release/frontlane-serp /usr/local/bin/frontlane-serp
RUN ln -s /usr/local/bin/frontlane-serp /usr/local/bin/openserp

# Copy default configuration
COPY --chown=serp:serp config.yaml ./config.yaml

ENV FRONT_SERP_SERVER_HOST=0.0.0.0 \
    FRONT_SERP_SERVER_PORT=7000 \
    OPENSERP_SERVER_HOST=0.0.0.0 \
    OPENSERP_SERVER_PORT=7000

USER serp

EXPOSE 7000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -fsS "http://127.0.0.1:7000/health" || exit 1

ENTRYPOINT ["frontlane-serp"]
CMD ["serve"]
