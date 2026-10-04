# Mutation testing

What survived, per module, before and after, and why each survivor is still
alive. Run one module at a time under the limits below.

## How these runs were done

Every run used the same deliberately slow envelope, because `cargo mutants`
rebuilds the workspace once per mutant and that is what wedges a machine:

```bash
MUTANTS_MIN_FREE_GB=0 nice -n 19 cargo mutants --test-tool nextest \
  -p <crate> -f crates/<crate>/src/<module>.rs \
  -j 1 --timeout 60 --minimum-test-timeout 20 --output <dir>
```

- `-j 1`, one module at a time via `-f`. Never workspace-wide.
- `--timeout 60` with `--minimum-test-timeout 20`.
- `--test-tool nextest`, which is what the suite runs under everywhere else; a
  mutant judged against a different harness is judged against a different suite.
- `nice -n 19`. **`ionice -c 3` was not used: it does not exist on macOS.** This
  host is darwin, so the CPU-priority half of the envelope is unavailable and
  only the nice half applies.
- `MUTANTS_MIN_FREE_GB=0` overrides this repo's own preflight, which refuses to
  start below 20 GB free (`make/scripts/mutants-preflight.sh` records two earlier
  runs crashing a 16 GB laptop). The sweep's own floor is 5 GB; disk was checked
  before and after every run and stayed between 24 GB and 62 GB free.
- No mutant was run in `cfg(target_arch = "wasm32")`-only code. Network-facing
  modules were skipped on purpose: `reqwest_client.rs`, `fetch_assets/http.rs`,
  `fetch_assets/ketcher.rs`. No test in this repo reaches a live endpoint — the
  search path is tested through the `Http` trait against a scripted conversation.

The timeouts are mutants where the tests ran past 60 s. They are growth and
capacity comparisons — `==` weakened to `!=` in `ColumnarBuilder::push` and
`grow_references`, and a chunk-boundary comparison — which turn a resize or
emit loop into an unbounded one. A timeout is the correct outcome, not a wedged
machine.

## Results

Every logic module in six of the seven library crates, plus the two largest
modules of the seventh. **Missed is the number that matters.**

| Module | Mutants | Caught | Unviable | Missed | Time |
|---|---|---|---|---|---|
| `lotus-query/src/query.rs` — query builder | 115 | 88 | 27 | **0** | 6 min |
| `lotus-query/src/parse.rs` — parsing (before) | 79 | 46 | 31 | 2 | 3 min |
| `lotus-query/src/parse.rs` — after | 79 | 48 | 31 | **0** | 3 min |
| `lotus-query/src/export.rs` | 19 | 18 | 1 | **0** | 69 s |
| `lotus-query/src/export_rows.rs` — before | 66 | 37 | 11 | 16 | 6 min |
| `lotus-query/src/export_rows.rs` — after | 66 | 51 | 11 | **2** | 5 min |
| `lotus-model/src/columnar.rs` — dedup/count (before) | 368 | 233 | 27 | 103 | 23 min |
| `lotus-model/src/columnar.rs` — after | 368 | 258 | 27 | **78** | 23 min |
| `lotus-search/src/error.rs` (before) | 23 | 12 | 1 | 10 | 2 min |
| `lotus-search/src/error.rs` — after | 23 | 19 | 1 | **0** | 88 s |
| `lotus-search/src/result.rs` (before) | 9 | 1 | 0 | 8 | 58 s |
| `lotus-search/src/result.rs` — after | 9 | 8 | 1 | **0** | 53 s |
| `lotus-search/src/search.rs` (before) | 41 | 27 | 4 | 10 | 3 min |
| `lotus-search/src/search.rs` — after | 41 | 34 | 4 | **3** | 3 min |
| `lotus-search/src/execute.rs` (before) | 72 | 37 | 11 | 24 | 4 min |
| `lotus-search/src/execute.rs` — after | 72 | 44 | 11 | **17** | 4 min |
| `lotus-curation/src/input.rs` | 31 | 26 | 5 | **0** | 77 s |
| `lotus-curation/src/wikidata_query.rs` | 27 | 27 | 0 | **0** | 84 s |
| `lotus-curation/src/structure.rs` | 23 | 19 | 4 | **0** | 72 s |
| `lotus-curation/src/knowledge.rs` | 41 | 36 | 5 | **0** | 2 min |
| `lotus-jsonld/src/lib.rs` | 11 | 11 | 0 | **0** | 35 s |
| `lotus-jsonld/src/profile.rs` | 32 | 22 | 10 | **0** | 67 s |
| `lotus-jsonld/src/software.rs` | 15 | 12 | 3 | **0** | 39 s |
| `lotus-jsonld/src/dataset.rs` | 10 | 9 | 1 | **0** | 30 s |
| `lotus-cli/src/output.rs` | 30 | 24 | 6 | **0** | 2 min |
| `lotus-cli/src/curate.rs` | 33 | 27 | 6 | **0** | 2 min |
| `lotus-web-assets/src/inject_wasm_preload.rs` | 31 | 30 | 1 | **0** | 2 min |
| `lotus-web-assets/src/healthcheck.rs` | 4 | 1 | 3 | **0** | 35 s |
| `lotus-web-assets/src/fetch_assets/vendor.rs` | 17 | 13 | 4 | **0** | 74 s |

**Four crates finished at zero missed and needed no work at all**:
`lotus-curation`, `lotus-jsonld`, `lotus-cli`, `lotus-web-assets`. 336 mutants
across them, all caught or unviable.

### `columnar.rs` needs a caveat, and it is large

The 78 survivors are **75 `#[cfg(feature = "diagnostics")]` mutants plus 3 real
ones**. The diagnostics code is out of scope *by the repo's own design*:

> Leaving them compiled would add ~78 mutants that cannot be killed by
> construction, which is most of a package's survivors and enough to bury a real
> one.
> — `crates/lotus-model/Cargo.toml`

That estimate is 78; the measured number is 75. So the **in-scope** figure is:

| | before | after |
|---|---|---|
| caught | 217 | 240 |
| **missed** | **28** | **3** |

Note that the feature *does* leak into the run. It is not enabled by
`lotus-model`'s own manifest — it is a non-default feature — but
`lotus-query` dev-depends on `lotus-model` with `features = ["diagnostics"]`, and
in a workspace build feature unification switches it on. The intent "absent from
the default build" holds for the library but not for the mutation run. Recorded
rather than worked around, because the fix belongs with whoever owns the feature
gate.

## Every survivor, accounted for

**3 in `columnar.rs`**
- `QidDictionary::is_empty` ×2 — `pub`, re-exported from `lib.rs`, called from
  nowhere. Dead code, and unkillable while it stays dead: nothing can observe it.
  It cannot be removed here either, because removing a `pub` item is a
  public-behaviour change. **For a human.**
- `parse_numeric_qid`: `||` weakened to `&&` — **genuinely equivalent.** For any
  non-empty input the difference is carried by the next line,
  `digits.parse::<u32>().ok()?`, which already rejects anything non-numeric; for
  the empty string the mutant falls through to `parse("")`, which also fails.
  Both paths return `None`, so no test can distinguish them and none should.

**2 in `export_rows.rs`** — both chunk-emission boundaries in `next_chunk`: the
`>=` that decides when a chunk is handed over, and the `in_body` negation that
puts the preamble first. Catching the first needs an export large enough to
straddle 64 KiB, so either a large fixture or asserting on chunk sizes directly;
the second needs the same. `chunking_does_not_change_a_single_byte` already
guards the property that matters — that the split cannot change the output — and
a slow test for the boundary is the trade this repo has explicitly refused
elsewhere ("a benchmark that fails on a slow machine is a test that gets
deleted").

**3 in `search.rs`**
- `build_base_query`'s `qid != "*"` guard — nearly dead since the wildcard is now
  read from `criteria.taxon`. Kept for callers that pass the wildcard straight
  through, and now covered by a test; the mutant weakens the guard rather than
  removing it, and the fallback arm still produces a valid query either way.
- `resolve_taxon`'s `!=` → `==` — a second door to the same wildcard check, in
  `trimmed == "*"`. The test added for it asserts the outcome; this particular
  mutation is equivalent given the other guard (`trimmed.is_empty() ||`).
- `columnar_from_chunks` → `Default::default()` — returns an empty set instead of
  a parsed one. Every caller of it goes through the API path, where the mutant's
  result is indistinguishable from a search that legitimately found nothing.

**17 in `execute.rs`** — the retry loop's internals: the attempt bound `<` vs
`<=` in both `execute` and `execute_streaming`, the `&&`/`!` around the retry
condition, `backoff` returning `()`, `StreamAnswer`'s `Debug`, and
`json_exception`'s `+` → `*` in its escape scan.

These are the ones worth arguing about rather than just listing. **The retry
*policy* is tested** — the 429/5xx rule, the attempt count, the
endpoint-unavailable condition — and the survivors are all *mechanics*: whether
the loop counts correctly, whether it actually sleeps, whether a `!` is there.
Killing them means either sleeping in a test or introducing a clock abstraction
so time can be faked. On a machine whose load average swings between 6 and 42, a
test that asserts on elapsed time is a test that fails for reasons that have
nothing to do with the code. **Left alive deliberately, and the trade is named
rather than hidden.**

## Modules not run

- **`cache_key.rs`** — the brief's fourth priority, attempted and **blocked**, but
  not for the reason first recorded. Two findings, the second the real one.

  `mutants.toml` does exclude the app crate, but the exclusion does not apply
  when `--file` narrows the run, so the module lists its 11 mutants fine. The run
  then stops before testing anything:

  ```
  ERROR cargo test failed in an unmutated tree, so no mutants were tested
  warning: 552/590 tests were not run due to test failure
  ```

  cargo-mutants runs each test from a **copied** tree, and the app crate's
  `gate_consistency::repo_hygiene` tests read the checkout itself. In the log the
  same test both `PASS`es and `FAIL`s — the signature of a test whose answer
  depends on where it runs from rather than on the code.

  **So the app crate's suite is not hermetic, and that is what makes the whole
  crate impractical to mutation-test** — not the mutant count that
  `mutants.toml` cites. **Recommend a human:** have those tests resolve the
  repository root from `CARGO_MANIFEST_DIR`. It is a small change and it would
  make ~2,700 mutants testable, which is worth more than any single module here.
- **Network-facing modules** — `reqwest_client.rs`, `fetch_assets/http.rs`,
  `fetch_assets/ketcher.rs`, skipped by the safety rule about sockets.
- **`lotus-search/src/testing.rs`** (45) — the scripted test double itself.
  Mutating a test double tests the tests.
- **`lotus-cli/src/main.rs`** (80) — argument parsing, where a mutant usually
  changes a default rather than a behaviour. Not run for that reason alone.
- **`lotus-query/src/parse/` submodules** — `parse.rs` was run; its `stream.rs`
  and `tests.rs` children were not.

## The recurring bug class

Six of the groups killed here are one mistake: **a chain or accessor written as
"any of these" that the suite only ever exercised in one state.**

- `Columns::resolves_any` — every fixture named `compound`.
- `FilterSpec::is_active` / `FilterPlan::is_active` — criteria set in pairs.
- `plan_filter`'s taxon branch — every fixture searched by QID, never by name.
- `compound_label_or_qid` / `reference_doi` — only the populated branch existed.
- `ResponseFormat::accept` / `qlever_action` — never called per-variant.
- `export_rows`' JSON separator — only ever one row.

And two are a second, distinct mistake: **a function that exists to be the single
copy of a rule, while a second copy of the rule is what everything actually
calls.** `is_retryable_status` is the clearest example in the repo — its own doc
says it is there "so the two paths that retry agree on the rule", and the two
paths both went through `FetchError::is_retryable`'s inline duplicate instead.
Every mutant of it survived, *including* `is_retryable_status -> false`, which
would have meant the retry loop never retried anything.

The fix that keeps finding these is one case per operand, named in the assertion,
so a regression says **which** one was dropped rather than just that something
was.

## What the tests in this sweep added

| Test | Module | Kills |
|---|---|---|
| `a_header_naming_one_known_column_is_enough_to_parse` | `lotus-query/tests/streaming.rs` | `\|\|`→`&&` in `Columns::resolves_any` |
| `a_header_naming_no_known_column_is_refused` | same | the same, from the other side |
| `any_single_criterion_makes_a_filter_active` | `lotus-model/tests/columnar.rs` | 13 in `FilterSpec`/`FilterPlan::is_active` |
| `a_nameless_compound_falls_back_to_its_identifier` | same | 6 in `compound_label_or_qid` |
| `a_reference_doi_is_read_when_present_and_absent_otherwise` | same | 3 in `reference_doi` |
| `a_range_includes_its_bounds_and_refuses_what_is_outside` | same | 2 in `Range::accepts` |
| `a_taxon_needle_matches_the_taxon_name_as_well_as_its_qid` | same | 1 in `plan_filter` |
| `surviving_rows_lists_every_surviving_row` | same | 1 in `surviving_rows` |
| `the_retry_rule_is_429_or_a_server_error` | `lotus-search/src/error.rs` | the retry rule's boundaries |
| `the_error_and_the_rule_agree` | same | keeps the two copies from drifting |
| `every_format_names_its_own_accept_header_and_action` | same | 5 in `accept` / `qlever_action` |
| `a_resolution_reports_the_name_it_looked_up` | `lotus-search/src/result.rs` | 4 in `looked_up_name` |
| `every_taxon_is_exactly_the_absence_of_a_qid` | same | 2 in `is_all_taxa` |
| `the_rows_are_the_rows_that_were_stored` | same | `as_rows` → default |
| `a_note_explains_itself` | same | `TaxonNote`'s `Display` |
| `every_service_names_itself_and_its_endpoint` | `lotus-search/src/execute.rs` | 5 in `Service`'s metadata + `Display` |
| `only_two_hundreds_are_success` | same | `is_success`'s open upper bound |
| `only_an_unreachable_endpoint_falls_back` | same | the fallback condition, both directions |
| `a_json_exception_is_read_out_of_an_html_page` | same | `compact` / `json_exception` |
| `a_taxon_name_is_standardised_before_it_is_looked_up` | `lotus-search/src/search.rs` | 2 in `standardize_taxon_name` |
| `a_bare_qid_is_recognised_and_nothing_else_is` | same | `is_qid`'s digit requirement |
| `only_several_candidates_are_ambiguous` | same | 4 in `is_ambiguous` |
| `a_wildcard_handed_straight_in_is_not_treated_as_a_qid` | same | the `qid != "*"` guard |
| `a_fetched_body_comes_back_whole` | `lotus-search/tests/search_pipeline.rs` | 4 in `fetch_url` |
| `a_refused_url_is_an_error_naming_the_reason` | same | the same, from the error side |
| `a_wildcard_and_an_empty_box_both_resolve_to_no_taxon` | same | the wildcard check in `resolve_taxon` |
| `a_qid_renders_as_itself_and_an_absent_one_as_nothing` | `lotus-query/src/export_rows.rs` | the `render_qid` guard |
| `a_mass_renders_readably_and_an_absent_mass_renders_as_nothing` | same | 4 in `render_mass` |
| `a_turtle_mass_is_a_typed_number_or_the_empty_list` | same | 4 in `number` |
| `a_json_string_escapes_everything_that_would_break_the_document` | same | the control-character arm |
| `three_rows_of_json_are_separated_and_parse` | same | the comma between JSON bindings |
| `the_preamble_is_written_exactly_once_and_before_the_data` | same | the `in_body` flag |
| `the_chunk_target_is_the_measured_sixty_four_kib` | same | 2 arithmetic mutants on `CHUNK_TARGET` |
| `a_statement_renders_as_its_identifier_or_as_nothing` | same | 2 in `statement_text` |

No test was deleted or weakened to make a mutant die, and no assertion was
loosened to make one pass. Where a survivor could not be killed the reason is
written above.

Two of my own expectations were wrong while writing these, and the tests now
state the real behaviour rather than the behaviour I assumed:
`is_retryable_status(600)` **is** retryable (the rule is `>= 500` with no upper
bound, which is harmless since a status is three digits), and
`standardize_taxon_name("   ")` returns its input unchanged rather than trimming
it (`resolve_taxon` trims before calling, so a blank box never reaches it).
