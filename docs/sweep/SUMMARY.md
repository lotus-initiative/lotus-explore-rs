# Morning handoff

Overnight sweep of `lotus-explore-rs` on branch **`sweep/overnight`**, forked from
`main` at `8a68da1`. `main` untouched, nothing force-pushed, no history rewritten.

**Two behaviour changes**, both in the query dispatch, both called out separately
below. Everything else is tests, docs, or one dead parameter. Nothing was
committed that was not run.

## Behaviour changes — only two, and both are query dispatch

**`d07b061` — `taxon="*"` with no structure now requires `P703`.**

It previously did not, so it returned the compounds nobody has tied to an
organism: the opposite of what `*` documents. Cause was plumbing, not a mistake
in either of the two commits involved — `resolve_taxon` resolves `"*"` to
`qid: None`, so by the time the builder ran, `*` and an empty box were the same
value. Reading `criteria.taxon` fixes every caller at once.

**`61e0383` — the HTTP API answers a blank taxon box the same way the browser
does.**

The server carried its own copy of the dispatch and had drifted. The library
resolves a wildcard to `None`; the server resolved it to `Some("*")` and so had
to treat "no QID" as a wildcard — which is also what an *empty box* produces. So
the API answered "what has been reported, and where" for a blank box while the
browser answered "what exists". That is precisely the bug commit `4ead47a` set
out to fix: fixed in the library, still live in the server. One implementation
now, and a test asserts the two front ends agree.

Both are the same underlying lesson, and it is the most important thing in this
sweep: **two implementations of one dispatch let a fix survive in one of them.**

## Refactors (no behaviour change)

| Commit | Change |
|---|---|
| `62dd958` | dropped one unused function parameter in the wasm-only download path |
| `1494ce6` | `lotus-search` dev-depends on itself so its own tests compile |
| `a0fff9f` | removed `sanitize_taxon_input` and a second `is_qid`, both duplicates |
| — | `normalized_structure_input`, a third duplicate, removed in `61e0383` |

## Bugs found

| Bug | Where | Fate |
|---|---|---|
| **Duplicated retry rule.** `is_retryable_status` was dead code whose stated purpose was to be the single copy of the rule; both retry paths called `FetchError::is_retryable`'s inline duplicate instead. Every mutant survived, including `-> false`, which would have meant never retrying. | `lotus-search/src/error.rs` | fixed, `106cd8c` |
| **Untested JSON separator.** The only test parsing the exporter's output used one row, where no comma is needed. Dropping the separator yields valid JSON for one row and invalid JSON for two. | `lotus-query/src/export_rows.rs` | fixed, `fbbcf01` |
| **Compound filter matches wrong rows.** On a two-row set, `"Q1"` returns both rows while `"Q4"` correctly returns one. Points at the filter's bit index: mask sized by dictionary length, indexed by sparse numeric QIDs. | `lotus-model/src/columnar.rs` | **written up, not fixed** |
| **`cargo test -p lotus-search` did not compile.** Its own tests import a feature nothing in its manifest enabled; it worked only because `lotus-curation` turned the feature on and workspace unification carried it. | `lotus-search/Cargo.toml` | fixed, `1494ce6` |
| **App suite is not hermetic.** `cargo mutants` cannot run on it at all: it copies the tree and the repo-hygiene tests read the checkout. | `apps/…/tests/gate_consistency.rs` | **written up** |
| **`./mk lint-wasm` red on `main`.** | `download/wasm.rs` | fixed, `62dd958` |

## Mutation testing

29 module runs, roughly 1,300 mutants. Full accounting in `MUTANTS.md`.

| | Missed |
|---|---|
| `lotus-curation` (4 modules, 122 mutants) | **0** |
| `lotus-jsonld` (4 modules, 68) | **0** |
| `lotus-cli` (2 modules, 63) | **0** |
| `lotus-web-assets` (3 modules, 52) | **0** |
| query builder (115) | **0** |
| parsing (79) | **0** |
| `export.rs` (19) | **0** |
| `error.rs` (23) | **0** |
| `result.rs` (9) | **0** |
| `columnar.rs` (368, in scope) | 28 → **3** |
| `export_rows.rs` (66) | 16 → **2** |
| `search.rs` (41) | 10 → **3** |
| `execute.rs` (72) | 24 → **17** |

Four crates needed no work at all. The 22 remaining survivors are each
accounted for: 2 dead `pub` accessors, 1 provably equivalent mutant, 2 chunk
boundaries, and 17 retry-loop mechanics where a test would have to sleep.

**The recurring bug class, six times over:** a chain or accessor written as "any
of these" that the suite only ever exercised in one state. And once, a distinct
second shape — a function that exists to be the single copy of a rule, while the
copy everyone calls is a duplicate.

## Benchmarks

Both were unmeasured before; both now have baselines.

| Path | Result |
|---|---|
| SPARQL generation | median **9.8 µs**/query; cell medians 7.5–15.9 µs; within-cell spread up to 140× the median |
| Cache-key construction | **1.8–3.1 µs**; sha256 67–73%, hex ~20% and flat, prefix 7–11% |

Neither was optimised. At 10 µs and 3 µs against a network round trip of hundreds
of milliseconds, both are ~0.001% of the work, and the noise on this host is
larger than any plausible win. Recorded so nobody spends a night proving it.

## What was found and deliberately not fixed

- **The compound-filter bit index** (`REVIEW.md` §12). Reproducible, but the fix
  needs a decision about which index space is intended and a recorded fixture with
  realistic QIDs. A wrong fix to a filter is worse than a written-up bug.
- **Two dead `pub` accessors** and **12 `pub` items unreferenced outside their own
  file** (§6, §15). Every crate is `publish = false`, so `pub` is workspace-internal
  and Rust's lint cannot see this. Deleting accessors is a judgement about future
  callers.
- **Four accessors I wrote tests for have no production caller** (§14) — including
  the tests I added while doing this. They pin real invariants, but they do not
  protect anything a reader sees, and the result should not be read as if they did.
- **No library crate's tests run standalone** (§16). `./mk test` builds the whole
  workspace, so feature unification quietly supplies what each crate needs.
- **The 71 diagnostics mutants** are out of scope *by the repo's own design*, which
  I initially mis-escalated to a decision. The repo had already answered it.

## Not measured — claimed nowhere

- **The 1271 test count in the brief does not exist at this commit.** Measured:
  1224 / 631 / 590 + 6 doctests at baseline. The gate enforced instead was "no
  count below the recorded baseline, per build".
- **No peak RSS or allocation count.** `dhat` is not installed; only the *retained*
  size of the built set. Every memory number says "retained", not "peak".
- **No crate-level wasm attribution.** `twiggy`'s rows are all anonymous because the
  module is `debug = false`, and `cargo bloat` is not installed.
- **No row counts for the twelve matrix cells.** Nothing was run against a live
  endpoint, deliberately — that work was scoped and is waiting on your nod.
- **The matrix tests check structure, not grammar.** No real SPARQL parse; that
  needs `spareval`, a dependency decision.

## Final gate status

| Gate | Result |
|---|---|
| `./mk fmt-check` | pass |
| `./mk lint` (native, `--all-features`) | pass |
| `./mk lint-wasm` (wasm32) | pass — **was red at `8a68da1`** |
| `./mk test` | **1265 web / 634 server / 590 desktop / 6 doctests, 0 failed** |
| `doc_links` | pass |
| `docs_in_sync` (5 tests) | pass |

Tests went **up** across the whole sweep: 1224 → 1265 web, 631 → 634 server. No
test was deleted, weakened or edited to make anything pass.

## Reproducing every number

```bash
# Gates, in order
./mk fmt-check && ./mk lint && ./mk lint-wasm && ./mk test

# WASM size
./mk web-build
out=target/dx/lotus-explore-rs/release/web/public
w=$(ls $out/assets/*_bg-*.wasm)
stat -f%z "$w"; gzip -c9 "$w" | wc -c; stat -f%z "$w.br"
wasm-opt -Oz -o /tmp/base_oz.wasm "$w" && stat -f%z /tmp/base_oz.wasm
twiggy top -n 20 "$w"

# Compile time
CARGO_TARGET_DIR=/tmp/sweep-clean cargo build --workspace --locked --timings
cargo build --workspace --all-targets --locked

# Parsing, aggregation, retained memory
cargo test -p lotus-query --release --locked -- --ignored --nocapture bench

# SPARQL generation, 30 samples, median + spread
cargo test -p lotus-search --release --test matrix -- --ignored --nocapture bench_matrix

# Cache-key construction, 50 samples, median + spread
cargo test -p lotus-explore-rs --release --bin lotus-explore-rs -- --ignored --nocapture bench_cache_key

# The matrix table, regenerated
cargo test -p lotus-search --test matrix every_cell_is_well_formed -- --nocapture

# Mutation testing, one module at a time (see MUTANTS.md for the full list)
MUTANTS_MIN_FREE_GB=0 nice -n 19 cargo mutants --test-tool nextest \
  -p lotus-query -f crates/lotus-query/src/parse.rs \
  -j 1 --timeout 60 --minimum-test-timeout 20 --output /tmp/mut
```

## Two things about this machine worth knowing

1. **Load sat between 6 and 42 on 8 cores**, attributed by `ps` to five
   `Metadata.framework` (Spotlight) processes indexing `target/` as the builds
   wrote it. Absolute timings move by ~1.7× between runs for that reason alone,
   which is why every benchmark reports a median and its spread. Adding
   `target/` and `graphify-out/` to Spotlight's privacy list is **the single
   highest-value thing you could do** for future measurement work. I did not
   change machine configuration unattended.
2. **`target/debug` was deleted from under the first session**, which made its
   first test run fail with a non-existent test binary — not a code defect. The
   volume is a desktop's and ran at 94–98% full. It recovered and ended at 52 GB
   free.

## Deviations from the brief, all deliberate and recorded

- `ionice -c 3` **does not exist on macOS**; heavy runs used `nice -n 19` only.
  `ulimit -v` was applied nowhere rather than wrongly — capping virtual memory
  breaks rustc spuriously.
- Gates are the repo's `./mk` tasks, which are stricter than the brief's commands
  (`--all-features` on clippy; `server` and `desktop` linted and tested
  separately), so they subsume them.
- **No criterion benches.** The repo has one hand-rolled harness that prints and
  asserts nothing, with a documented reason to exist. The gap is recorded; the
  two benches added do take 30 and 50 samples with medians.
- **Task C found almost nothing to clean,** and saying so is more useful than
  manufacturing churn: zero TODO/FIXME markers, zero commented-out code, every
  one of the 58 `#[allow]` sites carries a documented reason, no unused
  dependencies, and every `cfg` already includes `test`. What it *did* find was
  the four duplicated helpers and the drifted dispatch — which is exactly what
  the brief was looking for, and which clippy, `udeps` and `machete` all report
  as clean.