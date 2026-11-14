# AGENTS.md — lotus-explore-rs

Explorer and curation tool for the LOTUS compound–taxon–reference graph in
Wikidata, queried over SPARQL (QLever, falling back to WDQS). Rust workspace:
pure crates, a CLI, and one app crate (wasm web client, native API server,
desktop window). Human docs: `README.md`, `CONTRIBUTING.md`, `docs/`.

Read `.github/ai/PITFALLS.md` before changing curation, query building, search
retries, the server, or exports. Each entry there was a shipped bug.

## Commands

- `./mk setup` — install pinned tools (once). `./mk setup-extra` adds `dx`.
- `./mk check` — fmt, tombi, typos, clippy. Seconds when warm.
- `./mk test-fast -E 'test(name)'` — one test. `./mk test-fast` — all, nextest.
- `./mk test` — nextest over default, `server` and `desktop` builds, plus doctests.
- `./mk gate` — pre-push: everything above plus licence headers and wasm32 checks.
- `./mk ci` — full gate, what CI runs. `./mk --list` for every task.
- `./mk local:metadata` checks `CITATION.cff`/`codemeta.json`;
  `./mk metadata-write` regenerates them from `crates/lotus-jsonld`.
- `cargo run -p lotus-cli -- search --taxon Q158572 --explain` — print a query, no network.

Cold `./mk gate` is about 5 minutes. Cargo must be able to write `~/.cargo`.

## Verify cheaply, in this order

1. `cargo clippy -p <crate> --all-targets` (add `--features server`, or
   `--target wasm32-unknown-unknown`, when the code is behind them).
2. `cargo nextest run -p <crate> -E 'test(<name>)'`.
3. `cargo nextest run --workspace`.
4. `./mk gate`, then `./mk ci` before calling the work done.

## Where things live

| Changing | Crate |
|---|---|
| Domain type, validation (`validate_criteria`) | `crates/lotus-model` |
| SPARQL text, query rewriters, CSV parsing, export rows | `crates/lotus-query` |
| Endpoint choice, retry, taxon resolution, HTTP client | `crates/lotus-search` |
| Curation rows, TSV input, QuickStatements | `crates/lotus-curation` |
| JSON-LD, CodeMeta, CITATION | `crates/lotus-jsonld` |
| CLI flags, output, exit codes | `crates/lotus-cli` |
| Framework-free state, URL codec, API DTOs, export pipelines | `crates/lotus-explore-core` |
| UI, API server (`src/server/`), browser glue | `apps/lotus-explore-rs` |
| Asset fetcher, healthcheck | `crates/lotus-web-assets` |
| Lint allows, `deny.toml` skips, ratchets, and why | `docs/DEBT.md` |

Pure crates (`lotus-model`, `lotus-query`, `lotus-curation`, `lotus-jsonld`)
have no HTTP, clock or runtime; `./mk gate` builds them for wasm32 alone.
`lotus-explore-core` must not depend on Dioxus (`./mk local:boundary`).

## Rules

- SPDX header on every `.rs` file (`./mk local:license-headers`).
- `SearchCriteria` has no `Default`: use `SearchCriteria::up_to_year(year)`.
- Name crates at call sites: `lotus_model::SearchCriteria`.
- `CriteriaQueryDto` owns both directions of the URL contract; a new parameter
  needs a round-trip test.
- No `unwrap`/`expect`/`panic`/indexing outside tests (denied). A needed
  suppression gets `reason = "..."` and a `docs/DEBT.md` row.
- Tests assert exact values (exact SPARQL, statements, bytes). Fix the code,
  not the expectation, unless the expectation encoded the bug; then say so in
  the commit.
- Comments say why. Docs claim only what a test or a run showed.
- Commits: Conventional Commits (`commits` gate), one logical change, each one
  green on its own. Human docs never link to agent files.
- `./mk echoes` fails on duplicated code; `./mk refquiem` fails when a doc or
  comment names a changed or removed item — update the mention.

## Done means

The change has a test that failed before it; `./mk ci` passes; docs that
describe the behaviour say what it now does; any new suppression is in
`docs/DEBT.md`.

## Also here

- `.github/ai/PITFALLS.md` — invariants and known gaps from the last audit.
- `.github/ai/AGENTS.md` — using the optional graphify knowledge graph.
