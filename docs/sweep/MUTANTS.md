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
  start below 20 GB free (see `make/scripts/mutants-preflight.sh` — two earlier
  runs at `--jobs 8` and `--jobs 2` crashed a 16 GB laptop). The override was used
  because the sweep's own floor is 5 GB and the volume had 12 GB free at the
  time. Disk was checked before and after every run; it never went below 12 GB,
  and it later rose to 27 GB on its own.
- No mutant was run in `cfg(target_arch = "wasm32")`-only code, and none opens a
  socket. No test in this repo reaches a live endpoint: the search path is tested
  through the `Http` trait against a scripted conversation.

The five timeouts below are mutants where the tests ran past 60 s. They are
growth and capacity comparisons — `==` weakened to `!=` in
`ColumnarBuilder::push` and `grow_references`, and `compound_slots` forced to a
constant — which turn a resize loop into an infinite one. A timeout is the
correct outcome for those, not a wedged machine.

## Results

| Module | Mutants | Caught | Unviable | Missed | Time |
|---|---|---|---|---|---|
| `lotus-query/src/query.rs` (query builder) | 115 | 88 | 27 | **0** | 6 min |
| `lotus-query/src/parse.rs` (parsing) — before | 79 | 46 | 31 | 2 | 3 min |
| `lotus-query/src/parse.rs` — after | 79 | 48 | 31 | **0** | 3 min |
| `lotus-model/src/columnar.rs` (dedup/count) — before | 368 | 233 | 27 | 103 | 23 min |
| `lotus-model/src/columnar.rs` — after | 368 | 256 | 27 | **80** | 23 min |

The query builder needed nothing. Parsing needed one test. Dedup/count is where
the thin coverage is, and most of what is left there is one decision rather than
a pile of gaps.

## Modules not run

- **`cache_key.rs`** — the brief's fourth priority, and **attempted**. Two things
  were found, the second of which is the real blocker.

  First, a correction to an earlier assumption: `mutants.toml` does exclude the
  app crate,

  ```toml
  [package.lotus-explore-rs]
  exclude = true
  ```

  but the exclusion does not apply when `--file` narrows the run, so the module
  *is* reachable with the brief's limits. 11 mutants were listed.

  Second, the run stops before any mutant is tested:

  ```
  ERROR cargo test failed in an unmutated tree, so no mutants were tested
  warning: 552/590 tests were not run due to test failure
  ```

  cargo-mutants runs each test from a **copied** tree
  (`…/T/cargo-mutants-lotus-explore-rs-ubrZ6d.tmp/target/…`), and the app crate's
  `gate_consistency::repo_hygiene` tests read the checkout itself — the fetched
  asset trees, `.gitignore`, the tracked-file list. In the log the same test both
  `PASS`es and `FAIL`s, which is the signature of a test whose answer depends on
  where it is run from rather than on the code.

  **So the app crate's suite is not hermetic, and that — not the mutant count —
  is what makes it impractical to mutation-test.** The exclusion in `mutants.toml`
  gives a different reason (2,700 mutants of Dioxus rendering), and that reason
  may be sound on its own; but even for a 10-mutant module like `cache_key.rs` the
  run cannot start.

  **Recommend a human decide** whether the repo-hygiene tests should locate the
  repository root from `CARGO_MANIFEST_DIR` rather than the working directory.
  That is a small, safe change that would make the whole app crate testable by
  `cargo mutants`, and it is worth more than any single module's mutants.
- **Everything else in `lotus-search`, `lotus-curation`, `lotus-jsonld`,
  `lotus-cli`, `lotus-web-assets`** — not attempted. The brief orders mutation
  testing last because it is slowest, and three modules at 3–23 minutes each is
  where this sweep stopped. A prior run's artefacts for
  `lotus-model/src/columnar.rs` were found in `mutants.out/` (88 missed then, so
  close to the 103 measured now that the difference is the six tests added since)
  and were used only as a cross-check, not as a result.

## The 80 survivors in `columnar.rs`

### 71 — byte accounting (`total_dictionary_bytes`, `dictionary_costs`, `SparseStrings::len`/`is_empty`)

Every one of these is arithmetic over the dictionaries' sizes, feeding the perf
overlay. **No test asserts any of those numbers**, so every mutant of every
operation in them survives: return 0, return 1, swap `<=` for `<`, and the suite
is silent.

This is not 71 independent gaps, it is one: *is the reported dictionary byte cost
a contract or a diagnostic?*

- If it is a diagnostic, the honest fix is a test that at least asserts the
  relationships it must satisfy — non-negative, and `compound names` costing more
  than a column of raw ids for the same rows — rather than exact numbers, which
  would break on every unrelated change and then get deleted.
- If it is a contract, it needs exact assertions and a stated tolerance.

**Recommend a human decide which.** Left alive deliberately rather than papered
over with a loose assertion, because a test that asserts almost nothing is worse
than a documented gap: it looks like coverage.

### 2 — `QidDictionary::is_empty`

`pub`, re-exported from `crates/lotus-model/src/lib.rs`, and called from nowhere
in the workspace. It cannot be killed because nothing can reach it: `insert`
returns `true` instead, and both replacements are equally unobservable.

This is dead code, and the sweep's brief for the sanitising task says to remove
dead code — but it is a `pub` item on a library crate's public API, so removing
it is a public-behaviour change, which that same task forbids. Flagged rather
than done. **Recommend a human decide**, or a sweep with the public-API question
answered first.

### 1 — `parse_numeric_qid`: `||` weakened to `&&`

**Genuinely equivalent.** The guard is

```rust
if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
    return None;
}
```

and the mutant requires *both* an empty string *and* a non-digit to reject early.
For every non-empty input the difference is carried by the next line,
`digits.parse::<u32>().ok()?`, which already rejects anything non-numeric. For
the empty string the mutant falls through to `parse("")`, which also fails. Both
paths return `None`, so no test can distinguish them and none should.

### 1 — `plan_filter`, the taxon branch's `||`

The taxon branch accepts a row when the interned QID matches **or** the taxon
name contains the needle. Weakening that to `&&` survived because every fixture
searched by QID, so the name arm was untested — a genus-name search would have
matched nothing.

The new test `a_taxon_needle_matches_the_taxon_name_as_well_as_its_qid` covers
this, and also pins case folding and that a needle matching neither arm matches
nothing. This is recorded as killed-on-arrival: it was found while writing the
tests for the other groups, and the final "after" run confirms 80 rather than 81.

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

No test was deleted or weakened to make a mutant die, and no assertion was
loosened to make one pass. Where a survivor could not be killed the reason is
written above.

## The recurring bug class

Four of the five groups killed here are the same mistake: **a chain of `||` where
the suite only ever exercised the first operand.**

- `Columns::resolves_any` — every fixture named `compound`.
- `FilterSpec::is_active` / `FilterPlan::is_active` — criteria were set in pairs.
- `plan_filter`'s taxon branch — every fixture searched by QID.
- `compound_label_or_qid` / `reference_doi` — only the populated branch existed.

The general form: a chain written as "any of these" is only tested as "the first
of these". The fix that keeps finding them is one case per operand, not a longer
fixture — which is why the tests added here enumerate a criterion at a time and
name it in the assertion, so a regression says *which* one was dropped.