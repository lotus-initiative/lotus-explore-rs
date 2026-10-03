# Morning handoff

Overnight sweep of `lotus-explore-rs`, run on branch **`sweep/overnight`**, forked
from `main` at `8a68da1`. `main` was not touched, nothing was force-pushed, and no
history was rewritten.

**One behaviour change** (`d07b061`, below). Everything else is tests, docs, or a
dead parameter. Nothing was committed that I did not run.

## Commits

| Commit | Item | Effect | Before → after |
|---|---|---|---|
| `62dd958` | C — wasm gate | `open_sink` no longer takes a `format` it never read | `./mk lint-wasm` **red → green**; no test edits |
| `888d34e` | baseline | BASELINE.md, REVIEW.md written before anything else | — |
| `d07b061` | **A — query matrix, `*` fix** | **BEHAVIOUR CHANGE**: `taxon="*"` requires `P703` | 12 cells pinned; tests 1224 → 1231 web |
| `7beb904` | B — SPARQL generation bench | first ever measurement of the query builder | none → median **9.8 µs**/query, cell medians 7.5–15.9 µs, 30 samples |
| `bca6b1a` | A/D — doc correction | my own claim about the empty-taxon bound was wrong | — |
| `68abea2` | D — FAQ | 3 matrix cases answered, ×4 locales | 12 → 15 entries |
| `116edac` | E — parsing mutants | `\|\|`→`&&` in `Columns::resolves_any` killed | missed **2 → 0** (79 mutants) |
| `7400fc1` | E — dedup/count mutants | 6 tests; found one real bug | missed **103 → 80** (368 mutants) |
| `513e77c` | E — MUTANTS.md | every survivor accounted for | — |
| `a8f88c5` | E — cache-key attempt | corrected my own wrong blocker claim | — |

## Behaviour changes — only one, and it is the one task A allows

**`d07b061`: `taxon="*"` with no structure now requires `P703`.**

It previously did not, and so returned the compounds nobody has tied to an
organism — the opposite of what `*` documents and the opposite of what commit
`4ead47a` set out to fix.

The cause was plumbing, not a mistake in either commit. `resolve_taxon` resolves
`"*"` to `qid: None`, because a wildcard names no Wikidata entity. By the time
`build_base_query` runs, `*` and an empty box are the same value, and the builder
was reading the distinction off the QID — so its `Some(_)` arm was unreachable
through `search()`, `search_with_limit`, the app's handler, and the CLI. Reading
`criteria.taxon` fixes every caller at once.

**Not changed, on purpose:** the same gap in the *structure* branch. Routing `*`
through the root taxon `Q2382443` would require an occurrence and so could return
**fewer** compounds than "every taxon" means. That is a product judgement about
result size, not a bug fix. See `REVIEW.md` §7.

## Refactors (no behaviour change)

- `62dd958` — dropped one unused function parameter. Native builds compile a
  different `download` module, which is why only the wasm lint saw it.

## Tried, measured, reverted or not done

- **`wasm-opt -Oz`** on the release bundle: **−297 B, −0.02%.** `dx` already runs
  an equivalent pipeline, so there is no free win. Not adopted.
- **Micro-optimising the query builder**: not attempted, and now demonstrably not
  worth attempting. Generation costs ~10 µs against a network round trip of
  hundreds of milliseconds, and the within-cell spread on this host (up to 140× the
  median) is larger than the difference *between* cells. Recorded so nobody spends
  a night proving it.
- **criterion benches**: not added. The repo has one hand-rolled harness that
  prints and asserts nothing, with a documented reason ("a benchmark that fails on
  a slow machine is a test that gets deleted"). Adding criterion means a new
  dependency and a second convention. The gap it leaves — single samples — is
  recorded rather than papered over; the new generation bench does take 30 samples
  with median and spread.
- **Clean build timing** used a throwaway `CARGO_TARGET_DIR`, deleted immediately
  (1.0 GB of a volume that had 13 GB free). No `cargo clean` was ever run against
  the main `target/`.

## What was found and not fixed

- **A compound filter that matches the wrong rows** (`REVIEW.md` §12). On a
  two-row set, compound filter `"Q1"` returns both rows while `"Q4"` correctly
  returns one. It points at the filter's bit index: the mask is sized by dictionary
  length and indexed by numeric QID, which are sparse and large. Reproducible, but
  fixing it needs a decision about which index space is intended and a recorded
  fixture with realistic QIDs — neither of which this sweep had. A wrong fix to a
  filter is worse than a written-up bug.
- **The app crate's suite is not hermetic** (`REVIEW.md` §13). `cargo mutants`
  cannot run on it at all: it copies the tree, and `gate_consistency::repo_hygiene`
  reads the checkout, so the unmutated baseline fails. Making those tests resolve
  the root from `CARGO_MANIFEST_DIR` would make ~2,700 mutants testable — worth
  more than any module's results.
- **`QidDictionary::is_empty`** is `pub`, exported, and called from nowhere. Dead
  code, but removing a `pub` item is a public-behaviour change, which task C
  forbids.
- **71 surviving mutants** in `columnar.rs`'s byte accounting. One decision, not 71
  gaps: is the reported dictionary byte cost a contract or a diagnostic? Needs a
  human; a test asserting almost nothing would look like coverage without being it.
- **1 surviving mutant** in `parse_numeric_qid` is genuinely equivalent (argued in
  `MUTANTS.md`).
- An **empty taxon box fetches the entire projection** (2,990,730 rows, ~873 MB of
  CSV) with no cap, by deliberate design. What is missing is a signal to the
  reader, not a cap. `REVIEW.md` §10.

## Not measured — claimed nowhere

- **The 1271 test count in the brief does not exist at this commit.** Measured:
  1224 web / 631 server / 590 desktop / 6 doctests. The gate enforced instead was
  "no count below the recorded baseline, per build". Needs a human to say what
  1271 was measured from.
- **No crate-level wasm attribution.** The module is built `debug = false`, so all
  20 `twiggy top` rows are anonymous, and `cargo bloat` is not installed.
- **No peak RSS or allocation count.** `dhat` is not installed; only the retained
  size of the built `ColumnarResultSet` was measured, and every memory number in
  `BASELINE.md` says "retained" rather than "peak".
- **No row counts for the twelve matrix cells.** Nothing was run against a live
  endpoint, deliberately.
- **The matrix tests check structure, not grammar.** Balanced delimiters and scoped
  `^` catch malformed queries; a real parse needs `spareval` or `oxigraph`, which
  is a dependency decision (`REVIEW.md` §8).

## Final gate status

All green at `a8f88c5`:

| Gate | Result |
|---|---|
| `./mk fmt-check` | pass |
| `./mk lint` (native, `--all-features`) | pass |
| `./mk lint-wasm` (wasm32) | pass — was red at `8a68da1` |
| `./mk test` | **1239 web / 631 server / 590 desktop / 6 doctests, 0 failed** |
| `docs_in_sync` (5 tests) | pass |
| wasm test target compiles | pass |

Test counts went **up** across the whole sweep (1224 → 1239 web). No test was
deleted, weakened, or edited to make anything pass.

## Reproducing every number

```bash
# Gates, in the order the brief gives them
./mk fmt-check
./mk lint
./mk lint-wasm
./mk test

# WASM size (BASELINE.md)
./mk web-build
out=target/dx/lotus-explore-rs/release/web/public
w=$(ls $out/assets/*_bg-*.wasm)
stat -f%z "$w"; gzip -c9 "$w" | wc -c; stat -f%z "$w.br"
wasm-opt -Oz -o /tmp/base_oz.wasm "$w" && stat -f%z /tmp/base_oz.wasm
twiggy top -n 20 "$w"

# Compile time (BASELINE.md)
CARGO_TARGET_DIR=/tmp/sweep-clean cargo build --workspace --locked --timings
cargo build --workspace --all-targets --locked

# Parsing, aggregation, retained memory (BASELINE.md)
cargo test -p lotus-query --release --locked -- --ignored --nocapture bench

# SPARQL generation across the twelve cells, 30 samples, median + spread
cargo test -p lotus-search --release --test matrix -- --ignored --nocapture bench_matrix

# The matrix table itself, regenerated
cargo test -p lotus-search --test matrix every_cell_is_well_formed -- --nocapture

# Mutation testing, one module at a time (MUTANTS.md)
MUTANTS_MIN_FREE_GB=0 nice -n 19 cargo mutants --test-tool nextest \
  -p lotus-query -f crates/lotus-query/src/query.rs \
  -j 1 --timeout 60 --minimum-test-timeout 20 --output /tmp/mut-query
# ...and the same with -f crates/lotus-query/src/parse.rs
# ...and -p lotus-model -f crates/lotus-model/src/columnar.rs
```

## Two things about this machine worth knowing

1. **Load average was 25–42 on 8 cores for most of the session**, and it was not
   the build: `ps` attributed it to five `Metadata.framework` (Spotlight)
   processes indexing `target/` as the builds wrote it. Every timing number here is
   a single sample or a median with its spread recorded, and sub-20% wall-clock
   deltas should be read as noise. Adding `target/` and `graphify-out/` to
   Spotlight's privacy list is the fix and is **the single highest-value thing a
   human could do** for future measurement work. I did not change machine
   configuration unattended.
2. **`target/debug` was deleted from under the session early on**, which made the
   first `cargo test` fail with a non-existent test binary — not a code defect. The
   volume is a desktop's, at 94–98% full. It recovered on its own and ended at
   24 GB free, but it is why the first baseline run has a note about a rebuild.

## Deviations from the brief, all deliberate and all recorded

- `ionice -c 3` **does not exist on macOS**; heavy runs used `nice -n 19` only.
  `ulimit -v` was applied nowhere rather than wrongly: capping virtual memory
  breaks rustc spuriously.
- Gates are the repo's `./mk` tasks, not the brief's commands — they are stricter
  (`--all-features` on clippy; `server` and `desktop` linted and tested
  separately), so they subsume the brief's. `./mk` exists because `cargo make` with
  workspace support re-runs every task eight times.
- Task C produced almost nothing to do, and I am reporting that rather than
  manufacturing churn: **zero** TODO/FIXME/XXX/HACK markers in `crates/*/src` and
  `apps/*/src`, and every one of the 58 `#[allow(...)]` sites carries a documented
  reason. The only dead code found is `QidDictionary::is_empty`, which cannot be
  removed without a public-API decision.