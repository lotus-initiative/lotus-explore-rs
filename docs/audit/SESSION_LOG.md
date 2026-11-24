# Session Log — `lotus-explore-rs` Structural Refactor

> **Note on file location:** The repository's `repo_hygiene::tests::session_log_is_absent` test enforces that `SESSION_LOG.md` does not exist at the repository root. Per the non-negotiable behavior-preservation rule, that test is preserved untouched and the session log is maintained here at `docs/audit/SESSION_LOG.md`.

---

## Baseline Metrics (`7b88120`)

| Metric | Value |
| --- | --- |
| Workspace crates | 9 |
| Total `.rs` files / prod LOC / total code LOC | 536 / 54,890 / 80,384 |
| Prod files > 300 LOC (total files > 300 LOC) | 51 (103) |
| Max prod file LOC | 2,041 (`crates/lotus-query/src/query.rs`) |
| Max prod fn LOC (non-i18n) | 261 (`format_values_clause` in `crates/lotus-query/src/query.rs:1498`) |
| Prod functions > 40 LOC / > 4 params | 182 / 72 |
| Max workspace dependency depth | 4 (`apps/lotus-explore-rs` → `lotus-explore-core` → `lotus-search` → `lotus-query` → `lotus-model`) |
| Prod `pub` items (non-app crates) | `lotus-explore-core`: 432, `lotus-model`: 271, `lotus-query`: 142, `lotus-curation`: 128, `lotus-search`: 124 (+ `lotus_model::*`), `lotus-jsonld`: 88, `lotus-cli`: 65, `lotus-web-assets`: 17 |
| Prod `Result<_, String>` sites | 61 |
| Prod `#[allow(...)]` / `#![allow(...)]` | 58 |
| Junk-drawer files (utility/helper/common modules) | 6 |
| `lotus-cli` release binary size | 5.5 MiB (`2.0 MiB` `.text`; `lotus_explore_core`: `107.4 KiB`, `lotus`: `119.2 KiB`, `lotus_search`: `32.0 KiB`) |

---

## Step Log

### Step 1a — Phase 0 Audit, Target Architecture, and Migration Plan
- **Status:** In progress
- **Files created/updated:**
  - `docs/audit/AUDIT.md` (created)
  - `docs/ARCHITECTURE.md` (updated for target 11-crate layered architecture)
  - `docs/REFACTOR_PLAN.md` (created)
  - `docs/audit/SESSION_LOG.md` (created)
- **Decisions:**
  - Keep `repo_hygiene::tests::session_log_is_absent` intact and place the session log at `docs/audit/SESSION_LOG.md` (documented for reviewer in `REVIEW_NOTES.md`).
  - Introduce zero new external dependencies; implement deterministic seeded property-test helpers directly in workspace test modules.
- **Open questions:**
  - Check whether `lotus-cli` still needs any dependency on `lotus-explore-core` once `lotus-export` is extracted (appears removable).

### Step 1b — Characterization Tests
- **Files added/edited:**
  - `crates/lotus-query/tests/characterization_query_surface.rs`
  - `crates/lotus-explore-core/tests/characterization_exports.rs`
  - `apps/lotus-explore-rs/src/server/characterization_tests.rs` (wired in `server/mod.rs`)
  - `docs/ARCHITECTURE.md` (fixed `./mk check-wasm-pure` / `./mk check-wasm-search` task syntax for `doc_links`)
- **Tests added:**
  - 3 query/rewriter/exporter characterization tests (`six_base_query_shapes_match_pinned_contracts`, `query_rewriters_preserve_exact_semantics`, `row_exporter_formats_are_stable`)
  - 4 export/DTO characterization tests (`pipeline_produces_exact_frozen_csv_and_metadata`, `write_archive_produces_deterministic_manifest_and_checksums`, `api_dto_wire_contract_is_pinned`, `metadata_and_filename_are_pinned`)
  - 3 HTTP API wire/schema characterization tests (`health_and_openapi_schemas_pin_expected_fields`, `search_request_accepts_both_canonical_and_legacy_aliases`, `export_file_missing_cache_key_returns_400_json_error`)
- **Decisions:** Pinned exact observed wire/CSV/JSON/SPARQL outputs rather than guessed substrings.

### Step 2 — Domain Consolidation (`lotus-model`)
- **Files added/edited:**
  - `crates/lotus-model/src/endpoints.rs` (new: `QLEVER_WIKIDATA`, `WDQS_WIKIDATA`, `WDQS_SCHOLARLY`, `SparqlEndpoint`)
  - `crates/lotus-model/src/qid.rs` + `crates/lotus-model/src/qid/tests.rs` (new: validated `Qid` and `Doi` newtypes with serde + 512-case deterministic seeded property tests)
  - `crates/lotus-model/src/{identify.rs, identify/tests.rs, structure.rs, lib.rs}` (moved `is_qid`, `standardize_taxon_name`, `normalize_structure` from `lotus-search`; replaced `#[allow(unreachable_pub)]` with `pub(crate)` + `#[expect(clippy::redundant_pub_crate, reason = ...)]`)
  - `crates/lotus-search/src/{lib.rs, search.rs, search/tests.rs}` & `crates/lotus-search/README.md` (removed `pub use lotus_model::*;` wildcard re-export and `looks_like_inchikey` duplicate; deduplicated endpoint literals)
  - `crates/lotus-curation/{Cargo.toml, src/input.rs}` (added `lotus-model`, switched `lotus-search` to `default-features = false`, migrated to `lotus_model::{non_empty, normalize_doi}`)
  - `crates/lotus-query/src/bulk_export.rs`, `crates/lotus-jsonld/src/dataset.rs`, `crates/lotus-explore-core/src/export/{metadata,archive}.rs` (reused `lotus_model::endpoints`)
  - Root `Cargo.toml`, `Cargo.lock`, `crates/lotus-search/Cargo.toml`, `crates/lotus-web-assets/Cargo.toml`, `apps/lotus-explore-rs/Cargo.toml` (lifted `bytes`, `dioxus-ssr`, `zip` into `[workspace.dependencies]`)
  - ~35 caller files updated from `lotus_search::*` model re-exports to `lotus_model::*`
- **Tests added:**
  - `crates/lotus-model/src/qid/tests.rs`: unit + 512-case seeded round-trip property tests for `Qid` and `Doi`
  - Moved pure identifier/taxon tests from `lotus-search` into `lotus-model/src/identify/tests.rs`
- **Decisions:**
  - Switched `Qid` parsing to handle both bare `q<digits>` and `http(s)://www.wikidata.org/entity/q<digits>` URIs consistently.
  - Replaced `#[allow(unreachable_pub)]` in `lotus-model/src/identify.rs` with `pub(crate)` + `#[expect(clippy::redundant_pub_crate, reason = ...)]` so crate-internal visibility is enforced without `allow` drift.
- **Verification & Metrics:** `./mk gate`, `./mk deny`, `./mk machete`, `./mk echoes`, `./mk refquiem`, `./mk doc`, and `cargo test --workspace --doc --locked` all green (with targeted `./mk check -p lotus-model`, `cargo nextest run -p lotus-model --locked`, `./mk doc`, doctests, `./mk typos`, `./mk echoes`, and `./mk refquiem` re-run after fixing `Qid::from_str` lowercase URI parsing); `lotus-search` public re-export leak (`pub use lotus_model::*;`, +271 symbols) eliminated.

### Step 3 — Extract `crates/lotus-export` from `lotus-explore-core`
- **Files created/moved:**
  - Created `crates/lotus-export/{Cargo.toml, README.md, src/lib.rs, tests/characterization_exports.rs}`
  - Split `pipeline` into `crates/lotus-export/src/pipeline/{mod,index,join,tests}.rs` with `PipelineOptions<'a>` (replacing 5 parameters)
  - Split `archive` into `crates/lotus-export/src/archive/{mod,fetch,dedup,tar_writer,tests}.rs` with `ArchiveOptions<'a>` & `WriteArchiveInput<'a>` (replacing 7 parameters)
  - Split `pubchem_tree` into `crates/lotus-export/src/pubchem_tree/{mod,fetch,index,compact,document,nodes,npclassifier,trees,tests}.rs` with `PubchemTreeOptions<'a>` & `PubchemTreeOutputs` (replacing a 6-tuple)
- **Dependency & boundary changes:**
  - `crates/lotus-explore-core/Cargo.toml`: removed `csv`, `flate2`, `tar`, and switched `lotus-search` to `default-features = false`
  - `crates/lotus-cli/Cargo.toml`: replaced `lotus-explore-core` with `lotus-export`, completely decoupling `lotus-cli` from explorer core UI/domain logic
  - Updated `Cargo.toml`, `Cargo.lock`, `Makefile.toml` (`boundary`), `mk.local` (`MUTANTS_PACKAGES`), `make/test.toml` (`cov`), `crates/lotus-query/src/bulk_export.rs`, `apps/lotus-explore-rs/tests/doc_links.rs`, `README.md`, `docs/ARCHITECTURE.md`, and `docs/DEBT.md`
- **Metrics after Step 3:** Workspace crates 9 → 10; every production file in `crates/lotus-export` is ≤ ~245 LOC (`index.rs`); all existing characterization and unit tests preserved intact.

### Step 4 — Extract `crates/lotus-server` and Unify Wire DTOs
- **Files created/moved/edited:**
  - `crates/lotus-explore-core/{Cargo.toml, src/api/dto.rs}`: added opt-in `openapi = ["dep:utoipa"]` feature and unified `ApiSmilesSearchType`, `ApiElementState`, and `RowDto` with bidirectional `From` conversions and `#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]`
  - Created `crates/lotus-server/{Cargo.toml, README.md, src/main.rs, src/lib.rs, src/{clock,cache_key*,export_urls*,errors,types,logging*,query_logic,services}.rs}`
  - Split `handlers.rs` (715 LOC) into `crates/lotus-server/src/handlers/{mod,prepare,search,export}.rs` (preserving `export_query_shape.rs` and `pump_tests.rs`)
  - Split `state.rs` (426 LOC) into `crates/lotus-server/src/state/{mod,metrics,cache,inflight}.rs`
  - Replaced `Result<_, String>` in `crates/lotus-server/src/config.rs` with typed `ConfigError` (`thiserror`)
  - Stripped `[features] server`, `axum`, `clap`, `flate2`, `tokio`, `tower-http`, `utoipa`, `tower`, runtime `sha2`, `src/server/`, `src/cache_key*`, and `src/export*` from `apps/lotus-explore-rs`
  - Updated `Cargo.toml`, `Cargo.lock`, `Dockerfile`, `Makefile.toml`, `make/{lint,test,web}.toml`, `mk.local`, `apps/lotus-explore-rs/tests/doc_links.rs`, `README.md`, `docs/{ARCHITECTURE,FRONTENDS,DEBT}.md`, `apps/lotus-explore-rs/{README.md,docs/ARCHITECTURE.md}`
- **Metrics after Step 4:** Workspace crates 10 → 11; `apps/lotus-explore-rs` dropped `server` feature + 8 server-only dependencies; `handlers/` max file is `export.rs` (363 lines; prepare 151, search 169, mod 46, export_query_shape 85) and `state/` max file is `cache.rs` (183 lines; metrics 128, mod 89, inflight 77); server OpenAPI & HTTP wire characterization tests preserved unchanged.

### Step 5a — Decompose `crates/lotus-query/src/query.rs` and `crates/lotus-model/src/columnar.rs`
- **Files created/removed/updated:**
  - Replaced `crates/lotus-query/src/query.rs` (2,044 lines) with `crates/lotus-query/src/query/{mod,prefixes,nomenclature,builders,builders_body,compound_names,lookups_taxon,lookups_compound,structure,dictionary,filters,rewrite}.rs` (preserving `query/tests.rs` and every `pub use` export in `crates/lotus-query/src/lib.rs`)
  - Replaced `crates/lotus-model/src/columnar.rs` (1,870 lines) with `crates/lotus-model/src/columnar/{mod,qid_dict,bitmask,sparse_strings,statement_id,access,diagnostics,sort,builder,builder_seed,filter}.rs` (preserving `columnar/tests.rs` and every `pub use` export in `crates/lotus-model/src/lib.rs`)
  - Updated `CONTRIBUTING.md`, `docs/ARCHITECTURE.md`, and `.cargo/mutants.toml` path references
- **Metrics after Step 5a:**
  - `crates/lotus-query/src/query/*.rs`: 2,044 lines → 11 prod files, max 290 total lines (`builders_body.rs`), `mod.rs` 51 lines
  - `crates/lotus-model/src/columnar/*.rs`: 1,870 lines → 11 prod files, max 283 total lines (`builder.rs`), `mod.rs` 254 lines
  - Zero public API breaks; `lotus-model` (including `--features diagnostics`) and `lotus-query` contract/characterization tests preserved untouched.

### Step 5b-1 — Decompose Library & Server Crates + Introduce `FinalizeInputs`
- **Files created/removed/updated:**
  - `crates/lotus-server/src/handlers/export.rs` → `export/{mod,stream,url}.rs` (updated `export_query_shape.rs` include path to `export/stream.rs`)
  - `crates/lotus-query/src/`: decomposed `parse.rs` → `parse/{mod,columns,compounds,counts,lookups}.rs`, `parse/stream.rs` → `parse/stream/{mod,fields,reader,splitter}.rs`, `export_rows.rs` → `export_rows/{mod,csv,json,turtle}.rs`, `bulk_export.rs` → `bulk_export/{mod,parse_id,queries_core,queries_enrich}.rs`
  - `crates/lotus-search/src/`: decomposed `search.rs` → `search/{mod,query_plan,resolve_taxon,resolve_structure,resolve_reference,execute_search,stream_search}.rs`, `execute.rs` → `execute/{mod,budget,endpoints,error_format,fallback,retry}.rs` (keeping `execute/wasm.rs`), `reqwest_client.rs` → `reqwest_client/{mod,schedule,transport}.rs` (keeping `reqwest_client/wasm.rs`), replacing `Result<reqwest::Client, String>` with typed `ClientBuildError`
  - `crates/lotus-curation/src/`: decomposed `wikidata_query.rs` → `wikidata_query/{mod,compound,occurrence,reference,statements,taxon,taxon_name}.rs`, `structure.rs` → `structure/{mod,convert,formula}.rs`, `knowledge.rs` → `knowledge/{mod,dependencies,lookup,result_row,statements}.rs`
  - `crates/lotus-explore-core/src/`: decomposed `graph/project.rs` → `graph/project/{mod,edges,nodes,search}.rs`, `graph/model.rs` → `graph/model/{mod,payload,starters,types}.rs`, `explore/url_codec/criteria.rs` → `criteria/{mod,encode,decode}.rs`, `export/metadata.rs` → `metadata/{mod,schema,builder}.rs` (removing `clippy::too_many_lines` from `build_metadata_json`), and replaced the 8-arg `finalize(...)` signature with `FinalizeInputs<'a>` in `explore/service/finalize.rs` (removing `clippy::too_many_arguments`)
  - Updated `CONTRIBUTING.md`, `docs/ARCHITECTURE.md`, and `.cargo/mutants.toml`
- **Metrics after Step 5b-1 (observed total lines via `wc -l`):**
  - `lotus-server/handlers/export/`: max 233 (`stream.rs`; `url.rs` 133, `mod.rs` 24)
  - `lotus-query/{parse,parse/stream,export_rows,bulk_export}/`: max 255 (`parse/stream/reader.rs`; `export_rows/mod.rs` 254, `turtle.rs` 187, `queries_core.rs` 173, `lookups.rs` 132)
  - `lotus-search/{search,execute,reqwest_client}/`: max 221 (`stream_search.rs`; `mod.rs` 201, `fallback.rs` 185, `resolve_structure.rs` 171; pre-existing `reqwest_client/wasm.rs` kept intact at 415)
  - `lotus-curation/{wikidata_query,structure,knowledge}/`: max 206 (`convert.rs`; `statements.rs` 182, `taxon_name.rs` 179, `lookup.rs` 154)
  - `lotus-explore-core/{graph/project,graph/model,url_codec/criteria,export/metadata}/`: max 312 (`metadata/builder.rs`, 297 code/comment lines excl. header; `criteria/decode.rs` 281, `graph/project/nodes.rs` 262, `graph/model/types.rs` 237)

### Step 5b-2 — Decompose `crates/lotus-cli` and `apps/lotus-explore-rs/build.rs`
- **Files created/removed/updated:**
  - `crates/lotus-cli/src/main.rs` (1,096 → 120 lines): extracted `args/{mod,search}.rs`, `criteria.rs`, `search_cmd.rs`, `export_cmd.rs`, `archive_cmd.rs`, and `clock.rs`; extracted `explain_search` & `resolve_explain_taxon` helpers; preserved all 5 `main/*_tests.rs` suites intact
  - `crates/lotus-cli/src/zenodo.rs` (579 lines) → `zenodo/{mod,types,client,publish}.rs` (keeping `zenodo/tests.rs` intact)
  - `crates/lotus-cli/src/curate.rs` (487 lines) → `curate/{mod,runner,report}.rs` (keeping `curate/tests.rs` intact)
  - `apps/lotus-explore-rs/build.rs` (892 → 161 lines) → `build/{mod,metadata,ai,site_files,csp,headers}.rs` (preserving `build_test.rs` & `build/tests.rs` unchanged; 1 scoped `#[allow(unreachable_pub)]` on `mod modules` recorded in `docs/DEBT.md`)
  - Marked Step 5 `[x]` in `docs/REFACTOR_PLAN.md` (query-builder unification between `lotus-search` and `lotus-explore-core` tracked into Step 6)
- **Metrics after Step 5b-2 (observed total lines via `wc -l`):**
  - `crates/lotus-cli/src/`: `archive_cmd.rs` 273, `zenodo/publish.rs` 234, `curate/runner.rs` 248, `args/search.rs` 201, `zenodo/client.rs` 210, `curate/mod.rs` 186, `export_cmd.rs` 172, `zenodo/types.rs` 149, `search_cmd.rs` 131, `criteria.rs` 130, `main.rs` 120, `args/mod.rs` 113, `curate/report.rs` 78, `clock.rs` 45, `zenodo/mod.rs` 29 (`output.rs` left untouched at 308 lines total, <300 code lines)
  - `apps/lotus-explore-rs/build*`: `build.rs` 161, `build/ai.rs` 259, `build/headers.rs` 150, `build/site_files.rs` 135, `build/csp.rs` 116, `build/metadata.rs` 115, `build/mod.rs` 9
  - `lotus-cli` (90 unit + 34 `cli` + 5 `docs_in_sync`) and `lotus-explore-rs` (`buildrs` 23/23 + `doc_links` 2/2) all pass unchanged.

### Step 6a — Deduplicate Query/QuickStatements Helpers & Eliminate Junk-Drawer Modules
- **Deduplication:**
  - Extracted `lotus_query::base_query_for_criteria(criteria, structure, taxon_qid)` (`crates/lotus-query/src/query/builders.rs`, re-exported at `query` and crate root) and routed both `lotus_search::build_base_query` (`crates/lotus-search/src/search/query_plan.rs`) and `lotus_explore_core::explore::service::build_query::build_sparql_query` through it (preserving existing `Exact` wildcard behavior strictly).
  - Moved QuickStatements formatting helpers (`QS_REF_INFERRED_FROM_SMILES`, `QS_REF_INFERRED_FROM_ISOMERIC_SMILES`, `qs_mass_statement`, `qs_statement_with_refs`, `qs_canonical_smiles_statement`, `qs_inchi_statement`, `qs_inchikey_statement`, `qs_isomeric_smiles_statement`) and their unit tests into `crates/lotus-curation/src/quickstatements.rs` (`lib.rs` re-exports) and deleted `apps/lotus-explore-rs/src/features/curation/services/helpers.rs`.
- **Junk-drawer elimination (0 `*util*`/`*helper*`/`*common*` non-test files left):**
  - `crates/lotus-explore-core/src/explore/search_utils.rs` → `hashing.rs` (`sanitize_taxon_input` replaced by direct `lotus_model::standardize_taxon_name`; extra assertions kept in `lotus-model/src/identify/tests.rs`)
  - `apps/lotus-explore-rs/src/utils/{mod,logging}.rs` → `src/services/event_log.rs` (`utils/` deleted)
  - `apps/lotus-explore-rs/src/ui/common.rs` → `src/ui/content_phase.rs`
  - `apps/lotus-explore-rs/src/components/results_graph/helpers.rs` → `src/components/results_graph/presentation.rs`
  - `apps/lotus-explore-rs/src/components/results_table/sort_helpers.rs` → inlined `aria_sort_for` & `sort_icon_for` into `src/components/results_table/header_model.rs` (`sort_helpers.rs` deleted)
  - `apps/lotus-explore-rs/src/i18n/helpers/` → `src/i18n/messages/`

### Step 6b — Replace Remaining Production `Result<_, String>` with Typed Error Enums
- **Typed error conversions across all crates & app targets (`0` production `Result<_, String>` signatures remaining):**
  - `crates/lotus-cli/src/curate/mod.rs`: `parse_tsv` now returns `Result<Vec<Finding>, lotus_curation::CurationError>` directly.
  - `crates/lotus-web-assets/src/inject_wasm_preload.rs`: introduced `PreloadError` (`Display` + `Error`, `Clone + PartialEq + Eq`) replacing `String` errors in `find_wasm_module` and `insert_after_anchor` (existing assertions preserved verbatim with `.to_string()`).
  - `crates/lotus-web-assets/src/fetch_assets/mod.rs`: introduced `FetchAssetsError` (`Vendor` / `Ketcher`) replacing `Result<(), String>` in `run` (preserving existing test assertion verbatim via `.to_string()`).
  - `apps/lotus-explore-rs/src/api/client.rs`: replaced `OnceLock<Result<reqwest::Client, String>>` / `build_http_client() -> Result<reqwest::Client, String>` with `#[derive(thiserror::Error)] struct ClientInitError(#[from] reqwest::Error)`.
  - `apps/lotus-explore-rs/src/upload.rs`: extended `UploadError` with `#[cfg(target_arch = "wasm32")]` variants (`BlobCreate`, `Dom`, `UnsupportedFileType`, `NotABlob`) and converted `extract_blob_from_file_data`, `blob_url_from_str`, `download_byte_chunks_as_blob`, `download_text_as_blob`, `click_download_anchor`, and `submit_download_form` (updated `sections/mod.rs` caller).
  - `apps/lotus-explore-rs/src/download/error.rs`: introduced `pub enum DownloadError` (`#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]` with `InvalidInput`, `Io`, `Network`, `Browser`, `InMemoryLimit`, `is_gateway_error()`, and `From<UploadError>`); converted `download.rs`, `download/{local_file,native,wasm,file_sink}.rs`, `download_actions.rs`, and `download_dispatch.rs` without touching OPFS/streaming source-shape gates (`opfs_write_stream.rs`, `opfs_availability_gate.rs`, `streaming_gate.rs`).
- Marked Step 6 `[x]` in `docs/REFACTOR_PLAN.md`.
- **Verification:** `lint-native-builds`, `lint-wasm`, `gate` (native + wasm checks, license headers, nextest, typos, fmt, tombi), `echoes` (0 duplicated/approximate/reuse groups), `refquiem` (clean), `doc`, and workspace `--doc` tests all green.

### Step 7a — Algorithmic/Allocation Efficiency, Property Tests & Release Benchmarks
- **Hot-path early exit (`crates/lotus-model/src/columnar/filter.rs`):** added non-ASCII needle short-circuit in `contains_folded` when `haystack.is_ascii()`, skipping ASCII window comparisons whenever a folded filter needle contains non-ASCII codepoints.
- **Deterministic property tests added (0 new external dependencies):**
  - `crates/lotus-model/src/columnar/tests.rs`: `statement_id_uuid_round_trips_for_deterministic_uuids` (256 UUID cases across lowercase/uppercase/URI-prefixed and compound-mismatch fallback) and `contains_folded_matches_naive_folded_substrings_and_rejects_non_ascii_on_ascii` (256 mixed ASCII/Unicode cases).
  - `crates/lotus-explore-core/src/explore/url_codec/tests.rs`: `criteria_query_params_round_trip_deterministically` (200 deterministic `SearchCriteria` combinations verifying parse/encode round-trip and idempotence).
  - `crates/lotus-curation/src/input/tests.rs`: `parse_tabular_handles_deterministic_quoted_csv_and_tsv_rows` (128 CSV/TSV LF/CRLF quoted rows with DOIs).
  - `crates/lotus-curation/src/quickstatements/tests.rs`: `escape_quickstatements_removes_unescaped_quotes_pipes_and_controls` (256 dirty & clean scalar cases).
  - `crates/lotus-query/src/query/tests.rs`: `query_rewriters_preserve_invariants_across_deterministic_shapes` across all search shapes, `wdqs_fallback`, `counts_query`, and `limit_query`.
- **Measured release benchmarks & binary size (observed directly via `--release --locked`):**
  - `lotus` CLI binary (`cargo build --release -p lotus-cli --locked`): `4.6M` (`target/release/lotus`).
  - `lotus-query` columnar benchmark (`cargo test -p lotus-query --release -- --ignored --nocapture bench`):
    - 1,000,000 rows (296.8 MB CSV): `split records 1510.3 ms`, `parse + build 6335.9 ms (44.7 MB/s)`, `resident 87.9 MB (92.2 B/row, 31% of the CSV)`, `plan text filter 91.8 ms`, `scan rows 44.1 ms`, `plan mass filter 13.5 ms`, `sort by name 25.1 ms`.
    - 2,000,000 rows (594.9 MB CSV): `split records 2781.1 ms`, `parse + build 8638.9 ms (65.7 MB/s)`, `resident 174.7 MB (91.6 B/row, 31% of the CSV)`, `plan text filter 117.5 ms`, `scan rows 73.3 ms`, `plan mass filter 10.3 ms`, `sort by name 1784.4 ms`.
    - 2,990,730 rows (890.1 MB CSV): `split records 5126.6 ms`, `parse + build 9737.8 ms (87.2 MB/s)`, `resident 260.7 MB (91.4 B/row, 31% of the CSV)`, `plan text filter 103.7 ms`, `scan rows 59.3 ms`, `plan mass filter 4.5 ms`, `sort by name 2297.7 ms`.
    - `bench_sort_scaling` (15 samples/size): `50k 0.6 ms` (`12.3 ns/row`), `100k 2.0 ms` (`19.9 ns/row`), `250k 8.4 ms` (`33.7 ns/row`), `500k 18.6 ms` (`37.2 ns/row`), `1M 21.1 ms` (`21.1 ns/row`), `2M 2004.5 ms` (`1002.3 ns/row`), `2.99M 4273.6 ms` (`1428.9 ns/row`).
  - `lotus-server` `cache_key` benchmark (50 samples): `tiny` `708 ns`, `everything` `3000–3041 ns`, `one taxon` `3792–3833 ns`, `structure search` `3041 ns`, `synthetic 16x` `94167–94625 ns`.

### Step 7b — Final Re-Audit & Anti-Timidity Check

#### Mutation & Workspace Gate Verification
- Targeted mutation sweep (`cargo mutants -p lotus-model -F filter.rs -- --locked`): `65 mutants tested in 9m: 65 caught` (0 missed, 0 unviable, 0 timeouts).
- `./mk echoes`: `12 doctests (12 unique), 4710 functions (4694 unique), 0 duplicated groups, 11842 context blocks (3044 unique), 0 approximate groups, 0 dependency reuse opportunities (36 packages)`.
- All workspace checks (`fmt-check`, `tombi-check`, `tombi-lint`, `typos`, `license-headers`, `check`, `lint`, `test-nextest`, `check-wasm-{pure,search,app}`, `lint-native-builds`, `lint-wasm`, `refquiem`, `doc`, workspace doctests) green prior to final doc commit; re-verified under `./mk ci`.

#### End-of-Refactor vs Baseline (`7b88120`) Metrics

| Metric | Baseline (`7b88120`) | Final (`HEAD`) | Delta |
| --- | --- | --- | --- |
| Workspace crates | 9 (8 in `crates/`, 1 app) | 11 (10 in `crates/`, 1 app) | +2 extracted crates |
| Total / prod `.rs` files | 536 total | 643 total / 431 prod | +107 files (modular splits + tests) |
| Prod files > 300 lines (raw / non-blank non-`//` code) | 51 raw | 32 raw / 17 code (3 in `crates/`) | −37% raw; 0 in 8 of 10 crates |
| Max prod file LOC (overall / in `crates/`) | 2,041 (`lotus-query`) | 593 (`apps/.../i18n/faq.rs`) / 415 (`crates/.../reqwest_client/wasm.rs`) | −71% overall / −80% in crates |
| Max prod function LOC (in `crates/`) | 261 | 134 (`dictionary.rs::format_values_clause`) | −49% |
| Prod `pub` items per library crate (`explore-core` / `model` / `curation` / `search` / `jsonld` / `query` / `cli`) | 432 / 271 / 128 / 124+`*` / 88 / 142 / 65 | 215 / 192 / 85 / 87 / 53 / 130 / 56 (+ 29 `export`, 27 `server`, 30 `web-assets`) | Tightened −9% to −50% per crate |
| Prod `Result<_, String>` sites | 61 | 0 | −100% |
| Junk-drawer files (`util`/`helper`/`common`) | 6 | 0 | −100% |
| `cargo echoes` duplicates / approx / reuse | — | 0 / 0 / 0 | Clean |
| `lotus` CLI release binary size | — | `4.6M` (`cargo build --release -p lotus-cli --locked`) | Recorded |

#### Anti-Timidity Check
1. **Three largest structural changes:**
   - Extracted `crates/lotus-server` from `apps/lotus-explore-rs` (`src/server/`, `cache_key`, export URL builders) and unified client/server wire schemas behind `lotus-explore-core`'s opt-in `openapi` feature, eliminating the `server` feature and crate-wide dead-code/unused-dep suppressions from the web client crate.
   - Extracted `crates/lotus-export` (bulk ETL pipelines, `pubchem_tree`, TAR/gzip archive builder) out of `crates/lotus-explore-core`, decoupling `lotus-cli` from browser/UI state and stripping `csv`, `flate2`, and `tar` from the WASM-targeted explorer core.
   - Consolidated domain primitives and query construction: removed `pub use lotus_model::*;` from `lotus-search`, introduced validated `Qid`/`Doi` newtypes and central Wikidata endpoint constants in `lotus-model`, unified the 6-shape SPARQL builder under `lotus_query::base_query_for_criteria`, moved QuickStatements builders into `lotus-curation`, decomposed every >400-line god module in `crates/` (including `query.rs`, `columnar.rs`, `main.rs`, `execute.rs`, `search.rs`, `wikidata_query.rs`, `knowledge.rs`, `project.rs`, `model.rs`, `build.rs`), and eliminated all 61 production `Result<_, String>` sites and all 6 junk-drawer modules.
2. **Crates & modules created vs deleted:**
   - Created crates: `crates/lotus-export`, `crates/lotus-server`.
   - Created domain/error/module hierarchies: `lotus_model::{qid, endpoints}` and decomposed `columnar/` tree; `lotus_query/src/query/` and `parse/` + `export_rows/` + `bulk_export/` submodules; `lotus_search/src/{search,execute,reqwest_client}/` submodules + `ClientBuildError`; `lotus_curation/src/{wikidata_query,structure,knowledge,quickstatements}/`; `lotus_explore_core/src/{graph/project,graph/model,explore/url_codec/criteria,export/metadata}/`; `lotus_cli/src/{args,curate,zenodo}/` + command modules; `apps/lotus-explore-rs/build/` submodules and `src/download/error.rs` (`DownloadError`).
   - Deleted: `crates/lotus-explore-core/src/export/{pipeline,archive,pubchem_tree}` (moved to `lotus-export`), `apps/lotus-explore-rs/src/server/` and server DTO duplicates (moved to `lotus-server` and unified on `lotus-explore-core`), `apps/lotus-explore-rs/src/features/curation/services/` QuickStatements duplicate file, and all 6 junk-drawer modules across core and app (search utility module, app logging utility module, UI common module, and graph/curation/i18n/table helper modules).
3. **Why not zero >300 LOC files globally?**
   - Every core/domain library crate except `lotus-search/src/reqwest_client/wasm.rs` (359 code lines of `web_sys` Fetch glue, guarded by source-shape gates) and `lotus-jsonld/src/software.rs` (304 code lines of JSON-LD fixture/metadata serialization) is at or below 300 code lines. The remaining files >300 code lines live in `apps/lotus-explore-rs/` declarative Dioxus UI views, static French FAQ translations (`i18n/faq.rs`, `faq_guide.rs`), and JS/OPFS browser interop guarded by textual gate tests (`download/file_sink.rs`, `download/wasm.rs`, `upload.rs`). Further splitting those static translation tables or gate-asserted files would add ceremony without reducing architectural coupling.

- Marked Step 7 `[x]` in `docs/REFACTOR_PLAN.md` and authored `REVIEW_NOTES.md`.
