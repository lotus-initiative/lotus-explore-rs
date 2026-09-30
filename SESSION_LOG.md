# Session log

Append-only. Newest section at the bottom.

## Baseline — start of the slop-polish pass

Recorded before any change, so every later number has something to compare
against. Reproduce with `python3 tools/slop-metrics.py`, `cargo llvm-cov`,
`cargo mutants`, and `just test`.

### Crates

| crate | files | LOC | files >250 | fns >40 |
|---|---:|---:|---:|---:|
| apps/lotus-explore-rs | 225 | 32597 | 38 | 95 |
| crates/lotus-cli | 5 | 2737 | 4 | 7 |
| crates/lotus-curation | 9 | 2669 | 4 | 6 |
| crates/lotus-jsonld | 8 | 2056 | 4 | 6 |
| crates/lotus-model | 7 | 1667 | 3 | 2 |
| crates/lotus-query | 7 | 2408 | 5 | 4 |
| crates/lotus-search | 9 | 1952 | 4 | 2 |
| crates/lotus-web-assets | 2 | 1667 | 2 | 4 |
| **total** | **272** | **47753** | **56** | **126** |

### Twenty largest files

```
 1198  crates/lotus-web-assets/src/fetch_assets.rs
 1026  crates/lotus-curation/src/knowledge.rs
  937  apps/lotus-explore-rs/build.rs
  790  apps/lotus-explore-rs/src/server/tests.rs
  769  crates/lotus-cli/src/main.rs
  671  crates/lotus-cli/src/output.rs
  654  crates/lotus-query/src/query.rs
  597  crates/lotus-query/src/parse.rs
  597  crates/lotus-cli/src/curate.rs
  535  crates/lotus-curation/src/structure.rs
  494  apps/lotus-explore-rs/src/components/data_curation_page/sections/mod.rs
  487  apps/lotus-explore-rs/src/server/handlers.rs
  481  apps/lotus-explore-rs/src/app/routes.rs
  478  apps/lotus-explore-rs/src/features/curation/services/enrichment.rs
  469  crates/lotus-web-assets/src/inject_wasm_preload.rs
  469  crates/lotus-cli/tests/cli.rs
  458  apps/lotus-explore-rs/src/components/results_table/sort_model.rs
  450  apps/lotus-explore-rs/src/upload.rs
  448  apps/.../results_table/table_toolbar_sections/download_actions.rs
  445  crates/lotus-search/src/execute.rs
```

### Longest non-test functions

```
 293  apps/.../features/curation/services/enrichment.rs:68   fn enrich_and_generate
 183  apps/.../src/i18n/fr.rs:10                             fn fr_t
 183  apps/.../src/pages/ketcher_panel.rs:55                 fn KetcherPanel
 182  apps/.../src/i18n/it.rs:10                             fn it_t
 179  apps/.../src/i18n/de.rs:10                             fn de_t
 167  apps/.../src/i18n/en.rs:10                             fn en_t
 156  apps/.../src/components/layout/footer.rs:14            fn Footer
 148  apps/.../results_table/.../download_actions.rs:301     fn DownloadActionsGroup
 133  apps/.../form_sections/formula_section.rs:104          fn FormulaSection
 119  apps/.../components/search_panel.rs:93                 fn StructureSection
 111  apps/.../data_curation_page/sections/mod.rs:103        fn AddRowCard
 105  apps/.../src/export/filters.rs:7                       fn criteria_to_filters_value
 105  crates/lotus-web-assets/src/fetch_assets.rs:346        fn fetch_curation_assets
  97  apps/.../components/layout/dark_mode_toggle.rs:17      fn DarkModeToggle
  96  apps/.../src/server/query_logic.rs:15                  fn apply_request
```

126 non-test functions are over 40 lines. The four `*_t` locale functions are
translations held in one place on purpose; the rest are real candidates.

### Lints and hygiene

| metric | count |
|---|---:|
| clippy pedantic + nursery denies active | yes, workspace-wide |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `#[allow(…)]` in non-test code | 75 |
| `#[allow(…)]` in test code | 6 |
| `.unwrap()` in non-test code | 11 |
| `.expect(…)` in non-test code | 169 |
| `.expect(…)` in test code | 142 |
| commented-out code lines | 6 |
| unused dependencies (`cargo machete`) | 0 |
| `cargo deny check` | clean |

Most of the 169 non-test `expect`s are in the app's Dioxus render paths, where
clippy's `expect_used` is locally allowed. Each needs reading.

### Tests

| metric | value |
|---|---:|
| test functions | 849 |
| **doctests** | **0** |
| `cargo test --workspace` runtime | ~22 s |
| ignored (live network) | 0 in the default run |

The zero doctests is the largest single gap: no crate has a doctested example,
and no crate README is included as crate documentation, so nothing in the
documentation is compiled by the test suite.

### Coverage

Region coverage from `cargo llvm-cov`, excluding `build.rs` and test files.

| crate | regions covered |
|---|---:|
| crates/lotus-model | 95.3% |
| crates/lotus-curation | 70.3% |
| crates/lotus-query | 80.2% |
| crates/lotus-jsonld | 88.7% |
| crates/lotus-web-assets | 79.2% |
| crates/lotus-cli | 56.1% |
| crates/lotus-search | 47.2% |

### Mutation testing

`cargo mutants` over every library crate except the app: **931 mutants**.

### Bundle size

`target/dx/lotus-explore-rs/release/web/public`, before `just web-bytes`
pruned the superseded bundle:

```
wasm module   1 687 255 B raw
JS glue         55 376 B raw   (11 KiB brotli)
CSS             51 673 B raw   ( 8 KiB brotli)
```

RDKit's `RDKit_minimal.wasm` (7 333 095 B) is a separate vendored asset loaded
on demand by the structure editor, not part of the initial payload.
