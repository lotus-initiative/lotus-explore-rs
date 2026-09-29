# lotus-explore-rs

[![AGPL-3.0
license](https://img.shields.io/badge/License-AGPL%203.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0.html)
[![Tests](https://img.shields.io/badge/tests-passing-brightgreen)](https://github.com/lotusnprod/lotus-explore-rs/actions)

`lotus-explore-rs` --- LOTUS Explorer.

A linked open data (LOD) explorer for the LOTUS compound-taxon-reference
knowledge graph from Wikidata, queried over SPARQL. The search, query-building
and curation logic lives in the `lotus-*` crates below, which the web app, the
`lotus` CLI and the tests all share, so a query the explorer runs and a query
the CLI runs are the same query.

## Quick start

```bash
just serve
```

This is a development server: it serves an unhashed bundle plus the Dioxus JS
interpreter, and it is **not** what ships. Do not run Lighthouse or measure
transfer size against it. For anything you intend to publish, build first and
serve the output:

```bash
just build
```

That writes the real bundle to `target/dx/lotus-explore-rs/release/web/public`,
which is what the deploy publishes. The module there is 1.4 MiB raw / 456 KiB
brotli, against the dev server's 6.4 MiB.

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
│   ├── lotus-model/          ← Domain types, filter semantics, validation
│   ├── lotus-query/          ← SPARQL construction and CSV parsing (pure)
│   ├── lotus-search/         ← The search use case: resolve, run, fall back
│   ├── lotus-curation/       ← The curation vocabulary and QuickStatements
│   ├── lotus-jsonld/         ← Bioschemas JSON-LD, CodeMeta, CITATION.cff
│   ├── lotus-cli/            ← The `lotus` binary
│   └── lotus-web-assets/     ← Host-only frontend asset fetcher
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
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) --- the crates, and why the
  boundary is where it is
- [`CONTRIBUTING.md`](CONTRIBUTING.md) --- how to build, test, and where a change
  belongs
- [`docs/ARCHITECTURE.md`](apps/lotus-explore-rs/docs/ARCHITECTURE.md) --- the
  application side
- [`docs/cli.md`](docs/cli.md) --- the `lotus` command, kept honest against
  `--help` by a test

## Continuous integration

On every push to `main`:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`
- `cargo test -p lotus-explore-rs --features server --locked`
- The three pure crates built for `wasm32` on their own
- `codemeta.json` and `CITATION.cff` checked against the code
- WASM build and deploy to GitHub Pages

## License

`AGPL-3.0-only` --- see [`LICENSE`](https://www.gnu.org/licenses/agpl-3.0.html)
for details.
