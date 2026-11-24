# Architectural Audit — `lotus-explore-rs`

**Baseline commit:** `7b88120`  
**Scope:** Full workspace (`crates/*`, `apps/lotus-explore-rs`)  
**Method:** Static dependency graph inspection (`cargo metadata`, `cargo tree -d`), LOC & signature scan, `cargo machete`, `cargo deny check`, `cargo audit`, clippy cognitive complexity, and manual boundary inspection.

---

## Baseline Metrics Summary

| Metric | Baseline Value |
| --- | --- |
| Workspace crates | 9 (`apps/lotus-explore-rs`, 8 under `crates/`) |
| Rust source files | 536 (`80,384` code LOC total; `54,890` prod LOC) |
| Prod files > 300 LOC | 51 (`103` total files > 300 LOC) |
| Largest prod files | `crates/lotus-query/src/query.rs` (2,041), `crates/lotus-model/src/columnar.rs` (1,867), `crates/lotus-cli/src/main.rs` (1,053), `apps/lotus-explore-rs/build.rs` (889), `crates/lotus-search/src/execute.rs` (796), `apps/lotus-explore-rs/src/server/handlers.rs` (708) |
| Prod functions > 40 LOC | 182 (max 297 LOC in `i18n/fr.rs`, 261 LOC in `query.rs:1498`, 258 LOC in `results_graph/mod.rs:63`, 237 LOC in `export/metadata.rs:220`) |
| Prod functions > 4 params | 72 (max 18 in `GraphToolbar`, 10 in `run_pubchem_tree`, 8 in `run_lotus_exporter` and `finalize`) |
| Prod `Result<_, String>` sites | 61 |
| Prod `#[allow(...)]` / `#![allow(...)]` | 58 (`24` `#[expect(...)]`) |
| Junk-drawer files (`utils`/`helpers`/`common`) | 6 (`search_utils.rs`, `services/helpers.rs`, `results_graph/helpers.rs`, `results_table/sort_helpers.rs`, `ui/common.rs`, `utils/logging.rs`) |
| Prod `pub` items across crates | `lotus-explore-core` 432, `lotus-model` 271, `lotus-query` 142, `lotus-curation` 128, `lotus-search` 124 (+ wildcard `pub use lotus_model::*;`), `lotus-jsonld` 88, `lotus-cli` 65 |
| Supply chain / hygiene | `cargo machete`: clean; `cargo deny check`: pass (11 skipped duplicates in `deny.toml`); `cargo audit`: 0 advisories |

---

## Findings Ranked by Impact × Effort

### F1 (Critical, High Impact / Medium Effort): Web client and HTTP API server conflated in `apps/lotus-explore-rs`
- **Evidence:**
  - `apps/lotus-explore-rs/Cargo.toml:12-20,60-71`: `[features] server = ["dep:axum", "dep:clap", "dep:flate2", "dep:tokio", "dep:tower-http", "dep:utoipa"]`.
  - `apps/lotus-explore-rs/src/server/` (~2,200 prod LOC across 9 files + 1,435 test LOC) contains zero `dioxus` imports and only 4 thin links to `crate::` (`crate::clock::current_year` in `server/query_logic.rs:17,130,179`, `crate::cache_key` in `server/state.rs:4`, `crate::export::sanitize_download_filename` in `server/handlers.rs:544`, `crate::sparql` in `server/services.rs:8`).
  - `apps/lotus-explore-rs/src/main.rs:12-41`: forces 5 crate-wide `#![allow(...)]` (`unused_crate_dependencies`, `unreachable_pub`, `dead_code` under `cfg`, `empty_enums`, `missing_const_for_fn`) to silence cross-program dead-code and feature-gated dev-dependency (`tower`) false positives.
  - `apps/lotus-explore-rs/src/cache_key.rs:23-89`: browser target does not call server cache keys, requiring `#[cfg(any(not(target_arch = "wasm32"), test))]` gymnastics on every item.
- **Architectural defect:** Two independent delivery binaries (Dioxus WASM SPA vs native Axum OpenAPI service) live in one crate, inflating server compile scope with 29,185 prod LOC of UI and polluting the web crate's lints and manifest.

### F2 (Critical, High Impact / Medium Effort): CLI-only bulk ETL pipelines embedded inside `crates/lotus-explore-core`
- **Evidence:**
  - `crates/lotus-explore-core/Cargo.toml:12-39`: pulls `csv`, `flate2`, `tar`, and default-featured `lotus-search` (`reqwest`) solely for `src/export/pipeline.rs` (551 prod LOC), `src/export/archive.rs` (532 prod LOC), and `src/export/pubchem_tree/` (~1,200 prod LOC).
  - `crates/lotus-explore-core/src/lib.rs:5-15`: requires `#![cfg_attr(target_arch = "wasm32", allow(clippy::future_not_send, reason = "..."))]` because of async bulk-export HTTP futures, even though the interactive explorer core (`explore/`, `filters.rs`, `sort.rs`, `table_budget.rs`, `graph/`, `api/`) is synchronous.
  - `crates/lotus-cli/Cargo.toml:20`: CLI depends on the entire web explorer core just to invoke `export::{pipeline, pubchem_tree, archive}` (`crates/lotus-cli/src/main.rs:30,605,746`).
- **Architectural defect:** Interactive UI domain (`ExploreState`, URL codec, virtual table filtering/sorting, graph projection) is coupled to heavy batch ETL pipelines, dragging `tar`/`flate2`/`csv` into the web application core and coupling the CLI binary to UI state reducers.

### F3 (Critical, High Impact / Low-Medium Effort): Wildcard re-export, misplaced pure domain functions, and duplicated endpoint constants
- **Evidence:**
  - `crates/lotus-search/src/lib.rs:62`: `pub use lotus_model::*;` leaks the entire model crate surface through the HTTP/search layer, hiding actual dependency edges.
  - `crates/lotus-search/src/search.rs:65-75,279-309`: pure string normalization/classification functions (`normalize_structure`, `is_qid`, `standardize_taxon_name`) live inside the async HTTP orchestration crate instead of `lotus-model`.
  - `crates/lotus-curation/Cargo.toml:14` and `crates/lotus-curation/src/input.rs:19-20`: `lotus-curation` depends on `lotus-search` with default features (`reqwest` enabled!) despite its description claiming "No clock, no HTTP client", and imports `non_empty` / `normalize_doi` via `lotus_search` instead of `lotus_model`.
  - Endpoint URLs (`https://qlever.dev/api/wikidata`, `https://query-next.wikidata.org/sparql`) are duplicated across `crates/lotus-search/src/lib.rs:65-68`, `crates/lotus-query/src/bulk_export.rs:18`, `crates/lotus-explore-core/src/export/metadata.rs:17-19`, `crates/lotus-explore-core/src/export/archive.rs:135`, and `crates/lotus-jsonld/src/dataset.rs:164`.
  - `crates/lotus-model/src/identify.rs:27,110`: `pub fn strip_qid_digits` and `pub fn find_ascii_ci` carry `#[allow(unreachable_pub)]` instead of `pub(crate) fn`.
  - `crates/lotus-search/Cargo.toml:25` (`bytes`), `crates/lotus-web-assets/Cargo.toml:38` (`zip`), and `apps/lotus-explore-rs/Cargo.toml:45` (`dioxus-ssr`) bypass `[workspace.dependencies]`.

### F4 (High, High Impact / Medium-High Effort): Duplicated domain and service logic between app and library crates
- **Evidence:**
  - **Wire DTOs duplicated:** `apps/lotus-explore-rs/src/server/types.rs:30-210` duplicates `ApiSmilesSearchType`, `ApiElementState`, `SearchRequest`, `SearchStats`, `RowDto`, `SearchResponse`, and `ExportUrlResponse` already defined in `crates/lotus-explore-core/src/api/dto.rs:13-294`.
  - **Curation logic & fallback duplicated in app:**
    - `apps/lotus-explore-rs/src/features/curation/services/mod.rs:72-110` re-implements QLever→WDQS fallback (`execute_sparql_with_wdqs_fallback`) checking only `status: 502` and `Network` instead of reusing `lotus_search::execute_with_fallback`.
    - `apps/lotus-explore-rs/src/features/curation/services/helpers.rs:18-88` re-implements QuickStatements builders (`qs_mass_statement`, `qs_canonical_smiles_statement`, etc.), while `crates/lotus-curation/src/knowledge.rs:205-272` holds the shared builders.
    - `apps/lotus-explore-rs/src/features/curation/services/enrichment.rs` (566 prod LOC) and `wikidata.rs` (365 prod LOC) duplicate batch lookup/reconciliation and pass 7 arguments including `Arc<dyn CurationKnowledgeRepository>` + `Arc<Mutex<OccurrenceAskCache>>` (`enrichment.rs:48-56`).
  - **Explore query dispatch duplicated:** `crates/lotus-explore-core/src/explore/service/build_query.rs:58-105` duplicates the 6-shape dispatch of `crates/lotus-search/src/search.rs:151-189`.
  - **Taxon QID check duplicated:** `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/mod.rs:45-58` re-implements `is_qid` with `#[allow(clippy::indexing_slicing)]`.

### F5 (High, Medium-High Impact / Medium-High Effort): God files and high-complexity functions
- **Evidence (top offenders > 300 prod LOC):**
  - `crates/lotus-query/src/query.rs` (2,041 prod LOC): mixes nomenclature path generation, 6 base query builders, lookup queries, IDSM clauses, 4 string rewriters, dictionary chunking, filter injection, and escaping. `format_values_clause` is 261 LOC.
  - `crates/lotus-model/src/columnar.rs` (1,867 prod LOC): mixes `QidDictionary`, `Bitmask`, `SparseStrings`, `StatementId` UUID codec, `ColumnarResultSet`, `ColumnarBuilder`, text folding, and `FilterPlan`.
  - `crates/lotus-cli/src/main.rs` (1,053 prod LOC): mixes CLI `clap` definitions, `search`, `export`, `export_pubchem_tree`, `archive`, Zenodo orchestration, and clock/range helpers.
  - `apps/lotus-explore-rs/build.rs` (889 prod LOC): mixes `site-metadata.json` schema, XML/JSON/TXT generators, inline-script SHA-256 + custom base64 encoder, CSP headers, and dx cleanup.
  - `crates/lotus-search/src/execute.rs` (796 prod LOC) and `search.rs` (641 prod LOC).
  - `apps/lotus-explore-rs/src/server/handlers.rs` (708 prod LOC) and `state.rs` (426 prod LOC).
  - `crates/lotus-explore-core/src/graph/project.rs` (655 prod LOC) and `model.rs` (530 prod LOC).
  - `crates/lotus-curation/src/wikidata_query.rs` (631 prod LOC) and `knowledge.rs` (574 prod LOC).
  - `crates/lotus-query/src/parse/stream.rs` (604 prod LOC) and `export_rows.rs` (518 prod LOC).

### F6 (Medium-High, Medium Impact / Low-Medium Effort): Stringly-typed errors, primitive obsession, parameter bloat, and junk-drawer modules
- **Evidence:**
  - **61 `Result<_, String>` sites in prod code:** `crates/lotus-search/src/reqwest_client.rs:41,362`, `crates/lotus-cli/src/curate.rs:94`, `crates/lotus-web-assets/src/inject_wasm_preload.rs:70,121`, `apps/lotus-explore-rs/src/server/config.rs:92-178`, `apps/lotus-explore-rs/src/download/{mod.rs, native.rs, file_sink.rs}`, `apps/lotus-explore-rs/src/upload.rs`.
  - **Primitive obsession:** QIDs and DOIs passed as raw `&str` / `String` across boundaries instead of validated `Qid` / `Doi` newtypes (requiring repeated ad-hoc `sanitize_qid` / `normalize_qid` / `strip_doi_prefix` calls).
  - **Parameter bloat (72 functions > 4 params):** `run_pubchem_tree` (10 params), `run_lotus_exporter` (8), `finalize` (8, carrying `#[allow(clippy::too_many_arguments)]`), `write_archive` (7), `curate_single_row` (7), `GraphToolbar` (18), `GraphCanvasStage` (13).
  - **Junk-drawer modules:** `crates/lotus-explore-core/src/explore/search_utils.rs`, `apps/lotus-explore-rs/src/utils/{mod.rs, logging.rs}`, `apps/lotus-explore-rs/src/ui/common.rs`, `apps/lotus-explore-rs/src/features/curation/services/helpers.rs`, `apps/lotus-explore-rs/src/components/results_graph/helpers.rs`, `apps/lotus-explore-rs/src/components/results_table/sort_helpers.rs`.
  - **Missing `thiserror` / `anyhow` discipline:** `lotus-web-assets` uses `Box<dyn Error>` + `String` errors; `server/config.rs` uses `String` errors.

### F7 (Medium, Medium Impact / Medium Effort): Weak property-based and benchmark coverage on hot/parser paths
- **Evidence:**
  - Existing benchmarks cover only `cache_key/bench.rs` and `lotus-query/examples/measure_export.rs`; no micro-benchmarks for `ColumnarResultSet` filter/sort or `CsvColumnarReader` streaming fold.
  - Parsers (`StatementId` UUID codec, `Qid`/`Doi` parsing, `CriteriaQueryDto` URL codec, `parse_tabular` TSV reader, `with_filters`/`counts_query`/`occurrences_query` rewriters) rely solely on example-based tests without randomized property tests.
