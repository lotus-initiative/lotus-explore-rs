# lotus-explore-rs Architecture

The workspace layout --- what each crate is for, and which of them may be pure ---
is in [`docs/ARCHITECTURE.md`](../../../docs/ARCHITECTURE.md). This page covers
what this application adds on top of it.

`lotus-model`, `lotus-query`, `lotus-curation` and `lotus-jsonld` have no HTTP,
no clock, and no async runtime. They are built for `wasm32` in CI in their own
right, not only because the app happens to pull them in, and that is what keeps
them free of IO: a crate that cannot read a clock cannot read the current year
by accident, so it takes the year as an argument and its tests pin one.

`lotus-search` owns the decisions --- which endpoint, when to fall back, whether
a taxon string is a name or a QID --- and takes its transport as a two-method
trait. The whole use case, including the QLever-to-WDQS fallback, is tested
against a scripted HTTP conversation with no network at all.

The web app and the CLI both call `lotus-search` and `lotus-curation`, so a
query the explorer runs and a query `lotus search` runs are the same query, and
a curation file the web page reads and `lotus curate` reads is parsed the same
way. That was not true until recently: the repository once had two
implementations of each, which disagreed, and said nothing.

## What the application adds

The app owns what is about the host or the product rather than the domain.

```
src/
  app/          application bootstrap, shell, and root view
  components/   rendering-only UI components
  features/
    explore/    search, results, download, error recovery
    curation/   data curation workflows
  hooks/        Dioxus reactive hooks
  services/     cross-feature application services
  state/        shared application state
  api/          API client, DTOs, config, error types
  repositories/ data access (maps DTOs to domain types)
  clock.rs      the one place that reads a clock
  cache_key.rs  cache keys shared by the client and the server
  sort.rs       how the results table is ordered
  table_budget.rs  how many rows this machine will render
  sparql.rs     the app's HTTP layer: the only place that names reqwest
  download/     download effects (wasm + native)
  export/       export metadata and filename resolution
```

There are no re-export shims. A call site that needs a domain type writes
`lotus_model::` and one that needs a query builder writes `lotus_query::`, so
the crate a symbol comes from is visible where it is used. The app used to have
`models.rs` and `queries.rs` re-exporting both crates under names the app had
grown up with, which meant a reader could not tell from a call site whether a
type came from the domain or was a presentation wrapper, and a rename had to be
made in two places.

`sparql.rs` is the one module that keeps its own name, because it is not a
re-export: it is the layer that turns "run this on that endpoint" into a
request, and it is the only place in the app that names `reqwest`.

`runtime_table_row_limit` reads `navigator.deviceMemory` and sniffs the user
agent. That is a question about the machine asked by a browser, so it is in the
app rather than in a crate a query library depends on.

## Rules

- Components render and dispatch --- no business logic.
- Feature internals are private; each feature exposes a typed facade via
  `mod.rs`.
- API DTOs stop at repository boundaries and never reach components.
- State subscriptions are narrow --- components read only the slices they use.
- Every async path has a stable token; stale completions are discarded before
  state commit.
- Typed errors (`thiserror`) at all boundaries; user messages are derived
  separately.
- A crate that can be pure is pure. Anything that needs a year, a clock or a
  request takes it as a parameter.
- Both halves of a serialisation contract live together. The URL scheme was once
  split between a model method and a DTO, which is how a rename breaks every
  link already in a browser's history.

## Data flow

```
form → criteria → lotus-query (build) → lotus-search (run) → rows → table
```

On the server the same path runs with the app's own HTTP client in place of the
scripted transport `lotus-search` tests against.
