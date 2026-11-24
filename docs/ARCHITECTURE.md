# Architecture

The repository is structured around a strict one-way dependency rule across three layers:

1. **Pure Domain** (`wasm32`-checked, zero I/O, no clock, no async runtime)
2. **Application / Use-Case Services** (injectable `Http` transport or synchronous framework-free state; no UI framework)
3. **Adapters / Delivery Front Ends** (Dioxus web client, native Axum HTTP API server, `lotus` CLI, build/deploy host tools)

This layering prevents the same query, validation, or curation logic from being implemented independently per front end, and allows every domain rule to be verified offline.

---

## Crate Map & One-Line Responsibilities

### 11 Workspace Crates (Three Layers)

| Crate | Layer | One-line responsibility | Workspace dependencies |
| --- | --- | --- | --- |
| [`lotus-model`](../crates/lotus-model/README.md) | Pure Domain | Core LOTUS vocabulary, validated identifiers, search criteria, columnar store, and filters | *(none)* |
| [`lotus-query`](../crates/lotus-query/README.md) | Pure Domain | Pure SPARQL query builders, query rewriters, streaming CSV/JSON fold, and row exporters | `lotus-model` |
| [`lotus-jsonld`](../crates/lotus-jsonld/README.md) | Pure Domain | Bioschemas JSON-LD, CodeMeta (`codemeta.json`), and `CITATION.cff` generation | `lotus-model` |
| [`lotus-search`](../crates/lotus-search/README.md) | Application | Transport-abstracted SPARQL execution, retry/fallback, and input resolution | `lotus-model`, `lotus-query` |
| [`lotus-curation`](../crates/lotus-curation/README.md) | Application | Curation TSV ingestion, Wikidata reconciliation queries, and QuickStatements V1 bundling | `lotus-model`, `lotus-search` (`default-features = false`) |
| [`lotus-explore-core`](../crates/lotus-explore-core/README.md) | Application | Framework-free explorer state machine, URL codec, client filter/sort, graph projection, and API DTOs | `lotus-model`, `lotus-query`, `lotus-search` (`default-features = false`), `lotus-jsonld` |
| [`lotus-export`](../crates/lotus-export/README.md) | Application | Full-graph bulk ETL pipelines (frozen CSV export, PubChem trees, Zenodo archive) over injectable `Http` | `lotus-model`, `lotus-query`, `lotus-search` (`default-features = false`) |
| [`lotus-server`](../crates/lotus-server/README.md) | Adapter | Native Axum OpenAPI HTTP server (`/v1/*`, `/health`, `/metrics`, static SPA hosting) | `lotus-model`, `lotus-query`, `lotus-search`, `lotus-explore-core` (`openapi`) |
| [`lotus-cli`](../crates/lotus-cli/README.md) | Adapter | The `lotus` terminal binary (`search`, `curate`, `export`, `export-pubchem-tree`, `archive`) | `lotus-model`, `lotus-query`, `lotus-search`, `lotus-curation`, `lotus-jsonld`, `lotus-export` |
| [`apps/lotus-explore-rs`](../apps/lotus-explore-rs/README.md) | Adapter | Dioxus WASM web client and browser platform adapters | `lotus-model`, `lotus-query`, `lotus-search`, `lotus-curation`, `lotus-explore-core` |
| `lotus-web-assets` | Adapter | Host-only build and container utilities (`fetch-assets`, `inject-wasm-preload`, `lotus-healthcheck`) | *(none)* |

---

## Layering & Boundary Rules (Mechanically Checked)

1. **Pure crates stay pure (`./mk check-wasm-pure`):**
   `lotus-model`, `lotus-query`, and `lotus-jsonld` have zero HTTP, clock, or async runtime dependencies and compile for `wasm32-unknown-unknown` in isolation. Because pure crates cannot read a clock, `SearchCriteria` has no `Default` — callers supply `year` via `SearchCriteria::up_to_year(year)`.
2. **Transformation is separate from transport (`./mk check-wasm-search`):**
   `lotus-query` turns criteria into SPARQL strings and parses CSV bytes into model types. `lotus-search` owns endpoint selection, retry, QLever→WDQS fallback, and resolution over the `Http` trait (tested offline against [`Scripted`](../crates/lotus-search/src/testing.rs)). Crates that only need the `Http` trait (`lotus-curation`, `lotus-explore-core`, `lotus-export`) depend on `lotus-search` with `default-features = false` so `reqwest`/`tokio` are never pulled in unnecessarily.
3. **Application core contains no UI framework (`./mk boundary`):**
   `lotus-explore-core` (and `lotus-export`, `lotus-server`) must never depend on or reference `dioxus`.
4. **Error handling strategy:**
   Library crates and `lotus-server` expose typed `thiserror` error enums carrying structured context (`ValidationError`, `ParseError`, `FetchError`, `CurationError`, `RepositoryError`, etc.) rather than `String` or `anyhow::Error`. `anyhow` is used only at the `lotus-cli` binary edge.
5. **Concurrency & async strategy:**
   - **Pure domain:** synchronous, zero locks.
   - **Browser WASM:** single-threaded executor; `lotus-search` yields cooperatively to the browser event loop via `MessageChannel` (`yield_to_browser`) instead of `setTimeout(0)` to avoid Firefox background-tab throttling.
   - **Native server & CLI:** `tokio` multi-threaded runtime; server concurrency is bounded by explicit `tokio::sync::Semaphore` permits (separating client admission `max_concurrency` from polite upstream `upstream_concurrency`) and single-flight coalescing cells.
6. **Cargo hygiene:**
   `[workspace.dependencies]` and `[workspace.lints]` are the single sources of truth for versions, features (`default-features = false` where HTTP/runtime is optional), MSRV (`rust-version`), and strict clippy/rustdoc rules.

---

## End-to-End Request Lifecycle

When tracing a search result, count discrepancy, or export bug, follow the four stages below:

```text
1. Input resolution          taxon name/QID ──► TaxonResolution (QID + nomenclatural roots)
   (lotus-search / app)      structure box  ──► ResolvedStructure (compound QIDs + SMILES)
                             reference box  ──► reference QID (from DOI or QID)
                                      │
2. Base SPARQL construction           ▼
   (lotus-query::query)      One of 6 base 15-column SELECT shapes + with_filters(...)
                                      │
                    ┌─────────────────┴─────────────────┐
                    ▼                                   ▼
3a. CLI / API / WDQS fallback             3b. Browser & search_columnar split-stream
    limit_query(&base, limit)                 occurrences_query(&base)  [4 cols: c, t, taxon_name, r]
    + counts_query(&base)                     + dictionary_queries(...) [compound & ref metadata]
                    │                                   │
                    └─────────────────┬─────────────────┘
                                      ▼
4. Columnar store & views    CsvColumnarReader ──► ColumnarResultSet (lotus-model::columnar)
                                      ├──► Client filter/sort/virtual table (lotus-explore-core + app)
                                      └──► RowExporter CSV / SPARQL JSON / RDF Turtle (lotus-query::export_rows)
```

### Stage 1: Input resolution (turning user text into Wikidata QIDs)

- **Taxon (`crates/lotus-query/src/query.rs` + `crates/lotus-search/src/search.rs`):**
  - Empty string `""` means *all compounds, including untaxonomised ones*.
  - Wildcard `"*"` means *all compounds with at least one taxon occurrence*.
  - A bare QID (`Q16521`) is used without a network lookup.
  - Otherwise `taxon_lookup_query` checks scientific names (`wdt:P225` exact index match), falling back to `taxon_common_name_lookup_query` (`p:P1843/ps:P1843` scan). See [`TAXON-SEARCH.md`](TAXON-SEARCH.md).
- **Structure / Compound (`crates/lotus-query/src/query.rs` + `crates/lotus-search/src/search.rs`):**
  - Dispatched by shape (`lotus-model`):
    1. QID (`Q18216`) -> `compound_by_qid_query` (web app) or direct QID seed (CLI).
    2. InChIKey (`BSYNRYMUTXBXSQ-UHFFFAOYSA-N`) -> `compound_inchikey_query` (`wdt:P235`).
    3. Plausible chemical name (`aspirin`) -> `compound_label_query` (`rdfs:label` across 16 language tags and deduplicated casing variants, guarded by `wdt:P235` and `SELECT DISTINCT`), then `compound_alias_query` (`skos:altLabel`).
    4. SMILES or V2000/V3000 molfile -> `structure_compound_lookup_query` (IDSM Sachem similarity at cutoff `1.0`).
  - See [`STRUCTURE-SEARCH.md`](STRUCTURE-SEARCH.md).
- **Reference (`reference_by_doi_query` / `reference_by_qid_query`):**
  - Uppercases DOIs (`wdt:P356`) and resolves them to a reference QID (`?r`).

### Stage 2: Base SPARQL construction (`crates/lotus-query/src/query.rs`)

Both `lotus_search::build_execution_query_with` and `lotus_explore_core::explore::service::build_query::build_sparql_query` produce one of **six base 15-column `SELECT` queries**:

| Base shape | Builder in `lotus-query` | Inner helper | Occurrence (`p:P703`) |
| --- | --- | --- | --- |
| Empty taxon (`""`), no structure | `all_compounds_including_untaxonomised_query` | `optional_compounds_query_body` | `OPTIONAL` |
| Wildcard taxon (`"*"`), no structure | `all_compounds_query` | `all_required_compounds_query_body` | Required |
| Named taxon (`Q16521`), no structure | `compounds_by_taxon_query_with` | `taxon_compounds_query_body` | Required |
| Exact compound seed (`Q18216`), optional taxon | `exact_compounds_query` | `seeded_compounds_core` | `OPTIONAL` without taxon; Required with taxon |
| Structure search (Substructure / Similarity), no taxon | `structure_search_query_with(..., None, ...)` | `idsm_service_clause` | `OPTIONAL` |
| Structure search (Substructure / Similarity), with taxon | `structure_search_query_with(..., Some(qid), ...)` | `idsm_service_clause` | Required |

Every base query embeds `COMPOUND_PROPERTIES` and `REFERENCE_METADATA` near the outer `WHERE`, and `with_filters(&base, &criteria, year_max)` splices active server-side constraints before the closing `}`. See [`QUERY_MATRIX.md`](QUERY_MATRIX.md).

### Stage 3: Query rewrites and the two execution paths

`lotus-query` provides four pure string rewriters on top of any base query:
1. **`occurrences_query(&base)`** — rewrites header to 4 columns (`compound`, `taxon`, `taxon_name`, `ref_qid`) and strips display metadata constants.
2. **`dictionary_queries(compound_qids, reference_qids)`** — builds `entity_dictionary_query`, `compound_dictionary_query`, and `reference_dictionary_query`.
3. **`counts_query(&base)`** — strips display metadata and wraps in `COUNT(*)` / `COUNT(DISTINCT ...)`.
4. **`construct_from_select(&base)` and `wdqs_fallback(&base)`** — builds `CONSTRUCT` graphs for RDF exports or moves `REFERENCE_METADATA` into the scholarly subgraph `SERVICE` clause for WDQS v2 fallback.

> **Important invariant:** Because `counts_query` and `occurrences_query` strip `COMPOUND_PROPERTIES` and `REFERENCE_METADATA` by literal substring match, any variable used by a `FILTER` in `with_filters` must be bound outside those two constants (enforced by contract tests in `lotus-query`).

### Stage 4: Streaming fold into `ColumnarResultSet`, table views, and exports

- **`CsvColumnarReader` (`crates/lotus-query/src/parse/stream.rs`)** ingests either a 4-column occurrence stream + dictionary streams (`search_columnar` / browser path) or a 15-column CSV stream (WDQS fallback / fixtures).
- **`ColumnarResultSet` (`crates/lotus-model/src/columnar.rs`)** stores deduplicated per-entity arenas indexed by numeric QIDs plus a flat edge array.
- **Client-side filtering, sorting, and graph projection (`crates/lotus-explore-core`)** operate directly on `ColumnarResultSet` indices in memory.
- **Downloads (`crates/lotus-query/src/export_rows.rs`)** stream `ColumnarResultSet` rows through `RowExporter` to CSV, SPARQL JSON, or RDF/Turtle.

---

## Testing

- Pure crates (`lotus-model`, `lotus-query`, `lotus-jsonld`) run offline with no runtime: string/fixture assertions and deterministic property tests.
- `lotus-search` and `lotus-curation` are tested offline through the `Http` trait using `lotus_search::testing::Scripted`.
- `lotus-cli` integration tests assert on `--explain` and verify `docs/cli.md` parity with `--help`.
- Server HTTP routes are tested in-process via `tower::ServiceExt` without binding live sockets.

## See also

- [`apps/lotus-explore-rs/docs/ARCHITECTURE.md`](../apps/lotus-explore-rs/docs/ARCHITECTURE.md) — web client internals and clock placement
- [`cli.md`](cli.md) — CLI reference
- [`TAXON-SEARCH.md`](TAXON-SEARCH.md) — taxon resolution and nomenclatural relationships
- [`STRUCTURE-SEARCH.md`](STRUCTURE-SEARCH.md) — compound identity and structure resolution
- [`QUERY_MATRIX.md`](QUERY_MATRIX.md) — argument combinations and generated queries
- [`SPARQL-VARIANTS.md`](SPARQL-VARIANTS.md) — measured SPARQL variants
