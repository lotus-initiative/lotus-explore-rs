# Pitfalls — invariants that were broken once

Each item was a shipped bug with a regression test now. Keep the invariant;
the named test fails if it breaks.

## Curation and QuickStatements

- SMILES case is chemistry (`c` aromatic, `Co` cobalt). Never case-fold a
  structure. Taxon and DOI do fold. Test: `smiles_that_differ_only_in_case_are_different_findings`.
- `P703` and `P248` take items. Never emit a quoted string for them; with no
  resolved QID, emit nothing and say so in the row note (`--offline`).
- A new item is reachable only through `LAST` inside its own `CREATE` block.
  One new compound in several rows must be one `CREATE` carrying every
  occurrence: call `lotus_curation::merge_new_compound_creates` on the batch
  before `build_quickstatements_bundle`, in every front end (CLI `curate.rs`,
  app `services/pipeline.rs` and `workflow.rs`).
- Statements come in two shapes: one block per element (`## name\nCREATE\n  LAST|…`,
  crate/CLI) and one line per element (`CREATE`, `LAST|…`, app). Code that
  edits statements handles both.
- A structure the service cannot read fails its row (no InChIKey, status
  `Error`), never the batch. An unparsable service reply fails the batch.
- Several Wikidata items per InChIKey: `compound_by_inchikey_query` orders by
  `STRLEN(STR(?compound)) ?compound LIMIT 1` and `lotus_curation::qid_number`
  compares QIDs numerically (never lexicographic `MIN`).
- `lotus_curation::escape_quickstatements` is the single QuickStatements
  escaper across CLI and web app: escapes `"` and flattens `|` and control
  characters to `' '`.
- SPARQL `ASK` JSON parsing (`parse_ask_boolean`) requires an explicit boolean
  `"boolean"` field rather than defaulting missing values to `false`.
- A new compound is `P31 Q113145171` (type of chemical entity) + `P279 Q11173`
  (chemical compound), never `P31 Q11173`, in every front end. The web app
  uses `P31 Q59199015` instead when stereochemistry is undefined.

## Query building and search

- A triple a rewriter re-adds must land inside the group of the filter that
  uses it. Assert position, not presence (`counts_query`, `occurrences_query`).
- `lotus_search::search` asks for `LIMIT n + 1`; `truncated` is exact. When the
  count query fails on a truncated page, `stats` is `None`, not page counts.
- A query that ran out of time is never retried, on either endpoint: `QLever`
  429 (`Operation timed out`), WDQS v2 504 (`gateway_timeout` /
  `backend_timeout` in `wdqs-proxy`), and legacy WDQS 5xx with
  `java.util.concurrent.TimeoutException` all become `FetchError::TimedOut`;
  the app maps that to 429 "…time limit…", classified `QueryTooExpensive`. A
  429 from WDQS v2's REST Gateway (or a 4xx saying "too many" / "rate limit")
  is a rate limit (`FetchError::Http { status: 429, .. }`), never `TimedOut`.
  Never send `LOTUS_QLEVER_TOKEN` in `Authorization: Bearer` to non-QLever
  endpoints (Wikimedia's REST Gateway rejects non-JWT tokens with HTTP 401).
- The native HTTP client has an idle `read_timeout` (`CONNECTION_IDLE_TIMEOUT`,
  90 s) and no total `timeout`; long streams are legitimate. The idle deadline
  covers waiting for headers, so it stays above WDQS's 60 s limit. Streamed
  bodies also have their own 45 s stall watchdog.
- On `wasm32`, never yield with `TimeoutFuture::new(0)` / `setTimeout(0)` in
  streaming, split-query steps, or uploads (`crates/lotus-search/src/reqwest_client/wasm.rs`,
  `hybrid.rs`, `upload.rs`): Firefox clamps background timers to >= 1 s (and
  up to 10 s under budget throttling), stalling streams until QLever times out.
  Use `lotus_search::reqwest_client::yield_to_browser` (`MessageChannel`)
  gated by `YieldSchedule`, bound browser `send`/`bytes`/`text` with
  `wait_deadline` (`CONNECTION_IDLE_TIMEOUT`, never `RequestBuilder::timeout`),
  and spawn app-wide search tasks via `spawn_forever` so route/component
  unmounts do not cancel the task and leave `loading == true`.
- Every surface validates with `lotus_model::validate_criteria`: CLI,
  `lotus_search::search`, and the API's `apply_request`.
- Formula fields reach the API only when `formula_enabled`.
- Header parsing strips a UTF-8 byte-order mark (CSV stream, curation TSV).
- Sanitize QIDs with `lotus_model::normalize_qid` before interpolating
  `wd:{qid}`; `lotus_model::strip_doi_prefix` strips `doi:`,
  `http(s)://doi.org/`, and `http(s)://dx.doi.org/`.
- `construct_from_select` strips trailing `LIMIT`/`OFFSET` via
  `rsplit_once("\nLIMIT ")`.
- Preserve leading blank lines in the 3-line Molfile header block across search,
  URL encoding, and cache keys (do not `.trim()` a Molfile).
- `RowExporter` omits empty CSV cells when emitting JSON bindings; token-budget
  error extraction parses multi-digit durations (`> 30s`).
- `compound_label_query` and `compound_alias_query` enumerate deduplicated
  casing variants (`name_case_variants`) across `COMPOUND_NAME_LANGUAGES` in
  `VALUES ?name { ... }` instead of `LCASE(STR(...))` (which scans 17.9M labels
  and times out), use `SELECT DISTINCT`, and require `?compound wdt:P235
  ?_inchikey .` so capitalized names do not resolve to trade names, scholarly
  articles, songs, or disambiguation pages (or shadow chemical `skos:altLabel`
  matches such as `Taxol`). `lotus_search::resolve_structure` deduplicates QIDs
  in first-seen order. Tests:
  `compound_name_lookups_expand_case_variants_and_require_inchikey`,
  `compound_name_case_variants_cover_mixed_multiword_punctuated_hyphenated_and_cjk`,
  `compound_name_lookup_sends_case_variants_and_inchikey_guard_and_dedups_qids`,
  `lowercased_compound_name_sends_case_variants_with_p235_and_reuses_cache_across_cases`.

## Server and exports

- `apply_request` passes raw `min`/`max` bounds to `validate_criteria` without
  `.max()` clamping; `/v1/search` and `/v1/compounds` fetch `limit + 1` and
  propagate count-query errors via `resolve_count_stats` rather than
  substituting page counts.
- `ApiError::from_fetch` and `/v1/export` map upstream timeouts
  (`FetchError::TimedOut` / handler timeout) to HTTP `504`.
- `run_pubchem_tree` checks HTTP 2xx and propagates NPClassifier CSV
  fetch/parse errors.
- `lotus-healthcheck` loops `read` until `\n`/EOF and falls back to `PORT` env
  when the CLI port argument is omitted.

- Work that outlives a handler (spawned export pump) owns its permits:
  `try_acquire_owned`, moved into the task.
- In-flight cells are removed by any request that awaited them, not only the
  creator, or a cancelled creator pins the result.
- A streamed export that fails ends the body with an `Err` and no gzip trailer.
  A clean finish on failure ships a valid partial file.
- CSP inline-script digests live in two files: `_headers` (generated by
  `apps/lotus-explore-rs/build.rs`) and `docker/snippets/security-headers.conf`
  (by hand). A build test checks both against `index.html`.
- CSV output ends after the last row's newline. No blank last line.

## Tooling

- After a toolchain bump, lint every build: default, `--features server`,
  `--target wasm32-unknown-unknown`. `./mk gate` does.
  A new lint firing inside Dioxus macro output gets the narrowest allow
  possible plus a `docs/DEBT.md` row.
- Source-shape gates (`server/handlers/export_query_shape.rs`,
  `download/streaming_gate`, `fetch_results/no_limit_gate`) match literal code.
  Update them with the line they pin.
- Test doubles implementing `lotus_search::HttpResponse` need
  `#![expect(clippy::unused_async_trait_impl, reason = …)]`.
- In scripts, use `git diff --no-ext-diff`: a contributor's git may use an
  external diff tool.

## Known gaps (open, not fixed)

- `lotus export` holds the whole graph in memory (docs say so).
- `lotus-web-assets` fetches RDKit and Ketcher at `latest`, with no checksum
  and with redirects followed.
- CLI output files are written in place, not atomically.
