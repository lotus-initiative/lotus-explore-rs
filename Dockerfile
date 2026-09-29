# ── Stage 1: build ────────────────────────────────────────────────────────────
# Pin to the same channel as rust-toolchain.toml so the build is reproducible.
FROM rust:1.97.0-slim-bookworm AS builder

WORKDIR /build

# Copy manifests first, then ALL app sources: `cargo build -p lotus-explore-rs`
# resolves the entire workspace, so every member's manifest must be present.
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# --locked ensures Cargo.lock is respected exactly (no silent upgrades)
RUN cargo build --release --locked --features server -p lotus-explore-rs

# ── Stage 2: WASM client (with Ketcher) ──────────────────────────────────────
# Uses rust:1.97.0-slim-trixie (glibc 2.40) because the pre-built `dx` binary
# from Dioxus releases requires glibc >= 2.39 (Debian Bookworm only has 2.36).
# Same Rust 1.97.0 toolchain as the builder stage, so cached deps are reusable.
FROM rust:1.97.0-slim-trixie AS wasm-builder

# Reuse compiled dependencies from the builder stage (same Rust version).
COPY --from=builder /build/target /build/target

WORKDIR /build

# System deps: curl (for dx download), gcc (C linker for native build scripts),
# pkg-config + libssl-dev (crypto crates)
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl gcc pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Download the pre-built `dx` binary matching the host architecture
# (aarch64 on Apple Silicon, x86_64 on Intel/AMD). This avoids the slow
# `cargo install dioxus-cli` from source.
RUN arch=$(uname -m) && \
    case "$arch" in \
      aarch64) dx_arch="aarch64" ;; \
      x86_64)  dx_arch="x86_64" ;; \
      *) echo "unsupported arch: $arch" && exit 1 ;; \
    esac && \
    curl -fsSL -o /tmp/dx.tar.gz \
      https://github.com/DioxusLabs/dioxus/releases/download/v0.7.10/dx-${dx_arch}-unknown-linux-gnu.tar.gz && \
    tar -xzf /tmp/dx.tar.gz -C /usr/local/bin/ && \
    chmod +x /usr/local/bin/dx && \
    dx --version

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/

# Base path for the WASM bundle: "/" for Docker/local deployment,
# "/${REPNAME}" for GitHub Pages. Override with --build-arg.
ARG DX_BASE_PATH="/"

# Fetch configured curation assets and Ketcher (115 MB) then build the WASM web bundle.
# fetch-assets runs from apps/lotus-explore-rs/ so its relative asset
# directories land inside the app crate's public/ dir.
RUN cd apps/lotus-explore-rs && \
    cargo run --release -p lotus-web-assets --bin fetch-assets && \
    BROWSERSLIST='chrome >= 100, firefox >= 100, safari >= 15' dx build --release --platform web --base-path "${DX_BASE_PATH}" --package lotus-explore-rs --locked --debug-symbols=false --rustc-args=-Copt-level=z && \
    cd /build && cargo run --release -p lotus-web-assets --bin inject-wasm-preload

# ── Stage 3: export (for CI artifact extraction) ────────────────────────────────
# Exposes the built web bundle via a scratch image so CI can extract it with
# `docker buildx build --target export --output _final_site .` without needing
# a full runtime stage.
FROM scratch AS export
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public /
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public/index.html /404.html
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public/index.html /search/index.html
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public/index.html /curation/index.html
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public/index.html /draw/index.html

# ── Stage 4: runtime ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# ── OCI image labels ─────────────────────────────────────────────────────────
LABEL org.opencontainers.image.title="lotus-explore-rs" \
      org.opencontainers.image.description="LOTUS explorer API + client" \
      org.opencontainers.image.url="https://github.com/lotusnprod/lotus-explore-rs" \
      org.opencontainers.image.source="https://github.com/lotusnprod/lotus-explore-rs" \
      org.opencontainers.image.licenses="AGPL-3.0" \
      org.opencontainers.image.vendor="lotusnprod"

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Run as a non-root user — principle of least privilege
RUN adduser --system --no-create-home --uid 1001 appuser

# Copy server binary and WASM web bundle
COPY --from=builder /build/target/release/lotus-explore-rs /usr/local/bin/lotus-explore-rs
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public /app/public

# Bind to all interfaces by default when running in a container
ENV HOST=0.0.0.0
ENV PORT=8787
ENV PUBLIC_DIR=/app/public

EXPOSE 8787

USER appuser

# Liveness probe — requires the /health endpoint to respond
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider "http://localhost:${PORT}/health" || exit 1

CMD ["lotus-explore-rs"]
