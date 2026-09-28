# lotus-explore-rs

[![AGPL-3.0
license](https://img.shields.io/badge/License-AGPL%203.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0.html)
[![Tests](https://img.shields.io/badge/tests-passing-brightgreen)](https://github.com/lotusnprod/lotus-explore-rs/actions)

`lotus-explore-rs` --- LOTUS Explorer.

A linked open data (LOD) explorer for the LOTUS compound-taxon-reference
knowledge graph from Wikidata, queried via SPARQL. Powered by the `lotus` shared
crate and the QLever SPARQL endpoint.

## Quick start

```bash
just serve
```

To also run the optional API:

```bash
cargo run --locked --features server -p lotus-explore-rs
```

Then open `http://localhost:8080/?api_base=http://127.0.0.1:8787`.

Without the server, the explorer falls back to direct QLever/SPARQL queries.

## Structure

```
lotus-explore-rs/
├── Cargo.toml                ← workspace root
├── rust-toolchain.toml       ← pinned compiler, components, target
├── crates/                   ← shared library crates
│   ├── lotus/                ← SPARQL client, LOTUS models, transport, export
│   └── lotus-deploy/         ← Host-only frontend asset fetcher
├── apps/                     ← application crates
│   └── lotus-explore-rs/     ← Main app: WASM client + optional native server
│       ├── Cargo.toml
│       ├── Dioxus.toml       ← Dioxus CLI config
│       ├── build.rs          ← Generates metadata files (llms.txt, robots.txt, etc.)
│       ├── index.html
│       ├── tailwind/
│       │   └── styles.css    ← Tailwind input
│       ├── public/           ← Static assets (favicons, site.webmanifest, etc.)
│       │   └── assets/
│       │       └── lotus-explore.css  ← Compiled Tailwind CSS
│       └── src/
│           ├── main.rs
│           ├── document_head.rs
│           ├── app/
│           ├── components/
│           ├── features/
│           ├── server/
│           ├── state/
│           ├── ui/
│           └── utils/
```

## Prerequisites

The repo pins Rust 1.97, `clippy`, `rustfmt`, and `wasm32-unknown-unknown` in
`rust-toolchain.toml`. Running any `cargo` command will auto-download the pinned
toolchain via `rustup`.

The repository commands use the `just` task runner; install it with your
platform's package manager.

To serve or build the WASM app, also install the Dioxus CLI:

```bash
cargo install dioxus-cli --version 0.7.10 --locked
```

Dioxus 0.7.10 builds and watches Tailwind automatically during `dx serve` and
`dx build`, so Node.js and npm are not required for local development or release
builds.

## Documentation

- [`docs/DESIGN_SYSTEM.md`](apps/lotus-explore-rs/docs/DESIGN_SYSTEM.md) --- the
  four shell planes, their measured separation, and the border rules
- [`docs/PERFORMANCE.md`](apps/lotus-explore-rs/docs/PERFORMANCE.md) --- where
  load time goes, and the profile experiments that were kept and rejected
- [`docs/DEPLOYMENT.md`](apps/lotus-explore-rs/docs/DEPLOYMENT.md) --- what the
  production host actually serves, and how to measure it locally
- [`docs/ARCHITECTURE.md`](apps/lotus-explore-rs/docs/ARCHITECTURE.md)

## Continuous integration

On every push to `main`:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`
- `cargo test -p lotus-explore-rs --features server --locked`
- WASM build and deploy to GitHub Pages

## License

`AGPL-3.0-only` --- see [`LICENSE`](https://www.gnu.org/licenses/agpl-3.0.html)
for details.
