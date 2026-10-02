# lotus-explore-rs

A linked open data explorer for the [LOTUS](https://doi.org/10.7554/eLife.70780) compound-taxon-reference
knowledge graph from Wikidata, queried over SPARQL.

The search, query-building and curation logic lives in the `lotus-*` crates, which the web app,
the `lotus` CLI and the tests all share. A query the explorer runs and a query the CLI runs are
the same query.

## Quick start

```bash
cd apps/lotus-explore-rs
cargo run -p lotus-web-assets --bin fetch-assets
dx serve --platform web --package lotus-explore-rs --locked
```

That is a development server. For the release build, the optional HTTP API, and how to serve the
output the way the deploy does, see [`apps/lotus-explore-rs/README.md`](apps/lotus-explore-rs/README.md).

For the terminal instead:

```bash
cargo install --path crates/lotus-cli
lotus search --taxon "Gentiana lutea" --format csv
```

## The crates

| crate | what it is |
| --- | --- |
| `lotus-model` | the vocabulary: a filter set, a result row, what makes a filter active |
| `lotus-query` | SPARQL construction and CSV parsing. Pure. |
| `lotus-search` | the search use case: which endpoint, when to fall back, taxon resolution |
| `lotus-curation` | the curation vocabulary and the statements it produces |
| `lotus-jsonld` | Bioschemas JSON-LD, CodeMeta, `CITATION.cff` |
| `lotus-cli` | the `lotus` binary |
| `lotus-web-assets` | host-only fetcher for the third-party frontend assets |
| `apps/lotus-explore-rs` | the app: WASM client, optional native server, optional desktop window |

Each library crate has a README that is compiled as its documentation, so the example in it is
checked as a doctest. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) says why the boundaries
are where they are.

## Prerequisites

Rust 1.99.0 with `clippy`, `rustfmt` and `wasm32-unknown-unknown`, pinned in
`rust-toolchain.toml`. Everything else is installed for you:

```bash
./mk setup
```

That installs the test runner, the task runner, the linters and the supply-chain
tools, each at a pinned version. `dx` for the web app comes from it too. Node.js
is not needed, because Dioxus builds the Tailwind itself.

## Working on it

`./mk ci` is every check CI runs, in order. It needs no network access.

```bash
./mk ci       # formatting, clippy, tests, docs, wasm, supply chain
./mk mutants  # mutation testing: a passing test run cannot tell you this
```

`./mk --list-all-steps` is the list of tasks. It is the authoritative one:
a copy of it in this file would be a copy that goes stale.

`./mk` is a script at the repository root and the only command to learn. It runs
`cargo make --no-workspace`, and the flag is the point: without it cargo-make
re-runs each task once per crate, so `./mk ci` is 71s and a bare `cargo make ci`
is 578s — the same checks, eight times over. The comment in `mk` has the detail.

[`CONTRIBUTING.md`](CONTRIBUTING.md) has the full list, where a change belongs, and how to
refresh the fixtures.

## Documentation

- [`docs/cli.md`](docs/cli.md) — the `lotus` command, every flag, checked against `--help` by a test
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — the crates and their boundaries
- [`docs/TAXON-SEARCH.md`](docs/TAXON-SEARCH.md) — how a taxon name becomes a QID, and the four nomenclatural relationships
- [`docs/STRUCTURE-SEARCH.md`](docs/STRUCTURE-SEARCH.md) — how a name, an InChIKey or a QID becomes a compound identity
- [`docs/FRONTENDS.md`](docs/FRONTENDS.md) — web, desktop and CLI: how each is built
- [`apps/lotus-explore-rs/docs/`](apps/lotus-explore-rs/docs/) — design system, performance, deployment

## License

`AGPL-3.0-only`. See [`LICENSE`](LICENSE).
