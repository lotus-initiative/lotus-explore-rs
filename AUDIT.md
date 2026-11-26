# LOTUS Explorer Scientific Code Audit

Baseline commit: `fe50374`  
Audit tip before this report: `ace72bd`  
Total tracked files accounted for: **790** (779 reviewed, 11 N/A with explicit justification)

---

## 1. Executive Summary & Scientific Specification

`lotus-explore-rs` queries, exports, visualizes, and helps curate the **LOTUS natural-product knowledge graph** as represented in Wikidata.

Core scientific and data-model invariants:

- **Occurrence model:** Each occurrence row links a chemical compound (`?compound`, `Q...`), a taxon (`?taxon`, `Q...`), and a scholarly reference (`?r`, `Q...`) through a reified `p:P703` / `ps:P703` statement with provenance `prov:wasDerivedFrom / pr:P248`, excluding `wikibase:DeprecatedRank`.
- **Distinctness semantics:** `n_entries` counts returned rows; `n_entries_unique` counts distinct `(compound, taxon, reference)` triples; `n_compounds`, `n_taxa`, and `n_references` count distinct entities in each category (`lotus_model::DatasetStats`).
- **Chemical identifiers & representations:**
  - `InChIKey` (`P235`): standard 27-character `14-10-1` uppercase format (with explicit rejection of 2-block prefixes).
  - `SMILES` (`P233` canonical / `P2017` isomeric): case-sensitive (`c` aromatic vs `C` aliphatic, `Co` cobalt vs `CO` carbonyl); stereochemical marks `@`, `/`, `\` route RDF triples to `wdt:P2017`, otherwise `wdt:P233`.
  - Molfile (`V2000`/`V3000`): 3-line header block may start with blank lines that must not be stripped across normalization, URL encoding, or cache keys.
  - Molecular formula (`P274`): Hill-system ordering (`C`, `H`, remaining elements alphabetically, e.g. `C7H5NO5`); atom-count ceiling bounds `element_max::{C=512, H=1024, N=256, O=256, P=128, S=64}`.
- **Units & parameter domains:**
  - Exact molecular mass (`P2067`): daltons (`Da`, `U483261`), finite real range `[0.0, MASS_MAX]` where `MASS_MAX = 10_000.0` Da.
  - Publication year (`P577`): integer year in `[YEAR_MIN, current_year]` (`YEAR_MIN = 1800`; negative/BCE values formatted per `xsd:gYear` `-YYYY` in RDF Turtle exports).
  - Tanimoto similarity threshold (`similarity_threshold` / `--threshold`): finite score in `(0.0, 1.0]` (`structure_threshold > 0.0 && structure_threshold <= 1.0`).
- **Taxonomic nomenclature closure:** Traversal over Wikidata parent links (`P171*`) combined with four independent bidirectional name-relation pairs: accepted name/synonyms (`P1420`/`P12763`), basionym (`P566`/`P12766`), original combination/protonym (`P1403`/`P12765`), and replacement name (`P694`/`P12764`), deliberately excluding hybrid parentage (`P1531`).
- **Reproducibility & transport semantics:**
  - Upstream query timeouts (`QLever` 429 `"Operation timed out"`, `WDQS` 504 `gateway_timeout`/`backend_timeout`, or legacy `TimeoutException`) map to non-retryable `QueryTooExpensive` rather than retrying or failing over to a second endpoint.
  - Wikimedia rate-limit `429` responses remain retryable rate limits and never receive `LOTUS_QLEVER_TOKEN` `Authorization: Bearer` headers.

---

## 2. Canonical Domain Glossary

| Canonical Term | Code / Schema Identifier | Definition & Units |
| --- | --- | --- |
| Compound | `CompoundEntry`, `compound_qid` | Wikidata chemical entity carrying optional `InChIKey` (`P235`), SMILES (`P233`/`P2017`), `InChI` (`P234`), formula (`P274`), and mass (`P2067`, Da). |
| Taxon | `TaxonResolution`, `TaxonNomenclature` | Biological taxon (`Q...`) matched by QID, scientific name (`P225`), or common name (`P1843`) plus enabled nomenclatural links and descendants. |
| Reference | `reference`, `ReferenceMatch` | Scholarly work identified by Wikidata QID or uppercase bare DOI (`P356`). |
| Occurrence | `occurrences_query`, `P703` | Compound–taxon claim (`p:P703`/`ps:P703`) excluding `wikibase:DeprecatedRank`, optionally qualified with provenance `S248` (`P248`). |
| Search Criteria | `SearchCriteria`, `validate_criteria` | Validated filter state constructed via `SearchCriteria::up_to_year(year)`. |
| Monoisotopic / Molecular Mass | `mass_min`, `mass_max`, `MASS_MAX` | Molecular mass in daltons (`Da`), `[0.0, 10000.0]`. |
| Tanimoto Threshold | `structure_threshold`, `similarity_threshold` | IDSM Tanimoto cutoff strictly in `(0.0, 1.0]`; `1.0` for exact structure lookups. |
| Result Set | `ColumnarResultSet` | Dictionary-compressed occurrence rows preserving multiplicity (`n_entries`) and distinct triples (`n_entries_unique`). |
| QuickStatements Bundle | `QuickStatementsBundle` | Split dependency (`dependencies`) and compound/occurrence (`main`) V1 batches for Wikidata curation. |

---

## 3. Module Map & Data Flow

1. **Pure Domain (`crates/lotus-model`, `crates/lotus-query`, `crates/lotus-curation`, `crates/lotus-jsonld`):**
   Zero ambient clocks, no HTTP clients; wasm32-verified. Owns identifiers, `SearchCriteria`, validation, columnar dictionary storage, SPARQL builders/rewriters, streaming CSV parser, row exporters (CSV, SPARQL JSON, RDF Turtle), TSV/CSV curation parsing, and JSON-LD/CodeMeta/`CITATION.cff` emitters.
2. **Application Core (`crates/lotus-search`, `crates/lotus-export`, `crates/lotus-explore-core`):**
   Transport-abstracted (`Http` trait) search execution, endpoint failover, retry rules, bulk export pipelines (frozen CSV, PubChem trees, Wikidata SMILES archive), explorer reducer state machine, bidirectional URL query codec (`CriteriaQueryDto`), client filter/sort, export metadata builder, and graph projection.
3. **Delivery Adapters (`crates/lotus-cli`, `crates/lotus-server`, `crates/lotus-web-assets`, `apps/lotus-explore-rs`):**
   - `lotus` CLI (`search`, `curate`, `export`, `export-pubchem-tree`, `archive`, `completions`, `man`).
   - `lotus-server` Axum OpenAPI server (`/v1/search`, `/v1/export-url`, `/v1/export-file/{cache_key}/{format}`, `/health`, `/metrics`) with single-flight coalescing, bounded LRU+TTL cache, and streaming gzip export.
   - `lotus-explore-rs` Dioxus WASM client with RDKit/Ketcher/Citation.js/WebGL2/WebMCP bridges.

---

## 4. Findings & Fixes Ledger

Each batch below corresponds to one atomic, verified commit on `main` after `fe50374`:

| Commit | Scope | Key Scientific, Semantic & Implementation Fixes | Verified Behavioral / Numerical Changes |
| --- | --- | --- | --- |
| `1194fc5` | `lotus-model` | Normalized QIDs/DOIs strictly (`identify.rs`, `reference.rs`); preserved Molfile blank header lines and structure case (`structure.rs`); hardened columnar bitmask/statement-ID/UUID bounds and UTC validation (`utc.rs`, `validate.rs`). | Rejects invalid/non-finite bounds, inverted ranges, and malformed statement/QID tokens instead of wrapping or silently passing. |
| `b5bd9d9` | `lotus-query` (+ matrix test sync) | Fixed `with_filters` placement inside optional groups (`filters.rs`); corrected `counts_query`/`wdqs_fallback` and streaming CSV BOM/row/count parsing (`counts.rs`, `stream/`); routed chiral/isomeric SMILES to `wdt:P2017` and negative/padded years (`xsd:gYear`) with RFC 3986 IRI escaping in Turtle exports (`turtle.rs`); fixed bulk export query filters (`queries_core.rs`, `queries_enrich.rs`). | Exact Turtle predicate split (`P233` vs `P2017`), four-digit/negative `xsd:gYear` literals, strict count parse errors instead of zero fallbacks, accurate wildcard/reference matrix `P703` scoping. |
| `cb8b5e4` | `search`, `curation`, `jsonld` | Exact QLever host matching (`budget.rs`) preventing token leakage or misrouting; excluded `wikibase:DeprecatedRank` in curation `P703` ASK/batch queries (`occurrence.rs`); merged duplicate `CREATE` blocks for both `NewCompound` and `PendingDependencies` without resolved QID (`quickstatements.rs`); updated EDAM InChIKey IRI from `format_1198` (SMILES) to `format_1199` (`edam.rs`). | Metadata/JSON-LD now emits canonical EDAM `format_1199` for InChIKey; duplicate InChIKey pending rows emit one `CREATE` block; deprecated `P703` claims no longer suppress curation suggestions. |
| `633197e` | `lotus-export` | Prevented biological/chemical tree cycle drops, cross-tree ID collisions, and blank-cell clobbering during join enrichment (`pubchem_tree/`, `pipeline/index.rs`); enforced UTF-8 BOM stripping and strict header validation; stabilized deduplication/tar ordering (`archive/tar_writer.rs`). | Non-empty cell fields are never overwritten by blank cells; graph cycles in Wikidata `P279` / taxonomy no longer drop reachable nodes silently. |
| `0627eb4` | `lotus-cli` | Fixed `--year-max` filtering (`criteria.rs`), half-open `Range` upper bounds (`args/`), TSV control/tab escaping (`output.rs`), `search`/`export`/`curate` validation, blank-SMILES curation rejection (`curate/runner.rs`), and Zenodo error paths (`zenodo/`). | `--year-max` propagates instead of falling back; blank SMILES fail per-row cleanly; half-open `A..B` bounds enforce exact validation. |
| `06779c5` | `lotus-server` | Replaced duplicated QLever URL with `lotus_search::Endpoint`; percent-encoded `cache_key` export paths (`export_urls.rs`); fixed unbracketed IPv6 bind parsing (`config.rs`); removed `[0.05, 1.0]` clamping so finite `(0.0, 1.0]` validates strictly (`query_logic.rs`); enforced cache TTL on read and immediate capacity eviction on write (`state/cache.rs`); added RAII `InflightGuard` (`state/inflight.rs`); isolated caller `resolved_taxon_qid`/`warning` from shared cache entries (`handlers/search.rs`); acquired `upstream_permits` in `prepare.rs`; POSTed `action=csv_export` in `stream.rs`, mapped `408|504 -> 504` and `429|503 -> 503`, cancelled pumps via `tokio::select!`, propagated `GzEncoder::finish` errors, and excluded `application/gzip` from HTTP re-compression (`main.rs`). | Thresholds in `(0.0, 0.05)` are accepted un-clamped, while `<= 0.0` or non-finite fail `400`; cached searches no longer leak one caller's taxon-resolution warning to another; upstream timeouts return `504`. |
| `3a1bd3a` | `explore-core`, `web-assets` | Rejected backslash/drive-letter/NUL zip slips, required `/` across all non-junk entries in `common_prefix`, and bounded extracted size at 256 MiB (`fetch_assets/`); bracketed IPv6 `Host` headers and checked `HTTP/` response status line in `healthcheck.rs`; mapped HTTP `504`, capitalized `429` timeouts, `5xx` timeouts, and non-rate-limit parse timeouts to non-retryable `QueryTooExpensive` (`transport_classification.rs`); respected `max_retries == 0` (`retry.rs`); preserved `>2`-decimal thresholds in URLs without clamping `< 0.05` and filtered non-finite mass in `url_codec`; fixed `metadata/builder.rs` (`reference`, `taxon_nomenclature`, SACHEM/IDSM credit); added `CenterNotInRows` error & QID label fallback in `graph/project/`; fixed `*` + `reference` unscoped exact queries (`build_query.rs`). | Exact `max_retries=0` performs zero retries; URL round-trips preserve thresholds like `0.03` and precision `>2` decimals; ego graph with zero projected edges returns explicit `CenterNotInRows` instead of empty success. |
| `b1fe1d2` | `lotus-explore-rs` (logic/JS/CSS/build) | Prevented compound-only rows (`dependencies: None`) from emitting `P703|[NEW_TAXON_QID]` and extracted pure `build_new_compound_row` (`enrichment.rs`); chunked (`100`) curation Wikidata queries with deterministic lowest `qid_number` tie-breaking (`wikidata.rs`); guarded concurrent `process()`/`run_second_pass()` clicks (`page_controller.rs`); awaited `createWritable()` Promise and surfaced `DownloadError::Cancelled` on Save-dialog abort (`download/`); added `has_fetched_rows` flag so `?download=true` passes `None` instead of 0-row set (`download_effects/dispatch.rs`); handled explicit `data-lotus-base-path="/"` (`url_state.rs`); JSON-encoded `eval` interpolations (`shell.rs`); fixed JSON bracket scanning in `split_decoded_query` (`routes.rs`); scoped `:root` dark CSS vars inside `@media (prefers-color-scheme: dark)` + regenerated CSS; fixed `webmcp.js` table snapshot sync, `citation-bridge.js` timeouts, and `lotus-graph-gl.js` $O(N+E)$ star layout, coprime repulsion stride, 2D wheel zoom drift, and WebGL teardown; removed dead `build.rs` path and added Crossref/Crosscite/DataCite to CSP `connect-src`. | Compound-only new/existing curation rows emit zero `P703` placeholders; Wikidata entity ties select numerically smallest QID deterministically regardless of SPARQL row order; Save-dialog cancel stops without silent fallback. |
| `d976ae0` | `lotus-explore-rs` (UI/i18n) & `lotus-model` | Switched `DownloadActionsGroup` `toolbar_model` from static `use_signal` to reactive `use_memo` (`download_actions.rs`); replaced bogus `ExampleApiUrls` copy in `DownloadDispatchState`/`DownloadOnlyState` with `LoadingHint` and localized `DownloadOnlyHint` across En/Fr/De/It (`loading.rs`, `i18n/`); bound `NumPair` maxes to `lotus_model::element_max::*` and `MassRangeInput` max to `pub lotus_model::MASS_MAX` with `step="any"`; corrected non-Hill formula `C7H5O5N` -> `C7H5NO5` and aligned `similarity_threshold` bounds (`exclusiveMinimum: 0`, `min: "0.01"`) in `search_panel.rs` + `webmcp.js`; fixed `DuplicateRowsExplain` across all 4 locales to describe compound–taxon–reference triples vs compounds accurately; guarded `format_mass` against non-finite floats (`curation_results_table.rs`); aligned `*` row in `faq_guide.rs` with required `wdt:P703` occurrence semantics. | Export toolbar updates filenames/endpoint on every search; HTML5 form constraints match server/model validation (`MASS_MAX`, `element_max`, `> 0.0` threshold); Hill formula order and triple statistics copy match domain definitions. |
| `4bc26f5` | CI, Docker, build | Updated `Makefile.toml` `boundary` task to inspect non-comment `.toml` lines; added `--locked` to `.github/workflows/archive.yml`; verified `/network/index.html`, `/faq/index.html`, and `citation.js` in `.github/workflows/deploy.yml`; aligned `Dockerfile` `cli` stage to `distroless/cc-debian13:nonroot`. | Prevents `dioxus` dependency introduction in `Cargo.toml` manifests; enforces lockfile in archive builds; closes GitHub Pages export verification gaps. |
| `ace72bd` | Docs, tests & slop cleanup | Removed `comprehensive`, `leverage`, and `simply` across docs, comments, and NPClassifier metadata; updated `docs/cli.md` threshold range `(0, 1]`, `docker/snippets/security-headers.conf` pointer, `.github/ai/PITFALLS.md`, and stale item references flagged by `refquiem` (`tests.rs`, `nomenclature_flag_tests.rs`, `DESIGN_SYSTEM.md`, `local_file.rs`, `archive/fetch.rs`). | Zero `refquiem` warnings/errors and zero banned-vocabulary hits across source and docs. |

---

## 5. Open Questions

The following behaviors are documented or intentional system constraints whose scientific policy trade-off is left open for repository maintainers rather than changed silently:

1. **Wildcard taxon (`*`) + structure filter leaves occurrence `OPTIONAL`:**  
   `docs/QUERY_MATRIX.md` documents that `taxon="*"` alone requires `P703`, whereas `taxon="*"` combined with `--structure` / structure input keeps occurrences `OPTIONAL` rather than routing through root taxon `wd:Q2382443` (to avoid dropping compounds without occurrence statements on large structure queries). Decide whether `--taxon "*"` + `--structure` should strictly require at least one `P703` triple for consistency with `--taxon "*"` alone.
2. **Unpinned asset versions & checksums in `lotus-web-assets`:**  
   By default `fetch-assets` pulls latest upstream RDKit and Ketcher releases without pinned SHA-256 verification (`.github/ai/PITFALLS.md` Known gaps). Pinning default versions plus checksum manifests would eliminate upstream drift risk during Docker/CI builds.
3. **In-memory `lotus export` & in-place CLI output writes:**  
   `lotus export` joins the four SPARQL tables in RAM (`crates/lotus-export/src/pipeline/`), and CLI output writers overwrite target paths in place rather than via temp-file rename. Streaming disk-backed join / atomic temp-file writes remain future enhancements.
4. **Upstream Wikidata synonym transitivity (`Salvia` -> `Mentha`):**  
   `i18n/faq_guide.rs` notes that following Wikidata `P1420` across `Salvia` -> `Audibertia` -> `Mentha` widens results when `accepted_synonyms` is enabled, reflecting live Wikidata statements rather than a local bug.
5. **Exact structure matching via IDSM similarity `1.0`:**  
   Raw SMILES/Molfile exact searches query IDSM at Tanimoto `1.0` (which can return stereoisomers and close isomers) because the SPARQL/web search path does not locally compute `InChIKey` from arbitrary SMILES/Molfiles before querying `P235`.

---

## 6. LLM Slop Removal Log

- **Deleted untracked repository debris:** `apps/lotus-explore-rs/.DS_Store`, `crates/lotus-web-assets/public/assets/vendor/rdkit/RDKit_minimal.js`.
- **Dead build rule removed:** Nonexistent `crates/lotus/src` watch path removed from `apps/lotus-explore-rs/build.rs`.
- **Filler / marketing prose stripped:**
  - `crates/lotus-export/src/pubchem_tree/document.rs`: `"comprehensive classification"` -> `"three-level classification"`.
  - `apps/lotus-explore-rs/src/api/config.rs`: `"to leverage"` -> `"to use the"`.
  - Removed filler `"simply"` from `docs/FRONTENDS.md`, `docs/SPARQL-VARIANTS.md`, `docs/STRUCTURE-SEARCH.md`, `docs/TAXON-SEARCH.md`, `apps/lotus-explore-rs/docs/PERFORMANCE.md`, `apps/lotus-explore-rs/public/assets/js/curation/rdkit-bridge.js`, `apps/lotus-explore-rs/src/features/explore/service/resolve_structure.rs`, `apps/lotus-explore-rs/src/features/explore/structure_cache.rs`, `apps/lotus-explore-rs/src/components/faq/tests.rs`, `apps/lotus-explore-rs/src/download/streaming_gate.rs`, `apps/lotus-explore-rs/src/i18n/faq_guide.rs`, `crates/lotus-query/src/query/lookups_compound.rs`, `crates/lotus-query/src/export_rows/tests.rs`, `crates/lotus-query/tests/contract/export.rs`, `crates/lotus-query/tests/export_fidelity.rs`, `crates/lotus-query/tests/streaming.rs`, `crates/lotus-search/src/error.rs`, `crates/lotus-search/tests/search_pipeline.rs`, `crates/lotus-curation/src/knowledge/tests.rs`, `crates/lotus-model/tests/diagnostics.rs`, `crates/lotus-web-assets/src/healthcheck.rs`.
- **Wrong/placeholder UI strings & comments replaced:**
  - Replaced accidental `TextKey::ExampleApiUrls` placeholder in `DownloadDispatchState` / `DownloadOnlyState` (`apps/lotus-explore-rs/src/components/loading.rs`) with `LoadingHint` and localized `DownloadOnlyHint`.
  - Replaced stale comment path `apps/lotus-explore-rs/src/server/mod.rs` with `crates/lotus-server/src/main.rs` in `docker/snippets/security-headers.conf`, and resolved all stale/changed doc-comment symbol mentions (`refquiem`).

---

## 7. Complete File Coverage Ledger

Every file in `git ls-files` (790 files) is listed below:

| File | Status | Notes |
| --- | --- | --- |
| `.cargo/audit.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.cargo/mutants.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.config/nextest.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.dockerignore` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.echoes.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.editorconfig` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.gitattributes` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/CODEOWNERS` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/PULL_REQUEST_TEMPLATE.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/actions/setup/action.yml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/ai/.mcp.json` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/ai/AGENTS.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/ai/PITFALLS.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/dependabot.yml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/workflows/archive.yml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/workflows/ci.yml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/workflows/deploy.yml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/workflows/nightly.yml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.github/workflows/release.yml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.gitignore` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.graphifyignore` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.mk/catalog.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `.mk/core.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `.mk/gates.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `.mk/meta.py` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `.mk/mutants.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `.mk/mutants_plan.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `.mk/nightly.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `.mk/tier.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `.mk/tools.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `.refquiem.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.tombi.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.typos.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `.vscode/settings.json` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `AGENTS.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `AUDIT.md` | Reviewed | Audit findings ledger, glossary, open questions, and 100% file coverage table. |
| `CITATION.cff` | Reviewed | Checked under repository audit. |
| `CLAUDE.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `CONTRIBUTING.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `Cargo.lock` | N/A | Generated dependency lockfile enforced via `--locked` across builds and CI. |
| `Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `Dockerfile` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `LICENSE` | N/A | Upstream AGPL-3.0-only license text. |
| `Makefile.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `SECURITY.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/.gitignore` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/Dioxus.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/build.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/build/ai.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/build/csp.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/build/headers.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/build/metadata.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/build/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/build/site_files.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/build/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/build_test.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/docs/ARCHITECTURE.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/docs/CITATION.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/docs/CURATION_SHARE_LINKS.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/docs/DEPLOYMENT.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/docs/DESIGN_SYSTEM.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/docs/PERFORMANCE.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/index.html` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/metadata/site-metadata.json` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `apps/lotus-explore-rs/public/.well-known/agent-skills.json` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/.well-known/ai-catalog.json` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/.well-known/api-catalog.json` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/.well-known/apple-app-site-association` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/.well-known/security.txt` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/_headers` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/_redirects` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/android-chrome-192x192.png` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/android-chrome-512x512.png` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/apple-touch-icon.png` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/assets/js/curation/citation-bridge.js` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/assets/js/curation/rdkit-bridge.js` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/assets/js/graph/lotus-graph-gl.js` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/assets/js/webmcp.js` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/assets/lotus-explore.css` | Reviewed | Generated compiled stylesheet; regenerated via `./mk web-css` after light-mode FOUC fix in `tailwind/styles.css`. |
| `apps/lotus-explore-rs/public/docs/references.bib` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/favicon-16x16.png` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/favicon-32x32.png` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/favicon-dark.svg` | N/A | Static vector favicon asset. |
| `apps/lotus-explore-rs/public/favicon-light.svg` | N/A | Static vector favicon asset. |
| `apps/lotus-explore-rs/public/favicon.ico` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/favicon.svg` | N/A | Static vector favicon asset. |
| `apps/lotus-explore-rs/public/humans.txt` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/llms.txt` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/robots.txt` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/site.webmanifest` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/public/sitemap.xml` | Reviewed | Static web asset, JS bridge, CSP/header/redirect config, or site metadata checked against build generators. |
| `apps/lotus-explore-rs/src/api.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/api/client.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/api/client/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/api/config.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/api/config/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/app/bootstrap.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/app/bootstrap/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/app/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/app/routes.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/app/routes/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/app/shell.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/app_state.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/app_state/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/cache.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/cache/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/clock.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/clock/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/copy_button.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/curation_results_table.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/data_curation_page.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/data_curation_page/sections.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/data_curation_page/sections/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/faq.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/faq/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/form_inputs.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/form_sections.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/form_sections/basic_sections.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/form_sections/entity_group.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/form_sections/entity_group/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/form_sections/field_examples.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/form_sections/field_examples/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/form_sections/formula_section.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/form_sections/shared.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/form_sections/shared/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/dark_mode_toggle.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/footer.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/footer/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/header_meta.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/header_meta/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/lang_switch.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/notices.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/page_header.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/simple_analytics_badge.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/structured_data.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/structured_data/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/layout/view_switch.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/loading.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/loading/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/not_found.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_graph/bridge.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_graph/canvas_stage.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_graph/empty_starter.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_graph/history.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_graph/inspector.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_graph/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_graph/presentation.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_graph/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_graph/toolbar.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/download_model.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/download_model/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/header_model.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/header_model/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/render_model.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/render_model/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/compound.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/numeric.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/numeric/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/reference.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/structure.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/taxon.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/prepared.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/prepared/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/render.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/row_text.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/row_text/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/scroll_runtime.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/scroll_runtime/frame_closure_safety.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/scroll_runtime/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/scroll_signals.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/sort_model.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/sort_model/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/table_filter_row.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/table_filter_row/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/table_header.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/table_toolbar_sections/download_actions.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/table_toolbar_sections/filter_status.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/table_toolbar_sections/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/table_toolbar_sections/query_panel.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/table_toolbar_sections/stat_bar.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/table_view_model.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/table_view_model/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/toolbar.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/virtualization_controller.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/virtualization_controller/measurement_frame_safety.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_table/virtualized_table.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_viewport.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/results_viewport/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/search_panel.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/search_panel/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/ui/button.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/ui/button/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/ui/card.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/ui/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/ui/section_heading.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/components/welcome.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/curation.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/curation/contract_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/curation/share_links.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/curation/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/document_head.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/document_head/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/blob_limit_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/error.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/file_sink.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/local_file.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/local_file/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/native.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/opfs_availability_gate.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/opfs_write_stream.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/sink_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/streaming_gate.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/download/wasm.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/page_controller.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/page_controller/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/queue.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/queue/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/repositories/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/repositories/wikidata.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/chemical.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/chemical/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/enrichment.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/enrichment/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/http_client.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/http_client/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/inputs.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/occurrence.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/occurrence/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/occurrence_cache.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/occurrence_cache/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/pipeline.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/pipeline/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/prefetch.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/prefetch/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/quickstatements.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/quickstatements/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/reference_metadata.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/reference_metadata/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/wikidata.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/services/wikidata/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/workflow.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/curation/workflow/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/download_dispatch.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/download_effects/dispatch.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/download_effects/metrics.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/download_effects/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/download_effects/startup.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/download_effects/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/executor.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/interactions.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/lifecycle.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/lifecycle/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/controller.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/controller/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/runtime.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/runtime/test_exports.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/search_state.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/search_state/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/selectors.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/selectors/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/api_pipeline.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/api_pipeline/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/native.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/native/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/no_limit_gate.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/progress.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/progress/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/wasm.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_reference.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_reference/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_structure.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_structure/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/match_selection.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/match_selection/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/results_pipeline/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/results_pipeline/plan.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/service/results_pipeline/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/structure_cache.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/structure_cache/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/taxon_cache.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/taxon_cache/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/url_state.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/explore/url_state/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/features/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/hooks/locale_provider.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/hooks/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/hooks/use_add_row_form.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/hooks/use_locale.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/hooks/use_virtualization.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/hooks/use_virtualization/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/curation/de.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/curation/en.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/curation/fr.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/curation/it.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/curation/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/de.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/en.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/faq.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/faq/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/faq_guide.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/faq_guide/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/fr.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/it.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/messages/aria.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/messages/errors.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/messages/errors/de.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/messages/errors/en.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/messages/errors/fr.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/messages/errors/it.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/messages/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/i18n/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/main.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/pages/draw_page.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/pages/ketcher_panel.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/pages/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/pages/network_page.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/perf.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/perf/clock.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/perf/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/repositories/hybrid.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/repositories/hybrid/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/repositories/mock.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/repositories/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/services/app_services.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/services/app_services/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/services/error_presenter.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/services/error_presenter/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/services/event_log.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/services/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/services/search_telemetry.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/sparql.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/state.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/state/form_context.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/state/form_context/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/state/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/ui/a11y_contract.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/ui/a11y_smoke.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/ui/a11y_smoke/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/ui/content_phase.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/ui/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/ui/notice.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/ui/segmented_control.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/ui/segmented_control/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/upload.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/upload/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/vendor_assets.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/src/vendor_assets/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/tailwind/styles.css` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/tests/doc_links.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/tests/gate/repo_hygiene.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/tests/gate/repo_hygiene/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/tests/gate/scrapers.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `apps/lotus-explore-rs/tests/gate_consistency.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `bacon.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `clippy.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `codemeta.json` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `compose.yaml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-cli/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-cli/README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-cli/src/archive_cmd.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/args/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/args/search.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/clock.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/criteria.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/curate/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/curate/report.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/curate/runner.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/curate/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/export_cmd.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/main.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/main/archive_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/main/clock_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/main/export_progress_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/main/nomenclature_flag_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/main/range_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/output.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/output/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/search_cmd.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/zenodo/client.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/zenodo/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/zenodo/publish.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/zenodo/record.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/src/zenodo/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/tests/cli.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/tests/common/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-cli/tests/docs_in_sync.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-curation/README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-curation/src/error.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/error/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/input.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/input/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/knowledge/dependencies.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/knowledge/lookup.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/knowledge/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/knowledge/result_row.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/knowledge/statements.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/knowledge/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/lib.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/quickstatements.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/quickstatements/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/resolution.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/row.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/row/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/structure/convert.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/structure/formula.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/structure/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/structure/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/wikidata.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/wikidata_query/compound.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/wikidata_query/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/wikidata_query/occurrence.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/wikidata_query/reference.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/wikidata_query/statements.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/wikidata_query/taxon.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/wikidata_query/taxon_name.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-curation/src/wikidata_query/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-explore-core/README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-explore-core/src/api/dto.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/api/dto/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/api/error.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/api/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/actions.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/command.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/command/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/error.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/error/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/error_recovery.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/error_recovery/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/form_actions.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/form_actions/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/form_validation.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/form_validation/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/hashing.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/hashing/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/outcome.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/recovery.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/recovery/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/request.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/request/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/retry.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/retry/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/search_metrics.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/search_metrics/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/search_state/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/search_state/reducer/lifecycle.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/search_state/reducer/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/search_state/reducer/result_data.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/search_state/reducer/ui.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/search_state/state_types.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/search_state/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/service/build_query.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/service/build_query/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/service/finalize.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/service/finalize/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/service/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/service/strategy.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/service/strategy/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/sparql_errors.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/sparql_errors/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/transport_classification.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/transport_classification/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/truncation.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/truncation/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/url_codec/criteria/decode.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/url_codec/criteria/encode.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/url_codec/criteria/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/url_codec/encode.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/url_codec/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/url_codec/network.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/url_codec/startup.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/explore/url_codec/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/export/filename.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/export/filename/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/export/filters.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/export/filters/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/export/metadata/builder.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/export/metadata/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/export/metadata/schema.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/export/metadata/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/export/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/filters.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/filters/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/model/kinds.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/model/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/model/payload.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/model/starters.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/project/edges.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/project/edges/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/project/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/project/nodes.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/project/search.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/graph/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/lib.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/ports.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/ports/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/sort.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/table_budget.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/src/table_budget/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-explore-core/tests/characterization_exports.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-export/README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-export/src/archive/dedup.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/archive/fetch.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/archive/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/archive/tar_writer.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/archive/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/lib.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pipeline/index.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pipeline/join.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pipeline/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pipeline/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pubchem_tree/compact.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pubchem_tree/document.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pubchem_tree/fetch.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pubchem_tree/index.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pubchem_tree/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pubchem_tree/nodes.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pubchem_tree/npclassifier.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pubchem_tree/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/src/pubchem_tree/trees.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-export/tests/characterization_exports.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-jsonld/README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-jsonld/src/bin/emit-metadata.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/bin/emit-metadata/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/compound.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/compound/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/dataset.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/dataset/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/edam.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/edam/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/lib.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/profile.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/profile/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/profile_bounds_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/software.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/software/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/taxon.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/taxon/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/src/uri_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-jsonld/tests/emit_metadata_cli.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-model/README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-model/src/columnar/access.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/bitmask.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/builder.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/builder_seed.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/diagnostics.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/filter.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/qid_dict.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/sort.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/sparse_strings.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/statement_id.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/columnar/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/criteria.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/criteria/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/endpoints.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/endpoints/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/entry.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/identify.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/identify/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/lib.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/percent.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/percent/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/reference.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/reference/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/stats.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/stats/from_entries_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/stats/spelling_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/stats/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/structure.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/structure/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/taxon_nomenclature_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/testing.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/utc.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/utc/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/validate.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/src/validate/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/tests/columnar.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/tests/diagnostics.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-model/tests/sentinels.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-query/README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-query/examples/measure_export.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/bulk_export/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/bulk_export/parse_id.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/bulk_export/queries_core.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/bulk_export/queries_enrich.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/bulk_export/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/error.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/export.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/export/log_name_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/export/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/export_rows/csv.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/export_rows/json.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/export_rows/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/export_rows/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/export_rows/turtle.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/lib.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/columns.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/compounds.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/counts.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/lookups.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/stream/fields.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/stream/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/stream/reader.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/stream/splitter.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/stream/splitter/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/parse/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/builders.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/builders_body.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/compound_names.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/dictionary.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/filters.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/lookups_compound.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/lookups_taxon.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/nomenclature.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/prefixes.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/rewrite.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/structure.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/src/query/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/bench.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/characterization_query_surface.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/contract/common.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/contract/export.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/contract/filters.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/contract/nomenclature.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/contract/structure.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/contract/structure_rules.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/export_fidelity.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/fixtures/compounds.csv` | Reviewed | Test fixture; compound fixture updated to keep row multiplicity tests explicit. |
| `crates/lotus-query/tests/fixtures/counts.csv` | Reviewed | Test fixture; compound fixture updated to keep row multiplicity tests explicit. |
| `crates/lotus-query/tests/fixtures/taxon.csv` | Reviewed | Test fixture; compound fixture updated to keep row multiplicity tests explicit. |
| `crates/lotus-query/tests/query_contract.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/query_rewrites.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/reference_node.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/result_parsing.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/streaming.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-query/tests/truncation_notice.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-search/README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-search/src/client.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/client/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/error.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/error/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/execute/budget.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/execute/endpoints.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/execute/error_format.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/execute/fallback.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/execute/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/execute/retry.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/execute/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/execute/wasm.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/lib.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/reqwest_client/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/reqwest_client/schedule.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/reqwest_client/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/reqwest_client/transport.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/reqwest_client/wasm.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/result.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/result/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/search/execute_search.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/search/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/search/query_plan.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/search/resolve_reference.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/search/resolve_structure.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/search/resolve_taxon.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/search/stream_search.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/search/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/testing.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/src/testing/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/tests/failure_stages.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/tests/matrix.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/tests/query_building.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/tests/search_columnar.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/tests/search_pipeline.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/tests/streaming_fold.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-search/tests/taxon_resolution.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-server/README.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-server/src/cache_key.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/cache_key/bench.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/cache_key/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/characterization_tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/clock.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/config.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/config/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/dto.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/errors.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/export_urls.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/export_urls/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/handlers/export/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/handlers/export/stream.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/handlers/export/stream/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/handlers/export/url.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/handlers/export_query_shape.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/handlers/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/handlers/prepare.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/handlers/search.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/logging.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/logging/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/main.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/query_logic.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/services.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/state/cache.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/state/inflight.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/state/metrics.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/state/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-server/src/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/Cargo.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `crates/lotus-web-assets/src/fetch_assets/http.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/fetch_assets/http/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/fetch_assets/ketcher.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/fetch_assets/ketcher/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/fetch_assets/mod.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/fetch_assets/test_support.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/fetch_assets/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/fetch_assets/vendor.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/fetch_assets/vendor/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/healthcheck.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/healthcheck/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/inject_wasm_preload.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/src/inject_wasm_preload/tests.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `crates/lotus-web-assets/tests/healthcheck_cli.rs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `deny.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docker/nginx.conf` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docker/snippets/security-headers.conf` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docs/ARCHITECTURE.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docs/DEBT.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docs/FRONTENDS.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docs/QLEVER-POLITENESS.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docs/QUERY_MATRIX.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docs/SPARQL-VARIANTS.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docs/STRUCTURE-SEARCH.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docs/TAXON-SEARCH.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `docs/cli.md` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `examples/curation_example.tsv` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `make/all.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `make/build.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `make/docker.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `make/format.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `make/lint.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `make/metadata.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `make/scripts/docker-timing.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `make/scripts/docker-verify.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `make/scripts/license-headers.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `make/scripts/opt-levels.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `make/scripts/setup.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `make/scripts/web-a11y-audit.mjs` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `make/scripts/web-bytes.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `make/scripts/web-css.sh` | Reviewed | Audited for scientific/domain correctness, error propagation, bounds, determinism, and lint hygiene. |
| `make/supply-chain.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `make/test.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `make/web.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `mk` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `mk.local` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `mk.versions` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `prek.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `renovate.json` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `rust-toolchain.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
| `rustfmt.toml` | Reviewed | Audited configuration, schema, build/CI rule, example dataset, or documentation. |
