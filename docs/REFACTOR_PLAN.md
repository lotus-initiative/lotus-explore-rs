# Refactor plan

Audit → characterization tests → crate split → CLI → structured data → cleanup → docs.

## 1. What the repo is now

35 249 lines of Rust, 252 files, 526 tests, in 3 crates.

| Crate | Lines | Problem |
|---|---:|---|
| `crates/lotus` | 5 881 | Domain types, SPARQL builders, CSV parsing, `reqwest` transport and export format in one crate. `#[cfg(target_arch)]` leaks into "core" (`models/runtime.rs:55` reads `navigator.deviceMemory`; `export/filename.rs:10` forks on `js_sys::Date`). |
| `apps/lotus-explore-rs` | 28 650 | 250 files in one binary. The WASM client and the axum server compile in the same crate, so `main.rs:22` needs `#![allow(dead_code, unreachable_pub, unused_crate_dependencies)]`. `src/models.rs`, `src/queries.rs`, `src/sparql.rs` are pure re-export shims. |
| `crates/lotus-deploy` | 751 | Fine. Two host-only binaries. Untouched. |

The repo is already a standalone workspace root — nothing external to vendor.

## 2. Findings that drive the design

**Duplicated logic, in the same binary, with divergent behaviour:**

1. **Taxon resolution, twice.** `features/explore/service/resolve_taxon/mod.rs:47` + `match_selection.rs:21` vs `server/query_logic.rs:168`. Both sanitize, run `query_taxon_search`, parse, prefer a case-folded exact hit, warn on ambiguity. The server re-implements `sanitize_taxon_input` (`query_logic.rs:222`) instead of calling the app's. The two "is this already a QID?" checks differ (`resolve_taxon/mod.rs:34` vs `query_logic.rs:239`).
2. **Query assembly, twice — and they disagree.** `service/build_query.rs:33` downgrades a multiline structure from similarity to substructure (`build_query.rs:40`); `server/query_logic.rs:111` does not. The same criteria therefore produce different SPARQL from the UI and from `/v1/search`. `normalize_smiles` (`build_query.rs:12`) and `normalized_structure_input` (`query_logic.rs:138`) are the same function under two names.
3. **WDQS fallback policy, twice.** `repositories/hybrid.rs:156` `is_qlever_unavailable` and `features/curation/services/mod.rs:102` `should_fallback_to_wdqs` — same rule, two functions.
4. **Fallback state via `thread_local!` + free functions** (`hybrid.rs:14-20`), read six call-sites away in `results_pipeline/plan.rs:28,58`. Consequence: `plan.rs:27-52` and `plan.rs:54-82` are 25 duplicated lines differing only in the fetched rows.
5. **QID parsing, four times.** `transport/csv.rs:38`, `sparql/types.rs:219`, `sparql/types.rs:300`, `curation/services/helpers.rs::extract_qid_from_uri`.
6. **DOI normalisation, three times, two case policies.** `sparql/types.rs:273` preserves case; `curation/services/helpers.rs::normalize_doi` uppercases; `curation/services/inputs.rs:124` is a private third copy that also uppercases.

**Architecture gaps:**

7. **The HTTP trait is unusable outside its module.** `transport/client.rs:85,104` declares `HttpClient`/`HttpResponse` as `pub(super)`. Only `transport/execute.rs` can name them, so the CLI and the web app cannot inject a fake. Make them public and move them to `lotus-sparql`.
8. **No CLI.** The only binaries are the Dioxus/axum app and two deploy helpers. `clap` is already wired for server env/flag config (`server/config.rs:23`); nothing exposes search or curation to a terminal.
9. **No structured data.** `export/metadata.rs` emits a `schema.org/Dataset` with an invented vocabulary (`search_parameters`, `chemical_search_service`, `sparql_endpoint`), `data:text/csv` placeholder URLs (`metadata.rs:306-322`), and no `dct:conformsTo`. It is never injected into a document head — the web app's `<head>` carries no JSON-LD, so Google Dataset Search and the Bioschemas validator see nothing.

**Slop, concretely:**

- `crates/lotus/src/state.rs:48` `safe_hex_char` — a 22-line `match` (with an `unreachable!`) where `write!(f, "{byte:02x}")` is one line.
- 2 660 comment lines (7.6 %). Module docs that are tables of the module's own submodules: `crates/lotus/src/queries/mod.rs:10`, `crates/lotus/src/lib.rs:11`.
- Doc comments that restate the item: `transport/csv.rs:19` "Get a trimmed field value…", `queries/structure.rs:24` "Human-readable label…", `lotus/src/export.rs:16` "The three archive formats…".
- 144 `#[allow]`s, several in justification-comment piles: `main.rs:4-25` (6 allows, 3 paragraphs), `curation/services/mod.rs:32-36`, `sparql/types.rs:15`, `export/metadata.rs:155-161`.
- Tautological tests: `app_state.rs:67-103` (set a `bool` to `true`, assert it is `true`), `service/finalize.rs:127` (`metadata_json_is_non_empty`), `service/strategy.rs:65` (assert the enum's own discriminant).
- Generic names: `helpers.rs`, `internal.rs`, `utils/logging.rs`, `search_utils.rs`, `results_pipeline/plan.rs`, and 11 × `mod.rs` under `features/curation/services/`.

Non-issues, verified: no `unwrap`/`expect`/`panic!` outside `#[cfg(test)]` bodies; `cargo clippy --workspace --all-targets` and `cargo fmt` are already clean; `HttpResponse` aside, the app's own lint set is genuine.

## 3. Target crate graph

```
                  lotus-core            (pure: types, validation, query model; no IO/async)
                   │    │    │
        ┌──────────┘    │    └──────────┐
        │               │               │
  lotus-sparql   lotus-curation   lotus-schema
  builders +     TSV parse,       Bioschemas /
  CSV parse      quickstatements,  schema.org
  over Http      2nd-pass merge    JSON-LD +
  trait          over Knowledge    profile check
        │          trait                │
        └──────────────┬─────────────────┘
                       │
              ┌────────┴────────┐
         lotus-cli          lotus-web
         (clap bin)         (Dioxus wasm + axum server)
```

- `lotus-core` — `SearchCriteria`, `CompoundEntry`, `DatasetStats`, `TaxonMatch`, `SortState`,
  `SmilesSearchType`, `ElementState`, `StructureKind`, element/year limits, `ValidationFault`.
  Deps: `serde` only. `wasm32`-clean, no async.
- `lotus-sparql` — `pub trait Http` + `pub trait HttpResponse`; query builders
  (`query_all_compounds`, `query_sachem`, `query_with_server_filters`, `query_with_limit`,
  `query_counts_from_base`, `query_construct_from_select`); CSV parsers
  (`parse_compounds_csv_*`, `parse_counts_csv_bytes`, `parse_taxon_csv_bytes`);
  `Qid`/`Doi`/`non_empty` normalisers, one of each; WDQS fallback as a typed `Endpoint`.
  `reqwest` behind the default `reqwest` feature — the only place HTTP exists.
- `lotus-curation` — `CurationInputRow`, `CurationResultRow`, `CurationStatus`, TSV parsing,
  `row_uniqueness_key`, `build_quickstatements_bundle`, second-pass merge, and
  `trait CurationKnowledge` for compound/taxon/reference lookup. No HTTP; the web app and
  the CLI each supply an implementation.
- `lotus-schema` — `to_jsonld(&CompoundEntry)`, `taxon_jsonld`, `dataset_jsonld`,
  `software_jsonld` + `codemeta_json()` / `citation_cff()`, and `validate(&Value, Profile)`
  checking the Bioschemas minimum set (shapes fetched from `bioschemas.org`, table below).
- `lotus-cli` — `search`, `curate`, `serve`, `completions`, `man`. Thin.
- `lotus-web` — components, i18n, signals, the axum server behind `#[cfg(feature = "server")]`.
  Components stay small; state lives in reducers, not in `rsx!`.

## 4. Bioschemas minimum sets to validate against

Fetched from `bioschemas.org/profiles/bioschemas_profiles_shacl.jsonld`.

| Profile | Minimum (Violation) | Minimum (Warning) |
|---|---|---|
| MolecularEntity 0.5 | `dct:conformsTo`, `identifier`, `name`, `url` | `inChI`, `inChIKey`, `iupacName`, `molecularFormula`, `molecularWeight`, `smiles` |
| Taxon 1.0 | `dct:conformsTo`, `name`, `taxonRank` | `scientificName`, `parentTaxon`, `sameAs`, `url` |
| Dataset 1.0 | `dct:conformsTo`, `description`, `identifier`, `keywords`, `license`, `name`, `url` | `citation`, `creator`, `datePublished`, `distribution`, `includedInDataCatalog`, `measurementTechnique`, `variableMeasured`, `version`, `isBasedOn`, `alternateName` |
| DataCatalog 0.3 | `dct:conformsTo`, `description`, `keywords`, `name`, `provider`, `url` | `about`, `citation`, `dataset`, `dateCreated`, `identifier`, `license`, `sourceOrganization` |
| ComputationalTool 1.0 | `dct:conformsTo`, `description`, `name`, `url` | `applicationCategory`, `applicationSubCategory`, `author`, `citation`, `featureList`, `license`, `softwareVersion` |
| ScholarlyArticle 0.3 (draft) | `dct:conformsTo`, `identifier`, `name` | `dateModified`, `keywords`, `abstract`, `author`, `citation`, `dateCreated`, `datePublished`, `isPartOf`, `license`, `publisher`, `sameAs`, `url` |

## 5. Ordered steps

Each step keeps `cargo test --workspace` green; nothing lands in one big commit.

1. **Characterization tests** in place, before any move: query builders (one test per builder,
   asserting the SPARQL contract, not "contains SELECT"), CSV parsing against recorded
   fixtures, curation quickstatements, taxon match selection, export formats. `crates/lotus/tests/fixtures/*.csv`.
2. **`lotus-core`** — lift `models/*` and the pure constants. Delete `cfg(target_arch)` from
   the domain: `current_year` takes the year as input, `runtime_table_row_limit` becomes a
   web-side decision. `unsafe`-free, `serde`-only, `wasm32`-clean.
3. **`lotus-sparql`** — `queries/*` + `sparql/*` + `transport/*`. Promote `Http`/`HttpResponse`
   to `pub`, make `reqwest` a default-on feature, collapse the four QID parsers and the three
   DOI normalisers into one each in `lotus-core`. Delete `app`'s `models.rs`/`queries.rs`/`sparql.rs`
   shims in the same commit.
4. **Deduplicate** — one `TaxonResolver` in `lotus-sparql`, used by the web pipeline and the
   server; one `build_execution_query`; one WDQS-fallback helper returning a typed
   `Endpoint`. Kill the `thread_local!` channel: return the endpoint from the call.
5. **`lotus-curation`** — pure parts move verbatim; the seven `services/*` IO modules become
   one `impl CurationKnowledge for WebKnowledge` in `lotus-web`, and one in `lotus-cli`.
6. **`lotus-cli`** — `search` with every UI filter, `--format table|tsv|csv|json|jsonl|jsonld`,
   `--limit`/`--offset`, stdin, stdout/stderr split, exit codes. `curate` with `--dry-run`
   default and TSV in/out. `completions` + `man` via `clap_complete`/`clap_mangen`;
   `docs/cli.md` with a test that fails if it drifts from `--help`.
7. **`lotus-schema`** — the four JSON-LD builders, the profile checker, `codemeta.json`,
   `CITATION.cff`, `insta` snapshots.
8. **Wire it up** — CLI `--format jsonld`; web head injection per view plus OpenGraph.
9. **Cleanup** — strip restating comments, banners, `Step 1/2/3` narration, `helpers.rs`/
   `internal.rs`/`utils` names, the tautological tests, the `#[allow]` piles.
10. **Docs** — README for users, `CONTRIBUTING.md` for developers, offline test story.

## 6. Non-goals

- No change to what the web app renders or how the URLs behave.
- No new runtime dependency in a library crate; `thiserror` for library errors, `anyhow`
  in binaries only.
- `lotus-deploy` stays as it is.
- No live network in the default `cargo test --workspace` run.
