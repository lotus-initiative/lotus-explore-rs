# lotus-explore-rs

[![CI](https://github.com/lotusnprod/lotus-explore-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/lotusnprod/lotus-explore-rs/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/lotusnprod/lotus-explore-rs/graph/badge.svg)](https://codecov.io/gh/lotusnprod/lotus-explore-rs)

A linked open data explorer for the [LOTUS](https://doi.org/10.7554/eLife.70780) compound-taxon-reference
knowledge graph from Wikidata, queried over SPARQL.

The search, query-building and curation logic lives in the `lotus-*` crates, which the web app,
the `lotus` CLI and the tests all share. A query the explorer runs and a query the CLI runs are
the same query.

To build it, run the checks, or send a change, see [`CONTRIBUTING.md`](CONTRIBUTING.md).

## CLI tools for full-graph operations

The CLI has three bulk-data subcommands that fetch the entire LOTUS graph, not
a filtered result set:

- `lotus export` — the full compound–taxon–reference graph as CSV (held in memory while it is joined; see [`docs/cli.md`](docs/cli.md))
- `lotus export-pubchem-tree` — three PubChem-format JSON trees
- `lotus archive` — InChIKey-to-SMILES mappings, deduplicated, tar+gzip packed, with optional Zenodo publishing (dry-run by default)

## Quick start

```bash
cargo install --path crates/lotus-cli
lotus search --taxon "Gentiana lutea" --format csv
```

For the web explorer, the release build, the optional HTTP API, and how to serve
the output the way the deploy does, see
[`apps/lotus-explore-rs/README.md`](apps/lotus-explore-rs/README.md).

## Architecture at a glance

```text
┌─────────────────────────────────────────────────────────────────────────┐
│ Front ends & build tools                                                │
│   apps/lotus-explore-rs (Web WASM / Desktop / Axum HTTP API server)     │
│   crates/lotus-cli      (`lotus` CLI: search, curate, export, archive)  │
│   crates/lotus-web-assets (build-time fetcher for RDKit / Ketcher)      │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────────┐
│ Use-case & framework-free application core (injectable `Http`, no UI)   │
│   crates/lotus-explore-core (Explore state machine, URL codec, client   │
│                              table filter/sort, bulk export pipelines)  │
│   crates/lotus-search       (SPARQL execution, QLever→WDQS fallback,    │
│                              split-stream fold + dictionary enrichment) │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────────┐
│ Pure domain & query crates (`wasm32`-checked, zero I/O, no clock)       │
│   crates/lotus-query    (SPARQL query builders, rewriters, CSV/RDF I/O) │
│   crates/lotus-model    (SearchCriteria, ColumnarResultSet, validators) │
│   crates/lotus-curation (TSV reader, Wikidata lookup, QuickStatements)  │
│   crates/lotus-jsonld   (Bioschemas JSON-LD, CodeMeta, CITATION.cff)    │
└─────────────────────────────────────────────────────────────────────────┘
```

## The crates

| crate                                                       | what it is                                                                                     |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| [`lotus-model`](crates/lotus-model/README.md)               | Domain vocabulary: `SearchCriteria`, `ColumnarResultSet`, structure/identifier classifiers     |
| [`lotus-query`](crates/lotus-query/README.md)               | Pure SPARQL construction, query rewriters (`counts_query`, `occurrences_query`), CSV/RDF I/O   |
| [`lotus-search`](crates/lotus-search/README.md)             | Search execution: taxon/compound/DOI resolution, retry, QLever→WDQS fallback, streaming fold   |
| [`lotus-curation`](crates/lotus-curation/README.md)         | Curation vocabulary, TSV reader, Wikidata reconciliation queries, QuickStatements V1 generator |
| [`lotus-jsonld`](crates/lotus-jsonld/README.md)             | Bioschemas JSON-LD, CodeMeta, `CITATION.cff`                                                   |
| [`lotus-explore-core`](crates/lotus-explore-core/README.md) | Framework-free app core: state reducers, URL codec, table filter/sort, bulk export pipelines   |
| [`lotus-cli`](crates/lotus-cli/README.md)                   | The `lotus` binary (`search`, `curate`, `export`, `export-pubchem-tree`, `archive`)            |
| `lotus-web-assets`                                          | Host-only fetcher for third-party frontend assets (Ketcher, RDKit, Citation.js)                |
| [`apps/lotus-explore-rs`](apps/lotus-explore-rs/README.md)  | Dioxus web client (WASM), optional Axum HTTP API (`--features server`), optional desktop app   |

Each library crate has a README that is compiled as its documentation, so the example in it is
checked as a doctest. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) explains the layer boundaries,
the end-to-end request lifecycle, and the module map inside each crate;
[`CONTRIBUTING.md`](CONTRIBUTING.md) has a symptom-to-file navigation table for common changes and bugs.

## Prerequisites

Install Rust via rustup, then bootstrap `cargo-binstall` before running setup.
The exact macOS and Debian/Ubuntu commands are in
[`CONTRIBUTING.md`](CONTRIBUTING.md#install-prerequisites); `./mk setup` alone
cannot install its own bootstrap tool.

```bash
./mk setup
```

Setup installs the pinned Rust toolchain (with `clippy`, `rustfmt` and
`wasm32-unknown-unknown`), task runner, test runner, linters, supply-chain
tools and `dx`. Node.js is not needed: Dioxus builds the Tailwind itself.

## Working on it

```bash
./mk ci       # formatting, clippy, tests, docs, stale references, wasm, supply chain
./mk mutants  # mutation testing: a passing test run cannot tell you this
```

`./mk ci` is every check CI runs, in order, and needs no network access.
`./mk --list-all-steps` is the list of tasks and is the authoritative one: a copy
of it in this file would be a copy that goes stale.

`./mk` is a script at the repository root and the only command to learn. It runs
`cargo make --no-workspace`, and the flag is the point: without it cargo-make
re-runs each task once per crate, so `./mk ci` is 71s and a bare `cargo make ci`
is 578s — the same checks, eight times over. The comment in `mk` has the detail.

[`CONTRIBUTING.md`](CONTRIBUTING.md) has where a change belongs, the test
conventions, and how to refresh the fixtures.

## Documentation

- [`CHANGELOG.md`](CHANGELOG.md) — what changed in each release
- [`docs/cli.md`](docs/cli.md) — the `lotus` command, every flag, checked against `--help` by a test
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — the crates and their boundaries
- [`docs/SHARING_PLAN.md`](docs/SHARING_PLAN.md) — how the export/archive/pubchem-tree ports share code
- [`docs/TAXON-SEARCH.md`](docs/TAXON-SEARCH.md) — how a taxon name becomes a QID, and the four nomenclatural relationships
- [`docs/STRUCTURE-SEARCH.md`](docs/STRUCTURE-SEARCH.md) — how a name, an InChIKey or a QID becomes a compound identity
- [`docs/FRONTENDS.md`](docs/FRONTENDS.md) — web, desktop and CLI: how each is built
- [`docs/QLEVER-POLITENESS.md`](docs/QLEVER-POLITENESS.md) — what a QLever 429 actually means, and how this project stays inside its budget
- [`apps/lotus-explore-rs/docs/`](apps/lotus-explore-rs/docs/) — design system, performance, deployment

## License

`AGPL-3.0-only`. See [`LICENSE`](LICENSE).
