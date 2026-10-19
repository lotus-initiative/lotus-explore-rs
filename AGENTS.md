# AGENTS.md — lotus-explore-rs

A linked open data explorer for the [LOTUS](https://doi.org/10.7554/eLife.70780)
compound-taxon-reference knowledge graph from Wikidata, queried over SPARQL.
The search, query-building and curation logic lives in the `lotus-*` crates,
which the web app, the `lotus` CLI and the tests all share.

## Quick commands

- `./mk setup` — install all tools at pinned versions (run once)
- `./mk gate`  — fast inner loop: fmt, tombi, typos, licence-headers, check,
  clippy, nextest, wasm type-checks (seconds, warms `target/`)
- `./mk ci`    — full gate: everything CI runs, in order, no network access
- `./mk --list-all-steps` — the authoritative task list
- `./mk test`  — nextest suite + doctests (`cargo test --doc`)
- `./mk ci-fast` — the gate minus supply-chain checks
- `./mk cli`   — run the lotus CLI (pass args after `--`)

## Where things live

| You are changing | It belongs in |
|---|---|
| A domain type, filter rule, validation | `crates/lotus-model/` |
| A SPARQL string, CSV column, export format | `crates/lotus-query/` |
| Endpoint selection, retry, taxon resolution | `crates/lotus-search/` |
| A curation row, TSV reader, QuickStatements | `crates/lotus-curation/` |
| JSON-LD, CodeMeta, CITATION | `crates/lotus-jsonld/` |
| A CLI flag, output format, exit code | `crates/lotus-cli/` |
| A rendered row, the clock, cache keys | `apps/lotus-explore-rs/` |

## Architecture

8 crates in a workspace: `lotus-model`, `lotus-query`, `lotus-search`,
`lotus-jsonld`, `lotus-curation`, `lotus-web-assets`, `lotus-cli` (libraries),
and `apps/lotus-explore-rs` (the app: wasm32 web client, optional native server,
optional desktop window). Dependency arrows point inward — the four pure crates
(`lotus-model`, `lotus-query`, `lotus-curation`, `lotus-jsonld`) have no HTTP,
no clock, no runtime, verified by CI building them for `wasm32` in isolation.

See `docs/ARCHITECTURE.md` for the full write-up. The application-side docs live
in `apps/lotus-explore-rs/docs/`.

## Conventions (not inferable from code)

- **SPDX headers:** every `.rs` file starts with exactly:
  ```
  // SPDX-License-Identifier: AGPL-3.0-only
  // SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
  ```
  Enforced by `./mk license-headers`.
- **`SearchCriteria` has no `Default`:** "no filters" is a value _plus a year_.
  Use `SearchCriteria::up_to_year(year)`. The year is an argument because the pure
  crates cannot read a clock.
- **Name crates at call sites:** `lotus_model::SearchCriteria`, not
  `crate::models::SearchCriteria`. There are no `models.rs` or `queries.rs`
  re-exports.
- **`CriteriaQueryDto` owns both halves** of the URL parameter contract (building
  and parsing). Adding a parameter means adding it there, with a round-trip test.
- **Panics and `expect` are for tests and unreachable states only** — never for
  user input, endpoint responses, or file contents. The workspace lints enforce
  `panic = "deny"`, `expect_used = "deny"`, `unwrap_used = "deny"`.
- **Comments explain *why*, not *what*.** If a line needs a comment saying what
  it does, rename the line instead.
- **Tests assert exact values** — exact SPARQL, exact statements, exact bytes —
  not just that something runs.
- **Doctests matter:** each library crate's `README.md` is its crate documentation
  via `#![doc = include_str!("../README.md")]`. `./mk test` runs nextest and then
  `cargo test --workspace --doc` because nextest does not run doctests.

## Verification

- `./mk gate` — the fast inner loop before you push
- `./mk ci`   — the full gate (what CI runs)
- Pre-push hooks (`prek.toml`) run the same tasks before a push. The alignment of
  CI, hooks, and makefiles is enforced by `apps/lotus-explore-rs/tests/gate_consistency.rs`
  (20 tests, all PASS).
- The `ci` task, `prek.toml`, the `ci.yml` workflow, and the tombi version in
  all three are checked for consistency by that test file.

## Knowledge graph (graphify)

If `graphify` (v0.9.90+) is on `PATH`, use it for codebase questions before
grep:

- `graphify query "how does taxon resolution work"` — returns a scoped subgraph
- `graphify path "lotus-search" "lotus-query"` — relationships between modules
- `graphify explain "SearchCriteria"` — focused concept explanation
- `graphify update .` — refresh after code changes (AST-only, no API cost)

The graph output at `graphify-out/` is gitignored and local-only. See
`.github/ai/AGENTS.md` for the full guide. When graphify is absent, use `grep`
and the compiler instead — do not install it.

## Tooling

- **Rust:** 1.99.0, pinned in `rust-toolchain.toml`. Components: clippy,
  rustfmt, llvm-tools-preview. Target: wasm32-unknown-unknown.
- **Task runner:** cargo-make 0.37.24 via the `./mk` script
  (`cargo make --no-workspace`, the `--no-workspace` flag is the point).
- **Tests:** cargo-nextest 0.9.146 (parallel, per-test-process), plus
  `cargo test --doc` for doctests.
- **TOML:** tombi (version read from `prek.toml`)
- **Mutation testing:** cargo-mutants (run `./mk mutants` on library crates only)
- **All tools:** installed at pinned versions by `./mk setup`
