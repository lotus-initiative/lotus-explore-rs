# AGENTS.md — lotus-explore-rs

Explorer and curation tool for the LOTUS compound–taxon–reference graph in
Wikidata, queried over SPARQL (QLever, falling back to WDQS). Rust workspace:
pure crates, a CLI, and one app crate (wasm web client and native API server).
Human docs: `README.md`, `CONTRIBUTING.md`, `docs/`.

Read `.github/ai/PITFALLS.md` before changing curation, query building, search
retries, the server, or exports. Each entry there was a shipped bug.

## Commands

- `./mk setup` — install pinned gate tools (including `dx`). `./mk setup-extra` adds optional audit/dev tools (`cargo-udeps`, `cargo-geiger`, `cargo-msrv`, `cargo-edit`, `cargo-bloat`).
- `./mk check [-p <crate>]` — fmt, tombi, typos, clippy (optionally scoped). Seconds when warm.
- `./mk check-changed` — inner loop scoped to affected packages + reverse-dependents.
- `./mk test-fast -E 'test(name)'` — one test. `./mk test-fast [-p <crate>]` — nextest (no doctests).
- `./mk test-changed` — nextest scoped to affected packages.
- `./mk test` — nextest over default and `server` builds, plus doctests.
- `./mk mutants-diff [--iterate]` — mutation testing on touched Rust files (`--iterate` retries remaining survivors).
- `./mk mutants-affected [base]` / `./mk mutants-plan [--mode diff|affected|full]` — downstream-affected mutation sweep / JSON shard plan.
- `./mk gate` — pre-push: everything above plus licence headers and wasm32 checks.
- `./mk ci-fast` / `./mk ci-core` — local pre-push gate / blocking PR gates excluding `mutants-diff`.
- `./mk ci` — full gate, what CI runs. `./mk --list` for every task.
- `./mk local:metadata` checks `CITATION.cff`/`codemeta.json`;
  `./mk metadata-write` regenerates them from `crates/lotus-jsonld`.
- `cargo run -p lotus-cli -- search --taxon Q158572 --explain` — print a query, no network.

Cold `./mk gate` is about 5 minutes. Cargo must be able to write `~/.cargo`.

## Verify cheaply, in this order

1. `./mk check -p <crate>` (or `cargo clippy -p <crate> --all-targets --features server` / `--target wasm32-unknown-unknown` when behind features/targets).
2. `./mk test-fast -p <crate> -E 'test(<name>)'` (or `./mk test-changed`).
3. `./mk mutants-diff --iterate` when touching code in `MUTANTS_PACKAGES`.
4. `./mk gate` (or `./mk ci-fast`), then `./mk ci` before calling the work done.

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
- `CriteriaQueryDto` owns both directions of the URL contract (including `reference` and `structure` without clearing explicit/empty taxon, and preserving Molfile leading blank header lines); a new parameter needs a round-trip test.
- No `unwrap`/`expect`/`panic`/indexing outside tests (denied). Every production
  `allow(...)` in `crates/` must carry `reason = "..."` (`gate_consistency`),
  and any workspace/app suppression gets `reason = "..."` plus a `docs/DEBT.md` row.
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
