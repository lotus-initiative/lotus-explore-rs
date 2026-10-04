# syntax=docker/dockerfile:1.7
#
# Two artefacts from one Dockerfile, because they are two ways of running the
# same code and they should not be able to drift:
#
#   target `runtime`  the `server` build: the HTTP API and the static web client
#                     in one image, which is what `compose.yaml` and the
#                     reverse-proxy deployment run.
#   target `cli`      the `lotus` binary, statically linked against musl and
#                     copied into distroless/cc. No libc, no shell, no package
#                     manager.
#   target `static`   the web client alone, served by an unprivileged nginx.
#                     For a deployment that already has its own reverse proxy
#                     and only wants the files.
#   target `export`   a scratch stage carrying just the built web bundle, which
#                     is how `deploy.yml` extracts it with
#                     `docker buildx build --target export --output _final_site`.
#                     That contract predates the stages above and is load
#                     bearing; do not rename it.
#
# Build args, all optional:
#   DX_BASE_PATH   the web bundle's base path. "/" for Docker and for a
#                  subdomain, "/<repo>" for a GitHub Pages project site.
#   CARGO_PROFILE  `debug` or `release`. The default is `release`, and there is
#                  no reason to build a debug image except to iterate on the
#                  Dockerfile itself.
#
# Every cargo invocation passes `--locked`: a silent dependency upgrade inside
# an image build produces an image that cannot be reproduced from its source.

# ── Shared build arguments ───────────────────────────────────────────────────
# Declared once and inherited by every stage via `ARG`s, so the toolchain
# version is written down exactly once in this file.

# 1.99.0 is the pin in rust-toolchain.toml. Reading it from here would be
# better; Docker has no way to read a file before the first FROM, so the one
# thing that has to be true of both is checked by `tests/gate_consistency.rs`.
ARG RUST_VERSION=1.99.0
# cargo-chef's version. Pinned because the `prepare`/`cook` output is a lockfile
# of recipes: a cargo-chef that formats a recipe differently invalidates every
# cached layer without the source having changed.
ARG CARGO_CHEF_VERSION=0.1.78

# A build platform, for a multi-arch build. `docker buildx build --platform`
# sets this automatically, and the musl target below is derived from it rather
# than hardcoded, so `--platform linux/arm64` produces an arm64 CLI without
# anyone editing this file.
FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-slim-trixie AS platform

# The `dpkg --print-architecture` spelling is the Debian one, and these are the
# Debian images. An unsupported architecture is a build failure here rather than
# a mystery further down.
RUN case "$(dpkg --print-architecture)" in \
      amd64) echo "x86_64-unknown-linux-musl" > /musl-target ;; \
      arm64) echo "aarch64-unknown-linux-musl" > /musl-target ;; \
      *) echo "unsupported architecture: $(dpkg --print-architecture)" >&2; exit 1 ;; \
    esac

# ── Stage: planner ───────────────────────────────────────────────────────────
# cargo-chef turns the dependency graph into a recipe file. This is what makes
# a source-only change rebuild the app in seconds instead of recompiling 200
# crates: the recipe depends on the manifests, so editing a .rs file leaves it
# unchanged and the next layer is a cache hit.
FROM rust:${RUST_VERSION}-slim-trixie AS planner

ARG CARGO_CHEF_VERSION

RUN cargo install cargo-chef --version ${CARGO_CHEF_VERSION} --locked

WORKDIR /build
# Every manifest, because `cargo chef prepare` resolves the whole workspace:
# a member whose manifest is missing is a workspace that does not resolve.
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/

RUN cargo chef prepare --recipe-path recipe.json

# ── Stage: chef (native dependencies) ─────────────────────────────────────────
# Native builds against the target, once, in its own layer. The rust image is
# slim and has no `libssl`, so this installs it; `--mount=type=cache` keeps the
# apt archive between builds, which is the layer that actually takes the time.
FROM rust:${RUST_VERSION}-slim-trixie AS chef

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# The binary from the planner, rather than a second `cargo install`: cargo-chef is
# a Rust program and compiling it from source is several minutes, in a layer that
# invalidates whenever cargo-chef itself is upgraded.
COPY --from=planner /usr/local/cargo/bin/cargo-chef /usr/local/cargo/bin/cargo-chef

WORKDIR /build
COPY --from=planner /build/recipe.json recipe.json

# `--release` because the release profile is what ships, and cooking a debug
# and a release graph is two caches to invalidate for every source change.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json

# ── Stage: builder (the server binary) ───────────────────────────────────────
FROM rust:${RUST_VERSION}-slim-trixie AS builder

# The musl target, derived rather than hardcoded: it has to match the
# architecture this stage is being built for, and hardcoding it is how an image
# ends up carrying a binary the kernel cannot execute.
COPY --from=platform /musl-target /tmp/musl-target

# `curl` is here because `utoipa-swagger-ui`'s build script downloads the Swagger
# UI assets at compile time and its only fallback is the `curl` binary. That is a
# build-time network fetch inside the image build, and it fails the build rather
# than producing a server with no docs endpoint -- which is the right way round,
# but it does mean the image build is not hermetic.
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl pkg-config libssl-dev musl-tools \
    && rm -rf /var/lib/apt/lists/*


WORKDIR /build
COPY --from=chef /build/target /build/target
COPY . .

# The two artefacts this Dockerfile ships.
#
#   `server`    the native binary: the HTTP API plus a static file server for
#               the web client. This is the whole runtime in one process.
#   `lotus`     the CLI, statically linked so the runtime stage can be
#               distroless/cc with no dynamic loader and no libc.
# The two musl artefacts land in /out, which has no architecture in its name, so
# the stages that copy them do not have to know which target produced them.
RUN musl=$(cat /tmp/musl-target) \
 && rustup target add "$musl" \
 && cargo build --release --locked --features server -p lotus-explore-rs \
 && mkdir -p /out \
 && cargo build --release --locked --target "$musl" \
      -p lotus-cli --bin lotus \
 && cp "target/$musl/release/lotus" /out/lotus \
 && cargo build --release --locked --target "$musl" \
      -p lotus-web-assets --bin lotus-healthcheck \
 && cp "target/$musl/release/lotus-healthcheck" /out/lotus-healthcheck

# ── Stage: wasm-builder (the web client) ─────────────────────────────────────
# On the same trixie base as the builder, for one concrete reason: the
# pre-built `dx` binary from Dioxus releases needs glibc >= 2.39, and Debian
# bookworm only has 2.36. Using the same tag as the builder is also what lets
# the two stages share a target directory.
FROM rust:${RUST_VERSION}-slim-trixie AS wasm-builder

ARG CARGO_CHEF_VERSION
# "/" for Docker and for a subdomain, "/<repo>" for a GitHub Pages project site.
ARG DX_BASE_PATH="/"
# The Dioxus version the crate depends on. Passed in rather than duplicated
# here, so the one value in Cargo.toml is the one that builds the bundle.
ARG DX_VERSION=0.7.10

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl gcc pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# The pre-built `dx` for this architecture, rather than `cargo install
# dioxus-cli` from source: the release binary is ~30 MB and the source build is
# several minutes of a layer that invalidates whenever dx is upgraded.
#
# The retry is for the same reason as the apt cache mount: a connection reset
# part way through a large download fails the whole image build for a reason
# that has nothing to do with the commit under test.
RUN arch=$(dpkg --print-architecture) && \
    case "$arch" in \
      amd64) dx_arch="x86_64" ;; \
      arm64) dx_arch="aarch64" ;; \
      *) echo "unsupported arch: $arch" >&2; exit 1 ;; \
    esac && \
    curl -fsSL --retry 5 --retry-all-errors --retry-delay 5 \
      -o /tmp/dx.tar.gz \
      "https://github.com/DioxusLabs/dioxus/releases/download/v${DX_VERSION}/dx-${dx_arch}-unknown-linux-gnu.tar.gz" && \
    tar -xzf /tmp/dx.tar.gz -C /usr/local/bin/ && \
    chmod +x /usr/local/bin/dx && \
    dx --version

WORKDIR /build
# Only the web bundle's dependency graph is cooked, and it is cooked for the
# wasm target. The native graph was already cooked by the `chef` stage.
COPY --from=chef /build/target /build/target
COPY . .

# Fetch the third-party assets (Ketcher, RDKit, citation.js -- ~115 MB) and
# build the wasm bundle.
#
# `fetch-assets` runs from the app's directory because it writes to
# `public/assets/` relative to where it runs, and the bundler only looks in the
# app's own. From the workspace root it downloads 115 MB into a `public/`
# nothing reads, and the build then succeeds with no assets in it at all.
#
# `fetch-assets` retries a transient failure inside one request. The retry
# around the whole command covers the failure it cannot see: a layer that starts
# and is cut off mid-download leaves a half-populated `public/assets/`, and
# `dx build` succeeds against that -- publishing a site with no structure editor
# rather than failing.
#
# `BROWSERSLIST` is the shipped floor. It is set here rather than left to
# autoprefixer because it changes what Tailwind emits: a wider floor means more
# vendor prefixes in the stylesheet every page loads.
#
# `-Copt-level=z` is asserted against `[profile.release]` and `[web.wasm_opt]`
# by `./mk opt-levels`, because `dx` passes `--rustc-args` last and would
# otherwise silently win over both.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cd apps/lotus-explore-rs && \
    for attempt in 1 2 3; do \
      cargo run --release --locked -p lotus-web-assets --bin fetch-assets && break; \
      echo "fetch-assets attempt $attempt failed; clearing the partial tree"; \
      rm -rf public/assets/ketcher public/assets/vendor; \
      [ "$attempt" = 3 ] && exit 1; \
      sleep $((attempt * 10)); \
    done && \
    test -f public/assets/ketcher/index.html && \
    test -f public/assets/vendor/rdkit/RDKit_minimal.wasm && \
    test -f public/assets/vendor/citation-js/citation.js && \
    BROWSERSLIST='chrome >= 100, firefox >= 100, safari >= 15' \
      dx build --release --platform web --base-path "${DX_BASE_PATH}" \
        --package lotus-explore-rs --locked --debug-symbols=false \
        --rustc-args=-Copt-level=z && \
    cd /build && \
    cargo run --release --locked -p lotus-web-assets --bin inject-wasm-preload

# ── Stage: export (the web bundle as a build output) ──────────────────────────
# `FROM scratch`, so `docker buildx build --target export --output _final_site .`
# writes the bundle to the host without an image. This is the stage
# `.github/workflows/deploy.yml` extracts the GitHub Pages site from.
FROM scratch AS export
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public /
# The SPA routes. dx serves the same document for each, and the client reads
# the path to decide what to render, so these are copies rather than redirects:
# a redirect would cost a round trip before the first paint.
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public/index.html /404.html
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public/index.html /search/index.html
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public/index.html /curation/index.html
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public/index.html /draw/index.html
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public/index.html /faq/index.html

# ── Stage: runtime (the server) ──────────────────────────────────────────────
# distroless/cc rather than debian-slim: this process serves a static directory
# and makes outbound HTTPS requests, so it needs a CA bundle and a TLS backend
# and nothing else. There is no shell in the image, so `--read-only` plus
# `--security-opt no-new-privileges` is not a hardening measure here -- it is the
# only configuration the image can run in.
#
# `debian13` and not `debian12`, and the reason is specific: the builder is
# `rust:*-slim-trixie`, so the binary is linked against glibc 2.40, and
# `debian12` provides 2.36. The image does not start -- with
#
#   /usr/local/bin/lotus-explore-rs: /lib/aarch64-linux-gnu/libc.so.6:
#   version `GLIBC_2.38' not found
#
# The two must agree on the glibc the binary is built against. This is the same
# constraint that already forces the wasm stage onto trixie for `dx`, arrived at
# a second time and from the other direction.
FROM gcr.io/distroless/cc-debian13:nonroot AS runtime

ARG RUST_VERSION
ARG VCS_REF=unknown

LABEL org.opencontainers.image.title="lotus-explore-rs" \
      org.opencontainers.image.description="LOTUS explorer: the query API and the web client" \
      org.opencontainers.image.url="https://github.com/lotusnprod/lotus-explore-rs" \
      org.opencontainers.image.source="https://github.com/lotusnprod/lotus-explore-rs" \
      org.opencontainers.image.licenses="AGPL-3.0-only" \
      org.opencontainers.image.vendor="lotusnprod" \
      org.opencontainers.image.revision="${VCS_REF}" \
      org.opencontainers.image.version="0.1.0"

COPY --from=builder /build/target/release/lotus-explore-rs /usr/local/bin/lotus-explore-rs
COPY --from=builder /out/lotus-healthcheck /usr/local/bin/lotus-healthcheck
COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public /app/public

# In a container the server has to bind all interfaces to be reachable at all;
# on a bare host `127.0.0.1` is the safer default and this is the line that
# changes.
ENV HOST=0.0.0.0 \
    PORT=8787 \
    PUBLIC_DIR=/app/public \
    RUST_LOG=info

EXPOSE 8787

# The probe is a binary, not `wget` or `curl`: distroless has no shell and no
# HTTP client, so the usual `HEALTHCHECK CMD wget --spider ...` cannot run in
# this image. `lotus-healthcheck` is a dependency-free static probe built from
# this repository for exactly that, and it is the reason this image can be
# distroless and still tell an orchestrator whether it is serving.
#
# The defaults inside it are the container's own configuration, so the line below
# passes no arguments and cannot drift from what the server was told to listen
# on.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/lotus-healthcheck", "127.0.0.1", "8787", "/health"]

USER nonroot:nonroot

ENTRYPOINT ["/usr/local/bin/lotus-explore-rs"]

# ── Stage: static (the web client alone, on an unprivileged web server) ───────
# For a deployment that already has a reverse proxy in front and wants only the
# files. nginx-unprivileged listens on 8080 as uid 101, which is the only
# configuration of the nginx image that does not need `--cap-drop` gymnastics.
FROM nginxinc/nginx-unprivileged:1.29-alpine AS static

ARG VCS_REF=unknown

LABEL org.opencontainers.image.title="lotus-explore-rs-web" \
      org.opencontainers.image.description="The LOTUS explorer web client, without the API" \
      org.opencontainers.image.source="https://github.com/lotusnprod/lotus-explore-rs" \
      org.opencontainers.image.licenses="AGPL-3.0-only" \
      org.opencontainers.image.revision="${VCS_REF}"

COPY --from=wasm-builder /build/target/dx/lotus-explore-rs/release/web/public /usr/share/nginx/html
COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY docker/snippets/security-headers.conf /etc/nginx/snippets/security-headers.conf

EXPOSE 8080

# ── Stage: cli (the lotus binary, statically linked) ─────────────────────────
# The CLI is the other half of this repository: the same query the web app runs
# is the query `lotus search` runs. A statically linked musl build into
# distroless/cc means no libc to keep patched and no loader to be found at
# runtime, which is the whole reason for choosing musl over the default
# glibc target.
FROM gcr.io/distroless/cc-debian12:nonroot AS cli

ARG VCS_REF=unknown

LABEL org.opencontainers.image.title="lotus" \
      org.opencontainers.image.description="Search LOTUS from a terminal, and export the result" \
      org.opencontainers.image.source="https://github.com/lotusnprod/lotus-explore-rs" \
      org.opencontainers.image.licenses="AGPL-3.0-only" \
      org.opencontainers.image.revision="${VCS_REF}"

COPY --from=builder /out/lotus /usr/local/bin/lotus

USER nonroot:nonroot

# No ENTRYPOINT: the CLI takes subcommands and flags, and an entrypoint would
# have to be a shell script to forward them. The image is run as
# `docker run --rm lotus search --taxon "Gentiana lutea"`.
CMD ["lotus", "--help"]
