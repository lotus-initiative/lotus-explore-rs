# Contributing

## Before you start

Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) first. The workspace is
arranged so that a change usually has an obvious home, and that arrangement is
load-bearing: the web app and the CLI share their query building and their
curation vocabulary precisely so that a query the explorer runs and a query
`lotus search` runs are the same query.

## 5-minute onboarding: inspect a query and run one test

You do not need to launch the web server, download frontend assets, or touch the
network to see what the codebase does or to test a change:

```bash
# 1. Print the exact SPARQL query that a search builds (zero network access).
#    A taxon is given by its Wikidata QID here (Q158572 is Gentiana lutea):
#    turning a name into a QID needs the network, so --explain refuses names.
cargo run -p lotus-cli -- search --taxon Q158572 --explain

# 2. Run a single unit or contract test by substring:
./mk test-fast -E 'test(exact_compound)'

# 3. Run the fast inner check (rustfmt, tombi, typos, clippy):
./mk check
```

Because both the CLI and the web app call `lotus-query` and `lotus-search`,
`cargo run -p lotus-cli -- search ... --explain` is the fastest way to inspect
the SPARQL produced for any combination of filters.

## Install prerequisites

Install [Rust via rustup](https://rustup.rs/) and ensure `cargo` and `rustup`
are on your `PATH`. `./mk setup` installs the toolchain and targets pinned in
`rust-toolchain.toml`, plus the tools pinned in `mk.versions` and `mk.local`.
It **requires `cargo-binstall` first**; it cannot install that bootstrap tool
itself.

On macOS, install `cargo-binstall` with Homebrew:

```bash
brew install cargo-binstall
./mk setup
```

Without Homebrew (also works on Linux), build the bootstrap tool from source:

```bash
cargo install cargo-binstall --locked
./mk setup
```


On Debian/Ubuntu, install the native dependencies before `./mk setup`:

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev libxdo-dev
```

These six packages are needed for the desktop build. `./mk setup` installs apt
packages automatically only on CI's Linux runners, not on a local machine.
`./mk setup-extra` is optional and adds development utilities outside the
required checks.

## Running the checks

Use the shortest check that covers the change, then run the full gate before
pushing:

```bash
./mk check                              # Rust/TOML format, TOML schema, typos, Clippy; no tests
./mk test-fast -E 'test(name)'          # one matching unit/integration test
./mk gate                               # formatting, lint, tests, wasm type-checks; no mutation run
./mk ci                                 # complete CI, including release wasm build and mutation gate
```

`./mk test-fast` without `-E` runs the unit/integration suite. `./mk gate`
omits the slow release wasm bundle and other full-CI gates; it is a faster
pre-push pass, not a substitute for `./mk ci`.

`cargo fmt` formats Rust only. **Tombi formats and lints `*.toml`**, including
the order of dependency keys in `Cargo.toml`. If the `tombi` gate fails, format
the affected file and rerun just that gate:

```bash
tombi format crates/lotus-cli/Cargo.toml  # replace with the reported path
./mk tombi
```

`./mk fix` applies Rust and TOML formatting, spelling fixes and Clippy fixes
across the workspace; review its changes before committing.

`./mk ci` is exactly what the CI workflow runs — the workflow calls this
one command, so a check that is green locally and red in a pipeline cannot happen
by drift. It covers formatting (rustfmt and tombi), compilation, clippy with
warnings denied, tests, docs, stale references (`./mk refquiem`), SPDX headers,
citation metadata, the wasm builds, and the supply-chain checks. It needs no
network access.

`./mk --list-all-steps` is the list of tasks. It is the authoritative one;
the summary above is not, because a copied list is a list that goes stale.

If it is slow, `./mk test` and `./mk lint` are the two that matter
most. The git hooks run the same tasks before a push — see `prek.toml`, which
delegates to them rather than repeating the cargo flags — so most failures are
caught before they reach CI.

### Tests run under nextest, and the doctests separately

The test suite runs under [`cargo-nextest`](https://nexte.st), which runs each
test in its own process and in parallel; it is roughly twice as fast as
`cargo test` on this workspace. nextest does **not** run doctests, so
`./mk test` runs the two in sequence, and so does every other place tests
run:

```bash
cargo nextest run --workspace          # the suite
cargo test --workspace --doc           # the doctests nextest skipped
```

The second command is not optional housekeeping. Each library crate's `README.md`
is its crate documentation via `#![doc = include_str!]`, and the example in it is
a doctest — so a dropped `cargo test --doc` leaves every README unchecked and
nothing notices.

Everything except the wasm builds runs with **no network access**, and that is a
property worth preserving. The search use case is tested through the `Http`
trait against a scripted conversation, so the QLever-to-WDQS fallback and the
retry policy are both covered without touching the network. If you find yourself
wanting to make a test hit a real endpoint, the thing to reach for is a fixture.

```bash
./mk metadata-write   # regenerate codemeta.json and CITATION.cff
```

### The one thing that does need the network

`./mk desktop` and the deploy fetch ~115 MB of third-party frontend assets —
Ketcher, RDKit, Citation.js — from four hosts. That is the only part of the gate
that reaches out, and it has been the source of intermittent failures: a single
reset connection failed a build that had nothing wrong with the commit.

It is now retried at two levels, and the reason is worth knowing before you
change either one. `fetch-assets` retries each request three times with a
doubling wait, so a blip inside a request is invisible. The `desktop` job and
the Dockerfile then retry the whole run and clear the partial tree first, for the
failure that retry cannot see: a run cut off part way through leaves a
half-populated `public/assets/`, and `dx build` succeeds against that — which
publishes a site with no structure editor rather than failing.

If you touch asset fetching, keep both levels and keep the `test -f` checks that
follow them.

## Where a change goes

| You are changing                                                           | It belongs in                                              |
| -------------------------------------------------------------------------- | ---------------------------------------------------------- |
| A domain type, a filter rule, validation                                   | [`crates/lotus-model/`](crates/lotus-model/)               |
| A SPARQL string, a CSV column, an export format                            | [`crates/lotus-query/`](crates/lotus-query/)               |
| Which endpoint, when to retry, taxon/compound/reference resolution         | [`crates/lotus-search/`](crates/lotus-search/)             |
| A curation row, a statement, the TSV reader                                | [`crates/lotus-curation/`](crates/lotus-curation/)         |
| JSON-LD, CodeMeta, CITATION                                                | [`crates/lotus-jsonld/`](crates/lotus-jsonld/)             |
| Framework-free state reducers, URL parameters, client-side filter/sort     | [`crates/lotus-explore-core/`](crates/lotus-explore-core/) |
| A CLI flag, an output format, an exit code                                 | [`crates/lotus-cli/`](crates/lotus-cli/)                   |
| How many rows this machine renders, the clock, cache keys, Dioxus UI       | [`apps/lotus-explore-rs/`](apps/lotus-explore-rs/)         |

The rule behind the table is **a crate that can be pure is pure**: no HTTP, no
clock, no async runtime in the four pure library crates (`lotus-model`,
`lotus-query`, `lotus-curation`, `lotus-jsonld`), checked in CI by building
them for `wasm32` in their own right. If you want a clock in one of them, the
year is meant to be an argument. The reasoning is in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

### Symptom → Exact file & function

When hunting a bug or adding a feature, start at the exact file that owns that
stage of the pipeline:

| If you are looking for… | Open this file | Key function / type |
| --- | --- | --- |
| The 6 base 15-column `SELECT` shapes & shared triple blocks | [`crates/lotus-query/src/query.rs`](crates/lotus-query/src/query.rs) | `COMPOUND_PROPERTIES`, `REFERENCE_METADATA`, `exact_compounds_query`, `structure_search_query_with`, `compounds_by_taxon_query_with`, `all_compounds_query`, `all_compounds_including_untaxonomised_query` |
| How `SearchCriteria` (mass, year, formula, reference QID) becomes SPARQL | [`crates/lotus-query/src/query.rs`](crates/lotus-query/src/query.rs) | `with_filters`, `filter_clauses` |
| How the 4-column `(?compound ?taxon ?taxon_name ?r)` fast query and `COUNT(DISTINCT ...)` query strip heavy optional joins | [`crates/lotus-query/src/query.rs`](crates/lotus-query/src/query.rs) | `occurrences_query`, `counts_query`, `limit_query`, `construct_from_select`, `wdqs_fallback` |
| How per-entity metadata (labels, SMILES, taxonomy, DOIs) is fetched in batches for the split-stream pipeline | [`crates/lotus-query/src/query.rs`](crates/lotus-query/src/query.rs) | `dictionary_queries`, `entity_dictionary_query`, `compound_dictionary_query`, `reference_dictionary_query` |
| How a compound name, InChIKey, QID, taxon name, or DOI is resolved before the main query | [`crates/lotus-query/src/query.rs`](crates/lotus-query/src/query.rs) and [`crates/lotus-search/src/search.rs`](crates/lotus-search/src/search.rs) | `compound_inchikey_query`, `compound_label_query`, `taxon_lookup_query`, `resolve_taxon`, `resolve_structure`, `resolve_reference` |
| How taxon synonyms and nomenclature work across GBIF/OTT/NCBI/iNaturalist/WFO | [`crates/lotus-query/src/query.rs`](crates/lotus-query/src/query.rs) | `SYNONYM_PATH`, `taxon_compounds_query_body` |
| How CSV, SPARQL JSON, and RDF/Turtle downloads and bulk exports are formatted | [`crates/lotus-query/src/export_rows.rs`](crates/lotus-query/src/export_rows.rs) and [`crates/lotus-query/src/bulk_export.rs`](crates/lotus-query/src/bulk_export.rs) | `RowExporter`, `ExportFormat` |
| How URL query parameters (`?taxon=...&structure=...`) are parsed and built | [`crates/lotus-explore-core/src/explore/url_codec/criteria.rs`](crates/lotus-explore-core/src/explore/url_codec/criteria.rs) | `CriteriaQueryDto` |
| How the web app's state machine transitions during search, streaming, filtering, and sorting | [`crates/lotus-explore-core/src/explore/search_state/`](crates/lotus-explore-core/src/explore/search_state/) | `ExploreState`, `ExploreAction`, `reduce_mut` |
| How the fetched table is filtered and sorted client-side over `ColumnarResultSet` | [`crates/lotus-explore-core/src/filters.rs`](crates/lotus-explore-core/src/filters.rs) and [`crates/lotus-explore-core/src/sort.rs`](crates/lotus-explore-core/src/sort.rs) | `ColumnFilters`, `SortColumn`, `SortState` |
| How clicking "Search" in the web UI orchestrates resolution, streaming, and progressive counts | [`apps/lotus-explore-rs/src/features/explore/`](apps/lotus-explore-rs/src/features/explore/) | `SearchTaskController` ([`orchestrator/controller.rs`](apps/lotus-explore-rs/src/features/explore/orchestrator/controller.rs)), `start_search` ([`orchestrator/runtime.rs`](apps/lotus-explore-rs/src/features/explore/orchestrator/runtime.rs)), `do_search` ([`executor.rs`](apps/lotus-explore-rs/src/features/explore/executor.rs)), `build_execution_plan` ([`service/results_pipeline/plan.rs`](apps/lotus-explore-rs/src/features/explore/service/results_pipeline/plan.rs)), `fetch_results` ([`service/fetch_results/`](apps/lotus-explore-rs/src/features/explore/service/fetch_results/)) |
| How the virtualized results table renders | [`apps/lotus-explore-rs/src/components/results_table.rs`](apps/lotus-explore-rs/src/components/results_table.rs) and [`apps/lotus-explore-rs/src/components/results_table/`](apps/lotus-explore-rs/src/components/results_table/) | [`virtualized_table.rs`](apps/lotus-explore-rs/src/components/results_table/virtualized_table.rs), [`row_cells.rs`](apps/lotus-explore-rs/src/components/results_table/row_cells.rs) |

## Two things that will bite you

**`SearchCriteria` has no `Default`.** "No filters" is not a value, it is a
value _plus a year_, so the constructor is `SearchCriteria::up_to_year(year)`.
Use the year your caller has.

**Name the crate, not a shim.** There are no `models.rs` or `queries.rs`
re-exports. `lotus_model::SearchCriteria` at a call site says where the type
lives; `crate::models::SearchCriteria` says only that the app once had a file
called `models.rs`.

**Half a serialisation contract is worse than none of it.** The URL parameter
names used to be built by a model method and parsed by a DTO in a different
crate, and a rename broke every link already in a browser's history with nothing
failing. Both halves now live in `CriteriaQueryDto`, and there is a round-trip
test to keep them together. If you add a parameter, add it there.

## Tests

- A test that would pass without the change is not a test. The most useful ones
  here assert a specific value: the exact SPARQL, the exact statements, the
  exact bytes.
- Prefer a fixture over a mock where a fixture will do.
- When behaviour changes on purpose, say in the commit message what the old
  behaviour was. `git log` is the only record of why a test asserts what it
  does.
- `docs/cli.md` is checked against `--help` in both directions and every
  documented example is run with `--explain`. If you change a flag, that test
  tells you.

## Headers

Every `.rs` file starts with exactly:

```rust
// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
```

`./mk license-headers` and CI both enforce it. Run it rather than fixing by
hand after the fact.

## Style

Match the file you are in. The conventions this repository holds to:

- Panics and `expect` are for tests and for genuinely unreachable states, never
  for input from a user, an endpoint, or a file. The lints are on.
- Comments explain **why**, and especially why the obvious thing is wrong. If a
  line needs a comment saying what it does, the line wants renaming instead.
- Prefer returning a value to mutating one, and an iterator to an index.
- A public item gets a doc comment; `#![warn(missing_docs)]` is on the library
  crates.
