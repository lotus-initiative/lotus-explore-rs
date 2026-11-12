# lotus-explore-rs Application Architecture

The workspace layout — what each of the 9 crates is for, which of them are pure,
and how a search flows end to end — is in
[`docs/ARCHITECTURE.md`](../../../docs/ARCHITECTURE.md). This page covers what
the `apps/lotus-explore-rs` crate adds on top of the shared libraries.

## Division of responsibility: `lotus-explore-core` vs `apps/lotus-explore-rs`

To keep business logic testable without a browser or Dioxus runtime, the
application is split across two crates:

- **[`crates/lotus-explore-core`](../../../crates/lotus-explore-core/README.md)**
  owns the framework-free Explore state machine (`ExploreState`, `ExploreAction`,
  `reduce_mut`), URL query parameter encoding/decoding (`CriteriaQueryDto`),
  client-side columnar filtering (`ColumnFilters`) and sorting (`SortColumn`,
  `SortDir`, `SortState`), table memory budgets (`table_budget.rs`), query
  construction (`explore::service::build_query::build_sparql_query`), REST API
  DTOs (`api/`), and the bulk export / PubChem tree / archive pipelines.
- **[`apps/lotus-explore-rs`](../README.md)** owns the Dioxus UI components and
  pages, curation UI state (`src/features/curation/state/`), reactive hooks,
  browser/desktop effects (clipboard, `localStorage`, file downloads, URL
  history, `navigator.deviceMemory`), the `reqwest` HTTP client
  ([`src/sparql.rs`](../src/sparql.rs)), the wall clock
  ([`src/clock.rs`](../src/clock.rs)), and the optional Axum server
  ([`src/server/`](../src/server/)).

## Directory layout of `apps/lotus-explore-rs/src/`

```text
src/
  app/                  Application bootstrap, shell, and routes
  pages/                Top-level views, including Ketcher structure editor (`draw_page.rs`, `ketcher_panel.rs`)
  components/           Rendering-only Dioxus UI components
    results_table/      Virtualized results table, column headers, and 2D SVG/canvas depictions
    ...
  features/
    explore/            Search orchestration, streaming pipeline, downloads, and error recovery
      orchestrator/     SearchTaskController (`controller.rs`), `start_search` (`runtime.rs`), and dispatch
      executor.rs       `SearchExecutor` and `do_search` entry point
      url_state.rs      Browser URL query string sync via `CriteriaQueryDto`
      service/
        results_pipeline/  Execution plan builder (`plan.rs`) and split-stream / full orchestration
        fetch_results/     Platform-specific CSV query execution (`wasm.rs` split-stream vs `native.rs`)
        resolve_taxon/     Taxon name / QID resolution and common-name fallback
        resolve_structure/ Compound name / InChIKey / SMILES / QID resolution
        resolve_reference.rs Reference Wikidata QID and DOI resolution
        api_pipeline.rs    Optional REST API fast-path execution
      download_dispatch.rs Formats and dispatches CSV/JSON/RDF/SPARQL downloads
      download_effects/    Browser blob download and native file write effects
    curation/           Interactive curation table, batch Wikidata prefetch, and QuickStatements preview
  hooks/                Dioxus reactive hooks (debounced inputs, theme, viewport)
  services/             Cross-feature browser/host services (clipboard, storage, error formatting)
  state/                Dioxus signal wrappers around application state
  repositories/         Data access layer streaming SPARQL CSV / JSON responses
  server/               Optional Axum HTTP server (`--features server`), REST API, and static hosting
  clock.rs              The one place in the app that reads a wall clock
  sparql.rs             The app's HTTP layer: the only place that names reqwest
```

There are no re-export shims. A call site that needs a domain type writes
`lotus_model::`, one that needs a query builder writes `lotus_query::`, and one
that needs a state reducer or filter writes `lotus_explore_core::`, so the crate
a symbol comes from is visible where it is used.

`sparql.rs` is the layer that turns "run this query on that endpoint" into an
HTTP request (including streaming CSV responses in the browser via the Fetch
ReadableStream API), and it is the only place in the app that names `reqwest`.

## How clicking "Search" works in the web app

When a user submits the search form or loads a shared URL:

1. **URL & form sync ([`features/explore/url_state.rs`](../src/features/explore/url_state.rs) & `CriteriaQueryDto`):**
   Query parameters are parsed via
   `lotus_explore_core::explore::url_codec::CriteriaQueryDto` into
   `lotus_model::SearchCriteria`.
2. **Orchestrator dispatch ([`features/explore/orchestrator/runtime.rs`](../src/features/explore/orchestrator/runtime.rs)):**
   `start_search` validates criteria through
   `lotus_explore_core::explore::form_validation::validate_dispatch_criteria`,
   registers a new run on `SearchTaskController`
   ([`controller.rs`](../src/features/explore/orchestrator/controller.rs)) while
   cancelling any previous in-flight task, transitions `ExploreState` to loading
   via `reduce_mut`, and spawns `executor::do_search`
   ([`features/explore/executor.rs`](../src/features/explore/executor.rs)).
3. **Sequential input resolution ([`features/explore/service/results_pipeline/plan.rs`](../src/features/explore/service/results_pipeline/plan.rs)):**
   Before building the main occurrence query, `build_execution_plan` resolves
   free-text inputs in sequence:
   - [`service/resolve_taxon/`](../src/features/explore/service/resolve_taxon/) resolves taxon scientific or common names to Wikidata QIDs.
   - [`service/resolve_structure/`](../src/features/explore/service/resolve_structure/) resolves compound QIDs, InChIKeys, labels/aliases, or exact structures.
   - [`service/resolve_reference.rs`](../src/features/explore/service/resolve_reference.rs) resolves Wikidata QIDs or DOIs (`P356`).
4. **Execution plan & fetch ([`features/explore/service/results_pipeline/plan.rs`](../src/features/explore/service/results_pipeline/plan.rs)):**
   `build_execution_plan` calls
   `lotus_explore_core::explore::service::build_query::build_sparql_query` to
   build the base 15-column SPARQL `SELECT` and executes:
   - **`SplitStream` on `wasm32` ([`service/fetch_results/wasm.rs`](../src/features/explore/service/fetch_results/wasm.rs)):**
     rewrites the base query via `lotus_query::occurrences_query` into the slim
     4-column `(?compound ?taxon ?taxon_name ?r)` query, streams its CSV response
     chunk-by-chunk into `CsvColumnarReader` while reporting row progress, and
     then runs `lotus_query::dictionary_queries` in batches to hydrate compound
     and reference metadata (falling back to the 15-column WDQS query if QLever
     is unavailable).
   - **Native spool ([`service/fetch_results/native.rs`](../src/features/explore/service/fetch_results/native.rs)):**
     spools the 15-column CSV response to a temporary file and folds it into a
     `ColumnarResultSet` via `lotus_query::parse_compounds_columnar`.
5. **Columnar state commit & rendering ([`components/results_table.rs`](../src/components/results_table.rs)):**
   Fetched rows live in an `Arc<lotus_model::ColumnarResultSet>`. Client-side
   per-column filtering (`lotus_explore_core::filters::ColumnFilters`) and
   sorting (`lotus_explore_core::sort::{SortColumn, SortState}`) compute view
   indices over the columnar store without cloning row strings, and the
   virtualized table
   ([`components/results_table/virtualized_table.rs`](../src/components/results_table/virtualized_table.rs))
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
