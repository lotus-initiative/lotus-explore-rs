# Architecture

The repository is a workspace of nine crates: seven library/tooling crates, one
CLI binary (`lotus-cli`), and one web/desktop/server application
(`apps/lotus-explore-rs`). The arrangement is not a preference: it is what keeps
the same query from being written three times, and it is checkable in CI.

## The nine crates

| Crate                                                          | Layer        | Contains                                                                                         | Workspace dependencies                       |
| -------------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------------------------ | -------------------------------------------- |
| [`lotus-model`](../crates/lotus-model/README.md)               | Pure         | `SearchCriteria`, `ColumnarResultSet`, `LotusEntry`, structure/identifier classifiers, validators | *(none)*                                     |
| [`lotus-query`](../crates/lotus-query/README.md)               | Pure         | SPARQL builders, query rewriters (`counts_query`, `occurrences_query`), CSV/JSON/RDF parsers/I/O | `lotus-model`                                |
| [`lotus-curation`](../crates/lotus-curation/README.md)         | Use-case     | Curation vocabulary, TSV reader, Wikidata lookup SPARQL, QuickStatements V1 generator            | `lotus-search`                               |
| [`lotus-jsonld`](../crates/lotus-jsonld/README.md)             | Pure         | Bioschemas JSON-LD, CodeMeta (`codemeta.json`), `CITATION.cff`                                   | `lotus-model`                                |
| [`lotus-search`](../crates/lotus-search/README.md)             | Use-case     | `Http` trait, taxon/compound/DOI resolution, retry, QLever→WDQS fallback, split-stream fold      | `lotus-model`, `lotus-query`                 |
| [`lotus-explore-core`](../crates/lotus-explore-core/README.md) | App core     | Framework-free `ExploreState` reducer, `CriteriaQueryDto` URL codec, client filter/sort, exports | `lotus-model`, `lotus-query`, `lotus-search` |
| `lotus-web-assets`                                             | Build tool   | Vendors Ketcher, RDKit, and Citation.js into `public/assets/vendor`; injects WASM preload links  | *(none)*                                     |
| [`lotus-cli`](../crates/lotus-cli/README.md)                   | Front end    | The `lotus` binary (`search`, `curate`, `export`, `export-pubchem-tree`, `archive`)              | all shared crates above                      |
| [`apps/lotus-explore-rs`](../apps/lotus-explore-rs/README.md)  | Front end    | Dioxus web client (WASM), optional Axum HTTP API (`--features server`), optional desktop window  | all shared crates above                      |

The arrows all point inward. `lotus-model` depends on no other crate in the
workspace, which is why it holds the foundational vocabulary that everything
else agrees on.

## Three rules, and why they are checkable

1. **A crate that can be pure is pure (`./mk check-wasm-pure`).**
   `lotus-model`, `lotus-query`, `lotus-curation`, and `lotus-jsonld` have no
   HTTP, no clock, and no async runtime. CI compiles them for
   `wasm32-unknown-unknown` in isolation. Because pure crates cannot read a
   clock, `SearchCriteria` has no `Default` — callers construct
   `SearchCriteria::up_to_year(year)` with the year supplied by the host clock.
2. **A transformation is separate from a network decision.**
   `lotus-query` turns criteria into SPARQL strings and parses CSV bytes into
   model types (pure, tested by exact string and fixture comparison).
   `lotus-search` decides which endpoint to call, when to retry or fall back
   from QLever to WDQS, and how to resolve user inputs over the two-method
   `Http` trait (tested offline against [`Scripted`](../crates/lotus-search/src/testing.rs)).
3. **Application state and domain logic contain no UI framework (`./mk boundary`).**
   `lotus-explore-core` holds the state machine (`ExploreState`), reducers, URL
   codec (`CriteriaQueryDto`), client-side table filtering/sorting, and bulk
   export pipelines. `./mk boundary` fails if `dioxus` appears anywhere in
   `lotus-explore-core`.

---

## End-to-end request lifecycle: how a search actually runs

When debugging a search result, a missing column, a count discrepancy, or an
export bug, trace the request through these four stages:

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

Before the main result query is built, human-typed strings are resolved to QIDs:

- **Taxon (`crates/lotus-query/src/query.rs` + `crates/lotus-search/src/search.rs` / `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/`):**
  - Empty string `""` means *all compounds, including untaxonomised ones*.
  - Wildcard `"*"` means *all compounds with at least one taxon occurrence*.
  - A bare QID (`Q16521`) is used without a network lookup.
  - Otherwise `taxon_lookup_query` checks scientific names (`wdt:P225` exact
    index match), falling back to `taxon_common_name_lookup_query` (`p:P1843/ps:P1843`
    case-insensitive scan). See [`docs/TAXON-SEARCH.md`](TAXON-SEARCH.md).
- **Structure / Compound (`crates/lotus-query/src/query.rs` + `crates/lotus-search/src/search.rs` / `apps/lotus-explore-rs/src/features/explore/service/resolve_structure/`):**
  - Dispatched by shape (`lotus-model::structure` / `lotus-model::identify`):
    1. QID (`Q18216`) -> `compound_by_qid_query` (in web app) or direct QID seed (in CLI).
    2. InChIKey (`BSYNRYMUTXBXSQ-UHFFFAOYSA-N`) -> `compound_inchikey_query` (`wdt:P235`).
    3. Plausible chemical name (`aspirin`) -> `compound_label_query` (`rdfs:label` across 16 language tags), then `compound_alias_query` (`skos:altLabel`).
    4. SMILES or V2000/V3000 molfile -> `structure_compound_lookup_query` (IDSM Sachem similarity at cutoff `1.0`).
  - See [`docs/STRUCTURE-SEARCH.md`](STRUCTURE-SEARCH.md).
- **Reference (`reference_by_doi_query` / `reference_by_qid_query`):**
  - Uppercases DOIs (`wdt:P356`) and resolves them to a reference QID (`?r`).

### Stage 2: Base SPARQL construction (`crates/lotus-query/src/query.rs`)

Both `lotus_search::build_execution_query_with` (used by the CLI and server) and
`lotus_explore_core::explore::service::build_query::build_sparql_query` (used by
the web/desktop app) call the same builders in `crates/lotus-query/src/query.rs`
to produce one of **six base 15-column `SELECT` queries**:

| Base shape | Builder in `crates/lotus-query/src/query.rs` | Inner helper | Occurrence (`p:P703`) |
|---|---|---|---|
| Empty taxon (`""`), no structure | `all_compounds_including_untaxonomised_query` | `optional_compounds_query_body` | `OPTIONAL` |
| Wildcard taxon (`"*"`), no structure | `all_compounds_query` | `all_required_compounds_query_body` | Required |
| Named taxon (`Q16521`), no structure | `compounds_by_taxon_query_with` | `taxon_compounds_query_body` | Required |
| Exact compound seed (`Q18216`), optional taxon | `exact_compounds_query` | `seeded_compounds_core` | `OPTIONAL` without taxon; Required with taxon |
| Structure search (Substructure / Similarity), no taxon | `structure_search_query_with(..., None, ...)` | `idsm_service_clause` | `OPTIONAL` |
| Structure search (Substructure / Similarity), with taxon | `structure_search_query_with(..., Some(qid), ...)` | `idsm_service_clause` | Required |

Every base query embeds two named display-metadata constants near the outer `WHERE`:
- `COMPOUND_PROPERTIES` — `OPTIONAL` bindings for `@en`/`@mul` label, `wdt:P233` (canonical SMILES), `wdt:P2017` (isomeric SMILES), `wdt:P2067` (mass), and `wdt:P274` (formula).
- `REFERENCE_METADATA` — sentinel-guarded (`BIND(COALESCE(?r, wd:Q0) AS ?_ref_source)`) `OPTIONAL` bindings for `wdt:P1476` (title), `wdt:P356` (DOI), and `wdt:P577` (publication date).

Finally, `with_filters(&base, &criteria, year_max)` splices active server-side
constraints (reference `VALUES ?r`, mass range, publication year range, and
chemical formula regex/element range filters) before the closing `}`.
See [`docs/QUERY_MATRIX.md`](QUERY_MATRIX.md).

### Stage 3: Query rewrites and the two execution paths

A base 15-column `SELECT` query is never sent raw when a faster specialized
rewrite applies. `crates/lotus-query/src/query.rs` provides four pure string
rewriters on top of any base query:

1. **`occurrences_query(&base)` (Split-stream Phase 1):**
   Replaces the 15-column `SELECT` header with `OCCURRENCES_SELECT` (4 integer/string columns: `compound`, `taxon`, `taxon_name`, `ref_qid`) and strips `COMPOUND_PROPERTIES` and `REFERENCE_METADATA` by literal string match (re-binding `?r wdt:P577 ?ref_date .` if a year filter is active).
2. **`dictionary_queries(compound_qids, reference_qids)` (Split-stream Phase 2):**
   Builds `entity_dictionary_query`, `compound_dictionary_query`, and `reference_dictionary_query` to fetch labels, SMILES, mass, formula, titles, DOIs, and years once per distinct compound/reference QID instead of repeating them across millions of occurrence edges.
3. **`counts_query(&base)` (CLI / API totals):**
   Strips `COMPOUND_PROPERTIES` and `REFERENCE_METADATA` (re-binding `?ref_date` when year-filtered) and wraps the query in `COUNT(*)` / `COUNT(DISTINCT ...)`.
4. **`construct_from_select(&base)` and `wdqs_fallback(&base)`:**
   Rewrites a `SELECT` into a `CONSTRUCT` graph for RDF exports, or moves `REFERENCE_METADATA` into `SERVICE <https://query-scholarly.wikidata.org/sparql>` when falling back from QLever to WDQS.

> **Important invariant:** Because `counts_query` and `occurrences_query` strip
> `COMPOUND_PROPERTIES` and `REFERENCE_METADATA` by literal substring match, any
> variable used by a `FILTER` in `with_filters` must be bound outside those two
> constants (enforced by `crates/lotus-query/tests/contract/filters.rs`), and
> display-only properties (`P233`, `P2017`, labels, titles, DOIs) must live
> inside those constants so counts and occurrence streams never pay for them
> (enforced by `crates/lotus-query/tests/contract/structure_rules.rs`).

### Stage 4: Streaming fold into `ColumnarResultSet`, table views, and exports

- **`CsvColumnarReader` (`crates/lotus-query/src/parse/stream.rs`)** accepts either a 4-column occurrence stream followed by dictionary enrichment streams (`search_columnar` / browser path) or a single 15-column CSV stream (WDQS fallback / test fixtures).
- **`ColumnarResultSet` (`crates/lotus-model/src/columnar.rs`)** stores the result set in compact, deduplicated per-entity arenas indexed by numeric QIDs (`compound_qids`, `taxon_qids`, `reference_qids`) plus a flat array of occurrence edges.
- **Client-side filtering & sorting (`crates/lotus-explore-core/src/{filters,sort,table_budget}.rs`)** operate directly on `ColumnarResultSet` row indices in memory without re-querying the endpoint.
- **Downloads (`crates/lotus-query/src/export_rows.rs`)** stream `ColumnarResultSet` rows through `RowExporter` to produce CSV, SPARQL JSON, or RDF/Turtle (`format=csv|json|ttl`).

---

## Module map of the major crates

When looking for a specific piece of logic, use this map of the largest crates:

```text
crates/
├── lotus-model/src/
│   ├── criteria.rs      SearchCriteria, SmilesSearchType, ElementConstraint
│   ├── validate.rs      validate_criteria (mass/year/element range checks)
│   ├── structure.rs     classify_structure, could_be_a_compound_name, names_a_compound
│   ├── identify.rs      looks_like_inchikey, looks_like_a_compound_qid, strip_doi_prefix
│   ├── columnar.rs      ColumnarResultSet & ColumnarResultSetBuilder (interned columnar store)
│   ├── entry.rs         LotusEntry (materialised single row view)
│   └── stats.rs         DatasetStats (compounds, taxa, references, entries counts)
│
├── lotus-query/src/
│   ├── query.rs         All search/lookup/dictionary SPARQL builders + counts/occurrences/CONSTRUCT/WDQS rewriters
│   ├── bulk_export.rs   Full-graph SPARQL queries for `lotus export`, `export-pubchem-tree`, and `archive`
│   ├── parse.rs         One-shot CSV parsers (taxon, compound lookup, reference lookup, counts, capped rows)
│   ├── parse/stream.rs  CsvColumnarReader (chunked streaming CSV fold + dictionary enrichment)
│   ├── export.rs        ExportFormat (Csv, Json, Rdf/Turtle, Sparql)
│   └── export_rows.rs   RowExporter (streams ColumnarResultSet / LotusEntry into CSV, JSON, or Turtle)
│
├── lotus-search/src/
│   ├── client.rs        Http, HttpResponse, BodyChunks traits (2-method transport abstraction)
│   ├── execute.rs       execute / execute_streaming + retry + QLever→WDQS fallback
│   ├── search.rs        resolve_taxon, resolve_structure, resolve_reference, search, search_columnar
│   ├── reqwest_client/  Native + WASM reqwest implementations of `Http` (with WASM paint yielding)
│   └── testing.rs       Scripted offline transport for deterministic tests
│
├── lotus-explore-core/src/
│   ├── explore/
│   │   ├── search_state/    ExploreState + pure reducers (lifecycle, result_data, ui)
│   │   ├── actions.rs       ExploreAction enum (every state transition)
│   │   ├── service/         build_query.rs (ResolvedStructure -> SPARQL), strategy.rs, finalize.rs
│   │   ├── url_codec/       CriteriaQueryDto (URL query string <-> SearchCriteria round-trip)
│   │   └── form_validation/ Form validation dispatch and error types
│   ├── filters.rs       Per-column client-side filtering over ColumnarResultSet
│   ├── sort.rs          Multi-column client-side sorting over ColumnarResultSet
│   ├── table_budget.rs  Row rendering budget calculation
│   ├── api/             REST API DTOs and error types shared by client and server
│   └── export/          Bulk pipelines: pipeline.rs (`lotus export`), pubchem_tree.rs, archive.rs
│
└── lotus-curation/src/
    ├── constants.rs      Wikidata property & entity constants
    ├── types.rs          CurationInputRow, ConvertedStructure, CurationResultRow, CurationStatus
    ├── input.rs          TSV curation file parser and row deduplication key
    ├── structure.rs      CDK Depict / Cactus SMILES->InChIKey/InChI/formula/mass conversion
    ├── wikidata_query.rs SPARQL lookups for existing compounds, taxa, references, occurrences
    ├── knowledge.rs      Reconciliation logic (matching local rows against Wikidata items)
    └── quickstatements.rs QuickStatements V1 batch generation
```

For the internal layout of `apps/lotus-explore-rs` (components, orchestrator,
executor, caches, and Dioxus hooks), see
[`apps/lotus-explore-rs/docs/ARCHITECTURE.md`](../apps/lotus-explore-rs/docs/ARCHITECTURE.md).

## Where things used to be wrong

The repository once had a `lotus` crate holding the models, the queries, the
parsers and the HTTP client, and the web app had a second copy of several of
those, including two implementations of "what a taxon filter means" that
disagreed. The refactor that produced the layout above deleted `crates/lotus`
and moved the app onto these crates, which is the only reason the arrows point
inward now. [`FRONTENDS.md`](FRONTENDS.md) has the later instance of the same
failure and why a duplicated dispatch is invisible to tests.

Two things came out of that refactor and look odd without knowing it:
`SearchCriteria` has no `Default`, because "no filters" is a value _plus a year_
and the old `default()` supplied the year by reading a clock; and
`has_year_filter`, `has_effective_filters` and `with_filters` take the year as an
argument, because the application has the clock.

## Testing

- The pure crates are tested with no network and no runtime: string comparisons
  for query builders, recorded fixtures for the parsers.
- `lotus-search` is tested through the `Http` trait, including the fallback path
  and the retry policy, which is otherwise very hard to provoke.
- `lotus-cli` runs its integration tests offline by asserting on `--explain`,
  which prints the SPARQL without sending it.
- `docs/cli.md` is checked against `--help` in both directions, and every
  documented example is run with `--explain` appended.

The full task list is `./mk --list-all-steps`; it is not copied here so that it
cannot go stale. [`CONTRIBUTING.md`](../CONTRIBUTING.md) covers the test
conventions and what to run.

## See also

- [`ARCHITECTURE.md`](../apps/lotus-explore-rs/docs/ARCHITECTURE.md) --- the
  application side, and why the clock lives there
- [`cli.md`](cli.md) --- the command line
- [`TAXON-SEARCH.md`](TAXON-SEARCH.md) --- how a taxon name becomes a QID, and the
  four nomenclatural relationships
- [`STRUCTURE-SEARCH.md`](STRUCTURE-SEARCH.md) --- how a name, an InChIKey or a QID
  becomes a compound identity, and when the structure service is called at all
- [`QUERY_MATRIX.md`](QUERY_MATRIX.md) --- the twelve argument combinations and the
  query each one builds
- [`SPARQL-VARIANTS.md`](SPARQL-VARIANTS.md) --- query forms measured against the
  live endpoint, and which were kept

## How to port to framework X

The app domain is split into `crates/lotus-explore-core` (framework-free) and `apps/lotus-explore-rs` (Dioxus adapter).

### Core (no Dioxus)
- State: `lotus_explore_core::explore::search_state/*` — plain structs and pure `reduce`/reducer functions
- Actions/outcomes/types: `lotus_explore_core::explore::{actions,outcome,types,request,retry,...}`
- URL/state encoding: `lotus_explore_core::explore::url_codec/*`
- Domain services/logic: `lotus_explore_core::explore::service/*`, `form_validation`, `form_actions`, `search_utils`, etc.
- Shared domain: `lotus_explore_core::{api,dto,error,export,filters,sort,table_budget,ports}`
- Clock/time: pass `year: u16`/ISO strings as parameters (no `now()` in core)

### Adapter (Dioxus-specific)
- Components/RSX: `apps/lotus-explore-rs/src/components/*`, pages, layout
- State wiring: Dioxus `Signal`, hooks, effects, context providers
- Routing/navigation: Dioxus router
- Server functions/HTTP: `apps/lotus-explore-rs/src/repositories/*`, `services/*`, server-only code
- Platform integration: browser APIs, assets, download handling
- Orchestrator/dispatch/executor: `apps/lotus-explore-rs/src/features/explore/{orchestrator,executor,interactions,...}` bridge core actions to side effects

### Port checklist
1. Replace adapter with target framework (components/state/routing)
2. Implement ports/traits from `lotus_explore_core::ports` against target runtime
3. Thread `year`/time from target clock into core calls
4. Translate core actions to framework state updates via pure reducers where possible
5. Reimplement side effects (HTTP, storage, downloads) in new adapter
6. Keep core unchanged; enforce boundary with `./mk boundary`

## Export, archive, and PubChem tree — where the bulk tools live

Three data-movement tools share the same home in
`crates/lotus-explore-core/src/export/`:

| Module | What it does | Entry points |
|--------|-------------|--------------|
| `pipeline.rs` | The full LOTUS compound–taxon–reference graph as `frozen.csv` + `frozen_metadata.csv` (ports `lotus_exporter.py`) | `lotus export` (CLI), API export endpoint (server) |
| `pubchem_tree.rs` | Three PubChem-format JSON trees: biological, chemical, NPClassifier — each full or compact (ports `lotus_pubchem_tree.py`) | `lotus export-pubchem-tree` (CLI) |
| `archive.rs` | All InChIKey-to-SMILES mappings, deduplicated and tar+gzip packed (ports `lotus-wikidata-archive`) | `lotus archive` (CLI) |

All three are target-agnostic: they take an `Http` trait implementor and an
`Endpoint`, return bytes/strings, and do all file I/O themselves. The CLI
(`crates/lotus-cli/src/main.rs`) is the thin native-only wrapper that writes
those bytes to disk and, for `archive`, optionally publishes to Zenodo behind
the `zenodo-rs` dependency. The web app never calls the archive or pubchem-tree
paths because those fetch the entire dataset — the browser's export buttons use
the per-query `pipeline` path instead.

`lotus-query` owns the SPARQL query strings and `ARCHIVE_PAGE_SIZE`.
`lotus-model` owns the `SearchCriteria` argument matrix that `pipeline` reuses.
`lotus-search` owns the `Http` trait, `Scripted` test double, and QLever/WDQS
endpoint resolution. See [`docs/SHARING_PLAN.md`](SHARING_PLAN.md) for the full plan.
