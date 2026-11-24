# Review Notes — `lotus-explore-rs` Structural Refactor

These notes guide human review of the refactor from baseline commit `7b88120` to `HEAD`.

> **Location note on session log:** Repository hygiene tests (`apps/lotus-explore-rs/tests/gate/repo_hygiene/tests.rs::session_log_is_absent`) explicitly forbid a root `SESSION_LOG.md` file. The chronological session log, per-step metrics, and Anti-Timidity Check are maintained in [`docs/audit/SESSION_LOG.md`](docs/audit/SESSION_LOG.md).

## Deliverables

- Architecture overview and dependency layering: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- Baseline audit (phase 0 findings & metrics): [`docs/audit/AUDIT.md`](docs/audit/AUDIT.md)
- Migration plan & step status tracker: [`docs/REFACTOR_PLAN.md`](docs/REFACTOR_PLAN.md)
- Chronological log, benchmarks, mutation results, and Anti-Timidity Check: [`docs/audit/SESSION_LOG.md`](docs/audit/SESSION_LOG.md)
- Active lint, duplicate-crate, and mutation exceptions: [`docs/DEBT.md`](docs/DEBT.md)

## Key Decisions & Rejected Alternatives

1. **Extract `crates/lotus-export` and `crates/lotus-server` (zero new external dependencies)**
   - **Decision:** Split batch ETL (`lotus-export`) out of `crates/lotus-explore-core` and the native Axum OpenAPI server (`lotus-server`) out of `apps/lotus-explore-rs`.
   - **Rejected alternative:** Leaving feature flags (`server`, ETL modules) inside `apps/lotus-explore-rs` and `lotus-explore-core`. Feature-gating kept `tar`/`flate2`/`csv`/`utoipa`/`axum` tangled with the WASM UI crate, forced crate-wide `dead_code`/`unused_crate_dependencies` suppressions, and coupled `lotus-cli` to interactive UI state reducers.
2. **Single wire schema via opt-in `openapi` feature on `crates/lotus-explore-core`**
   - **Decision:** Keep DTO definitions in `lotus-explore-core/src/api/dto.rs`, gating `utoipa::ToSchema` derives under an opt-in `openapi` feature enabled only by `lotus-server`.
   - **Rejected alternative:** Introducing a separate micro-crate solely for DTO structs (adds workspace manifest boilerplate with no dependency cycle benefit).
3. **Conservative preservation in query unification (`crates/lotus-query`)**
   - **Decision:** Extracted `lotus_query::base_query_for_criteria(criteria, structure, taxon_qid)` in `crates/lotus-query/src/query/builders.rs` for both `lotus-search` and `lotus-explore-core`. Preserved the pre-existing `SmilesSearchType::Exact` wildcard branch verbatim (`None` resolved taxon passes `None` rather than forwarding `*`, avoiding a silent SPARQL change).
4. **Target-clean typed errors (`DownloadError`, `UploadError`, `ClientInitError`, `PreloadError`, `FetchAssetsError`)**
   - **Decision:** Eliminated all 61 production `Result<_, String>` signatures. On `apps/lotus-explore-rs/src/download/error.rs` (`DownloadError`) and `apps/lotus-explore-rs/src/upload.rs` (`UploadError`), target-exclusive variants (`Io`/`InvalidInput` on native; `Browser`/`InMemoryLimit`/`BlobCreate`/`Dom`/`UnsupportedFileType`/`NotABlob` on `wasm32`) carry explicit `#[cfg(...)]` attributes so both targets compile cleanly with zero `dead_code` lint suppressions. Preserved exact error `.to_string()` text to keep UI/log messages and existing test assertions unchanged.
5. **No new external dependencies**
   - Implemented deterministic property tests across `lotus-model`, `lotus-explore-core`, `lotus-curation`, and `lotus-query` using small crate-local PRNGs/table sweeps rather than adding `proptest`/`quickcheck` to `Cargo.lock`.

## Intentional Behavior Changes

- **None intentionally altered externally.** Existing characterization/golden tests across SPARQL query construction, CSV/JSON/RDF exports, URL parameter codecs, curation parsing, QuickStatements emission, and HTTP server endpoints remain unchanged in their assertions.
- **Documentation/error typing:** Several crate/app helper APIs that previously returned `Result<_, String>` now return typed error enums implementing `std::fmt::Display` and `std::error::Error` with identical formatted messages.
- **Hot-path filter short-circuit:** `lotus_model::contains_folded` now returns `false` upfront when an ASCII haystack is tested against a folded needle containing non-ASCII codepoints, matching prior behavior while avoiding unnecessary window scans.

## Remaining Debt (Ranked)

1. **Larger UI & JS-glue modules in `apps/lotus-explore-rs` (medium priority, intentionally deferred)**
   - Files over 300 code lines remaining in the workspace are concentrated in declarative Dioxus views (`components/results_graph/mod.rs`, `inspector.rs`, `sections/mod.rs`, `download_actions.rs`, `routes.rs`), static translation tables (`i18n/faq.rs`, `faq_guide.rs`, `i18n/mod.rs`), app curation orchestration (`enrichment.rs`, `wikidata.rs`), and browser interop guarded by source-shape gates (`download/file_sink.rs`, `download/wasm.rs`, `upload.rs`, `crates/lotus-search/src/reqwest_client/wasm.rs`).
   - Further decomposition of browser OPFS/streaming interop must respect the textual assertions in `opfs_write_stream.rs`, `opfs_availability_gate.rs`, and `streaming_gate.rs`.
2. **Transitive dependency version splits from Dioxus 0.7 (`docs/DEBT.md`)**
   - `Cargo.toml` retains `[workspace.lints.clippy] multiple_crate_versions = "allow"`, paired with the 11 audited skips in `deny.toml` (`block-buffer`, `const-serialize*`, `cpufeatures`, `crypto-common`, `digest`, `getrandom`, `rustc-hash`, `sha2`, `syn`, `wasm-streams`). Re-audit when upgrading Dioxus.
3. **Target-conditional `apps/lotus-explore-rs` lint exceptions (`docs/DEBT.md`)**
   - `clippy::empty_enums` in `apps/lotus-explore-rs/src/main.rs` (Dioxus `Props` macro under clippy 1.100), `unreachable_code` on `mod route_table` in `src/app/routes.rs`, and `unreachable_pub` on `mod modules` in `build.rs` (compiled both standalone and inside `build_test.rs`).
4. **Full multi-million-row sort scaling**
   - `bench_sort_scaling` shows sub-40 ns/row up to 1M rows (21.1 ms at 1M) and 1,002–1,429 ns/row at 2M–2.99M rows (`2.0s` / `4.3s`). Worth profiling dictionary collation caching separately if interactive client-side sorts above 1M rows become common.

## What to Scrutinize First

1. **Workspace layering & dependency boundaries:** [`Cargo.toml`](Cargo.toml), [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), [`crates/lotus-export/Cargo.toml`](crates/lotus-export/Cargo.toml), and [`crates/lotus-server/Cargo.toml`](crates/lotus-server/Cargo.toml) (`lotus-explore-core` no longer depends on `csv`, `flate2`, or `tar`; pure domain crates compile cleanly for `wasm32`).
2. **Shared query builder & curation unification:** [`crates/lotus-query/src/query/builders.rs`](crates/lotus-query/src/query/builders.rs) (`base_query_for_criteria`), [`crates/lotus-search/src/search/query_plan.rs`](crates/lotus-search/src/search/query_plan.rs), and [`crates/lotus-curation/src/quickstatements.rs`](crates/lotus-curation/src/quickstatements.rs).
3. **Typed error boundaries & OPFS gates:** [`apps/lotus-explore-rs/src/download/error.rs`](apps/lotus-explore-rs/src/download/error.rs), [`apps/lotus-explore-rs/src/download/file_sink.rs`](apps/lotus-explore-rs/src/download/file_sink.rs), [`apps/lotus-explore-rs/src/upload.rs`](apps/lotus-explore-rs/src/upload.rs), and [`crates/lotus-search/src/reqwest_client/mod.rs`](crates/lotus-search/src/reqwest_client/mod.rs).
4. **Domain newtypes & property coverage:** [`crates/lotus-model/src/qid.rs`](crates/lotus-model/src/qid.rs), [`crates/lotus-model/src/columnar/filter.rs`](crates/lotus-model/src/columnar/filter.rs), [`crates/lotus-model/src/columnar/tests.rs`](crates/lotus-model/src/columnar/tests.rs), and [`crates/lotus-explore-core/src/explore/url_codec/tests.rs`](crates/lotus-explore-core/src/explore/url_codec/tests.rs).
