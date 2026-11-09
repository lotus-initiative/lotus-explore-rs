# lotus-explore-rs Application Architecture

The workspace layout — what each of the 9 crates is for, which of them are pure,
and how a search flows end to end — is in
[`docs/ARCHITECTURE.md`](../../../docs/ARCHITECTURE.md). This page covers what
the `apps/lotus-explore-rs` crate adds on top of the shared libraries.

## Division of responsibility: `lotus-explore-core` vs `apps/lotus-explore-rs`

To keep business logic testable without a browser or Dioxus runtime, the
application is split across two crates:

- **[`crates/lotus-explore-core`](../../../crates/lotus-explore-core/README.md)**
  owns the framework-free state machines (`ExploreState`, `ExploreAction`,
  `ExploreReducer`, `CurationState`), URL query parameter encoding/decoding
  (`CriteriaQueryDto`), client-side columnar filtering (`ResultFilters`) and
  sorting (`ResultSortKey`), table memory budgets (`table_budget.rs`), query
  dispatch (`sparql.rs`), and the bulk export / PubChem tree / archive pipelines.
- **[`apps/lotus-explore-rs`](../README.md)** owns the Dioxus UI components,
  reactive hooks, browser/desktop effects (clipboard, `localStorage`, file
  downloads, URL history, `navigator.deviceMemory`), the `reqwest` HTTP client
  ([`src/sparql.rs`](../src/sparql.rs)), the wall clock
  ([`src/clock.rs`](../src/clock.rs)), and the optional Axum server
  ([`src/server/`](../src/server/)).

## Directory layout of `apps/lotus-explore-rs/src/`

```text
src/
  app/                  Application bootstrap, shell, routes, and root view
    explore/            Explore page shell, URL sync, and top-level handlers
  components/           Rendering-only Dioxus UI components
    results_table/      Virtualized results table, column headers, and 2D SVG/canvas depictions
    structure_editor/   Ketcher / RDKit structure editor modal and bridges
    ...
  features/
    explore/            Search orchestration, streaming pipeline, downloads, and error recovery
      orchestrator/     ExploreController, reactive signals, and action dispatch
      executor.rs       Top-level search execution lifecycle and cancellation token handling
      service/
        results_pipeline/  Execution plan builder (Full vs SplitStream) and row assembly
        fetch_results/     Platform-specific query execution (wasm.rs split-stream vs native.rs)
        resolve_taxon/     Taxon name / QID resolution and synonym expansion
        resolve_structure/ Compound name / InChIKey / SMILES / QID resolution
        resolve_reference/ Reference DOI / PubMed / title resolution
        api_pipeline/      REST API search / export execution
      download_dispatch.rs Formats and dispatches CSV/TSV/JSON/RDF downloads
      download_effects.rs  Browser blob download and native file write effects
    curation/           Interactive curation table, QuickStatements preview, and TSV import/export
  hooks/                Dioxus reactive hooks (debounced inputs, theme, viewport)
  services/             Cross-feature browser/host services (clipboard, storage, URL)
  state/                Dioxus signal wrappers around lotus-explore-core state reducers
  repositories/         Data access layer mapping SPARQL JSON / TSV streams into domain rows
  server/               Optional Axum HTTP server (`--features server`), REST API, and SSR
  clock.rs              The one place in the app that reads a wall clock
  sparql.rs             The app's HTTP layer: the only place that names reqwest
```

There are no re-export shims. A call site that needs a domain type writes
`lotus_model::`, one that needs a query builder writes `lotus_query::`, and one
that needs a state reducer or filter writes `lotus_explore_core::`, so the crate
a symbol comes from is visible where it is used.

`sparql.rs` is the layer that turns "run this query on that endpoint" into an
HTTP request (including streaming TSV responses in the browser via the Fetch
ReadableStream API), and it is the only place in the app that names `reqwest`.

## How clicking "Search" works in the web app

When a user submits the search form or loads a shared URL:

1. **URL & form sync (`app/explore/` & `CriteriaQueryDto`):**
   Query parameters are parsed via `lotus_explore_core::api::dto::CriteriaQueryDto`
   into `lotus_model::SearchCriteria`.
2. **Controller dispatch ([`features/explore/orchestrator/controller.rs`](../src/features/explore/orchestrator/controller.rs)):**
   `ExploreController` increments the search generation token (so any in-flight
   previous search is ignored on completion), transitions `ExploreState` to
   loading via `ExploreReducer`, and spawns `executor::execute_search`.
3. **Input resolution ([`features/explore/executor.rs`](../src/features/explore/executor.rs)):**
   Before building the main occurrence query, `executor.rs` resolves free-text
   inputs in parallel where needed:
   - [`service/resolve_taxon/`](../src/features/explore/service/resolve_taxon/) resolves taxon names to Wikidata QIDs.
   - [`service/resolve_structure/`](../src/features/explore/service/resolve_structure/) resolves compound names, InChIKeys, or QIDs (fetching canonical/isomeric SMILES and labels) or validates chemical structures for cartridge search.
   - [`service/resolve_reference/`](../src/features/explore/service/resolve_reference/) resolves DOIs, PubMed IDs, or title substrings.
4. **Execution plan ([`features/explore/service/results_pipeline/plan.rs`](../src/features/explore/service/results_pipeline/plan.rs)):**
   `build_execution_plan` calls `lotus_explore_core::sparql::build_sparql_query`
   to build the base 15-column SPARQL `SELECT` and chooses one of two strategies:
   - **`SplitStream` (default on wasm when no text-only post-filters apply):**
     rewrites the base query via `lotus_query::occurrences_query` into a slim
     3-column `(?compound ?taxon ?reference)` query, streams its TSV response
     line-by-line in [`service/fetch_results/wasm.rs`](../src/features/explore/service/fetch_results/wasm.rs)
     to update unique compound/taxon/reference counts progressively, and then
     runs `lotus_query::dictionary_queries` in concurrent batches to hydrate the
     15 columns of the visible table.
   - **`Full` (used when text filters require labels or on native targets):**
     runs `lotus_query::limit_query` for the table rows and
     `lotus_query::counts_query` in parallel for the exact total counts.
5. **Columnar state commit & rendering ([`components/results_table.rs`](../src/components/results_table.rs)):**
   Fetched rows are interned into a `lotus_model::ColumnarResultSet`. Client-side
   per-column filtering (`lotus_explore_core::filters::TableFilters`) and
   sorting (`lotus_explore_core::sort::SortColumn`) compute a `Vec<u32>` view
   index over the columnar store without cloning row strings, and the virtualized
   table ([`components/results_table/virtualized_table.rs`](../src/components/results_table/virtualized_table.rs))
   renders only the visible slice.

## Rules

- Components render and dispatch — no business logic.
- Pure state transitions, URL contracts, and client-side filtering/sorting belong
  in `lotus-explore-core`, not inside Dioxus components.
- Feature internals are private; each feature exposes a typed facade via
  `mod.rs`.
- API DTOs stop at repository boundaries and never reach components.
- State subscriptions are narrow — components read only the slices they use.
- Every async path carries a generation token; stale completions are discarded
  before state commit.
- Typed errors (`thiserror`) at all boundaries; user messages are derived
  separately.
- Both halves of the URL serialisation contract live together in
  `CriteriaQueryDto`.
