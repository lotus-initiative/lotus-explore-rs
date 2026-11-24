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
| Junk-drawer files (`utils`/`helpers`/`common`) | 6 |
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
