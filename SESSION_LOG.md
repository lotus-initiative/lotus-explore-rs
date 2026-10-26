# Session log — decoupling the app from Dioxus

Goal: make a future move to another UI framework (Leptos, Yew, plain `web-sys`)
touch only a thin UI layer, and leave the rest of the code untouched.

Two passes:

- **Pass 1** — structural refactor: a Dioxus-free core, a thin Dioxus adapter,
  and a mechanical gate that keeps the dependency direction from regressing.
- **Pass 2** — coverage: baseline, prioritise, add tests, report before/after.

No functional change is intended in Pass 1.

---

## Pass 1 — Step 1: audit

### 1.1 What is already clean

The `lotus-*` library crates have **no** `dioxus` dependency today
(`crates/lotus-model`, `lotus-query`, `lotus-search`, `lotus-curation`,
`lotus-jsonld`, `lotus-web-assets`, `lotus-cli` — verified with
`grep dioxus crates/*/Cargo.toml`). The leak is entirely inside
`apps/lotus-explore-rs`, which is one crate holding four programs (browser
client, desktop window, HTTP API, test build) and 48 301 lines of Rust.

So the audit below is of **one** crate: the app.

### 1.2 Every place a Dioxus type reaches non-view code

`Signal` (142), `Element` (134), `use_signal` (47), `use_context_provider` (22),
`spawn` (21), `use_memo` (16), `use_effect` (13), `Memo` (9), `use_context` (8),
`Props` (7), `Properties` (5), `Task` (3), `Attribute` (3), `use_hook` (2),
`Scope` (2), `Readable` (1), `Component` (1).

| File (under `apps/lotus-explore-rs/src/`) | Symbol | Dioxus leak | Proposed destination |
|---|---|---|---|
| `state.rs` | `AppStateContext { state: Signal<AppState> }`, `ResultsContext { explore: Signal<ExploreState> }`, `use_app_state_context`, `use_results_context` | `Signal`, `use_context` | **adapter** (`ui/context.rs`); the `AppState` / `ExploreState` payloads move to **core** |
| `state/form_context.rs` | `FormCriteriaContext { criteria: Signal<SearchCriteria>, baseline: Signal<SearchCriteria> }` | `Signal` | **core**: `FormState { criteria, baseline }` + pure `apply`; **adapter**: signal-backed wrapper |
| `hooks/use_locale.rs` | `use_locale`, `use_locale_signal` | `use_context`, `Signal` | **adapter** |
| `hooks/locale_provider.rs` | `LocaleProvider` | `#[component]`, `use_context_provider` | **adapter** |
| `hooks/use_add_row_form.rs` | `AddRowForm { name, smiles, taxon, doi: Signal<String> }`, `use_add_row_form` | `Signal`, `use_signal` | **core**: `AddRowFormState` + pure `try_add`; **adapter**: signal wrapper |
| `hooks/use_virtualization.rs` | virtualization hook | `use_hook`, `Signal` | **adapter** (a scroll port) |
| `features/explore/selectors.rs` | `use_lifecycle_selector`, `use_result_selector`, `use_result_arc_selector`, `use_ui_selector`, `use_criteria_selector`, `use_toolbar_result_snapshot`, `use_header_meta_snapshot` | `Signal`, `use_memo`, `Memo` | **core**: the snapshot builders (`toolbar_snapshot_from_result`, `header_meta_snapshot_from_result`, `ExploreUiState::from_explore`); **adapter**: the `use_memo` wrappers |
| `features/explore/search_state/dispatch.rs` | `dispatch_explore_action(Signal<ExploreState>, ExploreAction)` | `Signal` | **adapter**; **core** keeps the pure `reduce_mut` |
| `features/explore/lifecycle.rs` | `SearchLifecycleCoordinator { explore: Signal<ExploreState> }` | `Signal` | **core**: pure functions over `&mut ExploreState`; **adapter**: a thin coordinator that owns the signal |
| `features/explore/interactions.rs` | `ExploreInteractions { criteria, form, explore: Signal<…> }` | `Signal` | **adapter** |
| `features/explore/download_dispatch.rs` | `use_startup_effect`, `use_download_dispatch_effect` | `use_effect`, `spawn`, `Signal` | **adapter**; **core** keeps `download_effects/*` (the decisions) |
| `features/explore/orchestrator/controller.rs` | `SearchTaskController { in_flight: Rc<RefCell<Option<Task>>> }` | `Task` | **adapter** |
| `features/explore/orchestrator/runtime.rs` | `start_search(Signal<…>, …)`, `spawn` | `Signal`, `spawn` | **adapter** |
| `features/explore/executor.rs` | `SearchExecutor<R, P, G>` | — (no Dioxus, but depends on the app's repository trait) | **core**, once the `LotusRepository` port is in **core** |
| `features/curation/state/page_controller.rs` | `CurationPageController` (nine `Signal`s), `use_curation_page_controller`, `start_curation_run`, `import_tsv_rows` | `Signal`, `use_signal`, `spawn` | **core**: `CurationPageState` + pure transitions; **adapter**: signal wiring |
| `features/curation/services/http_client.rs` | `rdkit_bridge_call` | `document::eval` | **adapter** (a browser side effect) |
| `document_head.rs` | `CurationScripts`, `AppStylesheet` | `#[component]`, `asset!`, `document::eval`, `use_effect` | **adapter** |
| `vendor_assets.rs`, `desktop_assets.rs` | `asset!` | `asset!` | **adapter** |
| `components/**` (9 908 lines) | every `rsx!`, `#[component]`, `EventHandler` | `rsx!` | **adapter** |
| `ui/notice.rs`, `ui/segmented_control.rs` | `NoticeBarProps { trailing: Option<Element>, children: Option<Element> }` | `Element` | **adapter** |
| `app/routes.rs` | `Route` | router | **adapter** |
| `main.rs` | `launch(AppRoot)` | launcher | **adapter** |
| `upload.rs` | `&[dioxus::html::FileData]` | `FileData` | **adapter** (file upload is a side effect) |
| `api/config.rs` | `api_base_url` | `web_sys` | **adapter** (platform) |
| `clock.rs` | `current_year` | `js_sys::Date::now` on wasm | **adapter**; **core** takes the year as an argument |
| `export/filename.rs` | `now_iso8601`, `today_yyyymmdd` | platform clock | **adapter**; **core** takes the date string |
| `export/filters.rs`, `api/dto.rs`, `features/explore/service/build_query.rs` | `crate::clock::current_year()` | platform clock | **core**: the year becomes an argument |

### 1.3 What is already pure and stays pure

The reducer (`search_state/reducer/`), the form actions and validation rules,
the URL codec, the export metadata builder, the TSV reader, the snapshot
builders, the retry policy and the error classification are **already** plain
Rust with no Dioxus type in them. They are moved as-is; the work is the
*boundary*, not a rewrite of the logic.

---

## Pass 1 — Step 2: target layout

### 2.1 Crates

| Crate | Contains | Depends on |
|---|---|---|
| `lotus-model` … `lotus-cli` | unchanged | unchanged |
| **`crates/lotus-explore-core`** (new) | the app's **domain**: explore state + reducer + actions, validation, URL codec, query building, finalization, curation vocabulary, export formatting, filters/sort, API DTOs, i18n message tables, the transport **ports** | `lotus-model`, `lotus-query`, `lotus-search`, `lotus-curation`, `serde`, `thiserror` — **no `dioxus*`** |
| `apps/lotus-explore-rs` | the **Dioxus adapter**: every `rsx!`, `Signal`, `use_*` hook, `#[component]`, the router, `asset!`, `document::eval`, the desktop launcher, the HTTP API server | `lotus-explore-core` + the five library crates + `dioxus` |

The dependency arrow points inward: the core does not know the adapter exists.

### 2.2 Module map

`lotus-explore-core`:

```
src/lib.rs
src/api/          dto.rs  error.rs                  — the wire contract
src/curation/     mod.rs  queue.rs  workflow.rs  share_links.rs
src/export/       mod.rs  filename.rs  filters.rs  metadata.rs  urls.rs
src/explore/
    actions.rs  command.rs  request.rs  outcome.rs  types.rs
    errors.rs  recovery.rs  retryable_orchestrator.rs
    search_metrics.rs  search_utils.rs  sparql_errors.rs
    transport_classification.rs  truncation.rs
    form_actions.rs  form_validation/  url_codec/  url_state.rs
    state/         — `ExploreState` and the reducer (`reduce_mut`)
    download_effects/  — the dispatch/startup decisions
    query/         — build_query.rs  finalize.rs  strategy.rs
src/filters.rs  src/sort.rs  src/sparql.rs  src/table_budget.rs
src/ports.rs     — `LotusRepository`, `RepositoryError`
src/i18n/        — the message tables
```

`apps/lotus-explore-rs` keeps: `components/`, `app/`, `pages/`, `ui/`,
`hooks/`, `document_head.rs`, `vendor_assets.rs`, `desktop_assets.rs`,
`clock.rs`, `download/`, `upload.rs`, `perf/`, `cache/`, `repositories/`
(the transport implementations), `services/`, `server/`, `main.rs`, and the
thin signal-wiring modules under `features/` (`explore/{executor,orchestrator,
lifecycle,interactions,selectors,download_dispatch,service}` and
`curation/{services,state}`).

### 2.3 State

UI state becomes plain structs plus pure update functions:

- core: `ExploreState` + `reduce_mut(&mut ExploreState, ExploreAction)` — already
  the shape; it moves to the core crate.
- core: `FormState { criteria, baseline }` + `apply_form_action` — already the
  shape behind `FormCriteriaContext`.
- core: `CurationPageState` + pure transitions — **new**, replacing the nine
  `Signal`s the controller holds today.
- adapter: the signals that hold those structs, and the wiring that turns a
  DOM event into a call into a pure function.

### 2.4 Side effects

Ports defined in the core, implemented in the adapter:

| Port | Where | Implementation |
|---|---|---|
| `LotusRepository` (SPARQL + REST search) | `core::ports` | `repositories::HybridRepository` (app) |
| clock (`current_year`, `now_iso8601`) | core takes `year: u16` / a date string as arguments | `clock.rs` (app) |
| browser document / `document::eval` | — | `features/curation/services/http_client.rs` (app) |
| file upload | — | `upload.rs` (app) |
| clipboard / storage / timers | — | `download/`, `components/results_table/scroll_*` (app) |

No generic UI-framework abstraction is introduced: Dioxus stays visible in one
crate, in the files that actually use it.

### 2.5 The mechanical gate

A new `./mk boundary` task:

```sh
cargo tree -p lotus-explore-core --edges normal,build | grep -i dioxus   # must be empty
```

plus a source scan of `crates/lotus-explore-core/` for `dioxus` (which catches a
`use` that a `cargo tree` cannot see, such as a path dependency in a dev-only
section). Wired into `./mk ci`, `./mk ci-fast` and a `prek` pre-push hook, so
the direction cannot regress.

---

## Pass 1 — Step 3: execution log

(filled in as the work proceeds)

## Pass 2 — coverage

(not started)
