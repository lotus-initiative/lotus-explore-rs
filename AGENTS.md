# AGENTS.md — lotus-explore-rs

A linked open data explorer for the [LOTUS](https://doi.org/10.7554/eLife.70780)
compound-taxon-reference knowledge graph from Wikidata, queried over SPARQL.
The search, query-building and curation logic lives in the `lotus-*` crates,
which the web app, the `lotus` CLI and the tests all share.

## Quick commands

- `./mk setup` — install the gate tools at pinned versions (run once; `./mk setup-extra` adds dx and other dev tools)
- `./mk check` — fast inner check (`fmt`, `tombi`, `typos`, `lint`)
- `./mk fix`   — auto-fix formatting, TOML, typos, and clippy
- `./mk test-fast` — unit + integration tests via nextest (`./mk test-fast -E 'test(name)'` for one test)
- `./mk test`  — nextest suite over three builds (default, server, desktop) + doctests (`cargo test --doc`)
- `./mk watch` — continuous feedback via bacon / cargo-watch
- `./mk gate`  — fast pre-push loop: fmt, tombi, typos, licence-headers, check, clippy, nextest, wasm type-checks
- `./mk ci`    — full gate: everything CI runs, in order (the rusti-graben standard gates plus `mk.local`)
- `./mk ci-nightly` — mutation testing, coverage floor, feature powerset, miri, outdated
- `./mk --list` — the authoritative task list (any other name runs that cargo-make task)
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
| Framework-free state reducers, URL/filters/sort/export/API DTOs | `crates/lotus-explore-core/` |
| A rendered row, the clock, cache keys | `apps/lotus-explore-rs/` |
| Technical debt, lint allows, `deny.toml` skips, and ratchets | `docs/DEBT.md` |

## Architecture

9 crates in a workspace: `lotus-model`, `lotus-query`, `lotus-search`,
`lotus-jsonld`, `lotus-curation`, `lotus-explore-core`, `lotus-web-assets`, `lotus-cli` (libraries/CLI),
and `apps/lotus-explore-rs` (the app: wasm32 web client, optional native server,
optional desktop window). Dependency arrows point inward — the pure crates
(`lotus-model`, `lotus-query`, `lotus-curation`, `lotus-jsonld`) have no HTTP,
no clock, no runtime, verified by CI building them for `wasm32` in isolation.

See `docs/ARCHITECTURE.md` for the full write-up and `docs/DEBT.md` for active exceptions and ratchets.

## Conventions (not inferable from code)

- **SPDX headers:** every `.rs` file starts with `// SPDX-License-Identifier: AGPL-3.0-only` and `// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project` (enforced by `./mk license-headers`).
- **`SearchCriteria` has no `Default`:** "no filters" is a value _plus a year_. Use `SearchCriteria::up_to_year(year)` because pure crates cannot read a clock.
- **Name crates at call sites:** `lotus_model::SearchCriteria`, not `crate::models::SearchCriteria`.
- **`CriteriaQueryDto` owns both halves** of the URL parameter contract (building and parsing). Adding a parameter means adding it there, with a round-trip test.
- **Panics and `expect` are for tests and unreachable states only** — never for user input, endpoint responses, or file contents (`panic = "deny"`, `expect_used = "deny"`, `unwrap_used = "deny"`, `unsafe_code = "forbid"`). Record any unavoidable exception in `docs/DEBT.md`.
- **Comments explain *why*, not *what*.** If a line needs a comment saying what it does, rename the line instead.
- **Tests assert exact values** — exact SPARQL, exact statements, exact bytes.
- **Doctests matter:** each library crate's `README.md` is its crate documentation via `#![doc = include_str!("../README.md")]`.
- **No duplicates (`./mk echoes`):** runs `cargo echoes --all-targets --config .echoes.toml` and fails on exact duplicate doctests, functions, or context blocks, or if approximate/reuse counts exceed `mk.local`. Deliberate copies need `// echoes: allow` or ` ```rust,echoes `.
- **Stale references (`./mk refquiem`):** runs `cargo refquiem --config .refquiem.toml` and fails on any mention of a Rust item, dependency, feature, binary, or example that a commit removed.

## Verification & Knowledge graph (graphify)

- Run `./mk gate` for fast feedback and `./mk ci` before finishing. Alignment of CI, hooks (`prek.toml`), and makefiles is enforced by `apps/lotus-explore-rs/tests/gate_consistency.rs`.
- If `graphify` is on `PATH`, use `graphify query "<question>"`, `graphify path`, `graphify explain`, and `graphify update .` (output in `graphify-out/` is gitignored).

## Tooling

All tool versions are pinned in `mk.versions` and `mk.local` and installed by `./mk setup`:
Rust toolchain (`rust-toolchain.toml`, target `wasm32-unknown-unknown`), `cargo-make`, `cargo-nextest`, `dioxus-cli` (`dx`), `tombi`, `shellcheck`, `typos-cli`, `cargo-deny`, `cargo-audit`, `cargo-machete`, `cargo-semver-checks`, `echoes`, `refquiem`, `zizmor`, `cargo-mutants`, `cargo-llvm-cov`, `cargo-hack`, and `cargo-outdated`.
