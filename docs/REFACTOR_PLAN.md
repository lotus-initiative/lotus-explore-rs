# Refactor Plan — `lotus-explore-rs`

Ordered migration plan implementing [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) against the findings in [`docs/audit/AUDIT.md`](audit/AUDIT.md). Each step lands as one green conventional commit passing all repository gates before advancing.

---

## Status Tracker

| Step | Scope | Key Findings | Status | Verification | Rollback Point |
| --- | --- | --- | --- | --- | --- |
| **1a** | Phase 0 audit, target architecture, plan, session log | Baseline | `[x]` | `./mk test-fast -E 'test(repo_hygiene)' && ./mk typos && ./mk refquiem` | `7b88120` |
| **1b** | Characterization / golden tests across query, export, server API | Rule 1 | `[x]` | `./mk gate` | Commit 1 |
| **2** | Consolidate `lotus-model` (`Qid`/`Doi`, endpoints, normalizers), remove `lotus_model::*` wildcard, `lotus-curation` `default-features = false` | F3, F6 | `[x]` | `./mk gate && ./mk deny && ./mk machete && ./mk echoes && ./mk refquiem && ./mk doc` | Commit 2 |
| **3** | Extract `crates/lotus-export` (bulk ETL) from `lotus-explore-core`; decouple `lotus-cli` | F2, F5, F6 | `[x]` | `./mk gate && ./mk deny && ./mk machete && ./mk echoes && ./mk refquiem && ./mk doc && ./mk local:boundary` | Commit 3 |
| **4** | Unify API DTOs; extract `crates/lotus-server` from `apps/lotus-explore-rs` | F1, F4, F5, F6 | `[x]` | `./mk gate && ./mk local:lint-native-builds && ./mk local:lint-wasm && ./mk test && ./mk deny && ./mk machete && ./mk echoes && ./mk refquiem && ./mk doc` | Commit 4 |
| **5** | Split god modules (`lotus-query`, `lotus-model`, `lotus-search`, `lotus-curation`, `lotus-cli`, `lotus-explore-core`, `build.rs`) & replace parameter lists with option structs | F5, F6 | `[x]` | `./mk gate && ./mk doc && ./mk echoes && ./mk refquiem` | Commit 5 |
| **6** | Deduplicate app curation & explore services against shared crates; eliminate junk drawers (utility/helper/common modules) and `Result<_, String>` | F4, F6 | `[x]` | `./mk gate && ./mk local:lint-native-builds && ./mk local:lint-wasm && ./mk echoes && ./mk refquiem` | Commit 6 |
| **7** | Efficiency (benchmarks + algorithmic/alloc wins), deterministic property tests, full re-audit (`./mk ci` + `./mk mutants-diff`), `REVIEW_NOTES.md` | F7, Exit Criteria | `[ ]` | `./mk ci` | Commit 6/7 |

---

## Step Details

1. **Step 1a — Audit & Target Architecture:** Establish baseline binary bloat, LOC, complexity, and dependency-graph metrics; document findings in `docs/audit/AUDIT.md`, target 11-crate architecture in `docs/ARCHITECTURE.md`, migration sequence here, and session tracking in `docs/audit/SESSION_LOG.md`.
2. **Step 1b — Characterization Tests:** Pin exact SPARQL output across all 6 query shapes + 4 rewriters + `RowExporter`, bulk export pipelines + metadata JSON, and server HTTP routes (`/health`, `/openapi.json`, `/v1/search`, `/v1/export-url`, `/v1/export-file`) before moving code.
3. **Step 2 — Domain Consolidation (`lotus-model`):** Add `endpoints.rs` and validated `Qid`/`Doi` newtypes; move pure string normalizers from `lotus-search` into `lotus-model`; delete `pub use lotus_model::*;` in `lotus-search`; drop `#[allow(unreachable_pub)]` in `lotus-model/src/identify.rs`; switch `lotus-curation` to `lotus-search` with `default-features = false`; lift `bytes`/`zip`/`dioxus-ssr` into `[workspace.dependencies]`.
4. **Step 3 — Extract `crates/lotus-export`:** Move `pipeline`, `archive`, and `pubchem_tree` out of `lotus-explore-core` into `crates/lotus-export` with option structs replacing 6–10 parameter functions; remove `csv`/`flate2`/`tar` from `lotus-explore-core`; decouple `lotus-cli` from `lotus-explore-core`.
5. **Step 4 — Extract `crates/lotus-server`:** Add opt-in `openapi` feature to `lotus-explore-core::api::dto` so server and client share one wire schema; move `src/server/`, `cache_key.rs`, and `export/urls.rs` into `crates/lotus-server`; split `handlers.rs` and `state.rs`; replace `String` errors in `config.rs` with `ConfigError`; strip `server` feature and crate-wide dead-code/unused-dep allows from `apps/lotus-explore-rs`.
6. **Step 5 — God-File & Parameter Refactor:** Decompose `lotus-query/src/query.rs` (2,041 LOC), `lotus-model/src/columnar.rs` (1,867 LOC), `lotus-cli/src/main.rs` (1,053 LOC), `lotus-search/src/{execute,search}.rs`, `lotus-curation/src/{wikidata_query,knowledge}.rs`, `lotus-explore-core/src/graph/`, and `apps/lotus-explore-rs/build.rs` into focused ≤300 LOC modules and ≤40 LOC functions; introduce `FinalizeInputs` and unify the 6-shape query builder between `lotus-search` and `lotus-explore-core`.
7. **Step 6 — App Deduplication, Error Typing & Junk-Drawer Removal:** Route app curation SPARQL fallback and QuickStatements formatting through `lotus-search` / `lotus-curation`, eliminate utility/helper/common modules, convert `Result<_, String>` in `reqwest_client.rs`, `curate.rs`, `download/`, `upload.rs`, and `lotus-web-assets` into `thiserror` enums, and tighten `pub` visibility.
8. **Step 7 — Performance, Property Tests & Final Verification:** Profile and measure hot paths (`ColumnarResultSet` filter/sort, `CsvColumnarReader` fold, `cache_key` hex encoding), add deterministic property tests for `Qid`/`Doi`, `StatementId`, `CriteriaQueryDto`, `parse_tabular`, and SPARQL rewriters, run `./mk ci` and mutation testing, and author `REVIEW_NOTES.md`.
