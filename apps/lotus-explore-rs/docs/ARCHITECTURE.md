# lotus-explore-rs Architecture

## The crates

Six library crates and one application. The crates are arranged so that
everything which can be pure is, and everything which makes a decision is named.

```
crates/lotus-model      Domain types, filter semantics, validation
crates/lotus-query      SPARQL construction and CSV parsing
crates/lotus-search     The search use case: resolve, run, fall back
crates/lotus-curation   The curation vocabulary and QuickStatements
crates/lotus-jsonld     Bioschemas JSON-LD, CodeMeta, CITATION.cff
crates/lotus-cli        The `lotus` binary
crates/lotus-web-assets Host-only: vendors Ketcher and RDKit, preloads wasm
apps/lotus-explore-rs   The web client and its optional native server
```

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
  table_budget.rs  how many rows this machine will render
  models.rs     the app's view of the domain
  sparql.rs     the app's view of talking to an endpoint
  queries.rs    the app's view of building a query
  download/     download effects (wasm + native)
  export/       export metadata and filename resolution
```

`models.rs`, `sparql.rs` and `queries.rs` are deliberately thin: they re-export
the crates under the names the app grew up with, and add a little. What they add
is the app's clock, which the pure crates require as an argument.

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
`Http` trait's recording double.

## Agent tooling

- `.github/ai/` --- AI collaboration guides, the contribution protocol, and the
  incident postmortem for the lotus-explore Qlever 429-storm fix.
