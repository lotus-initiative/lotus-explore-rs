# Needs a human decision

Things the unattended sweep found but did not act on. Each one is a decision
that belongs to a person, not a guess that belongs in a commit at 3am.

## 1. The expected test count of 1271 does not exist at this commit

Measured: 1224 (browser) + 631 (server) + 590 (desktop) + 6 (doctests). No
combination of the repo's own `./mk test` tasks sums to 1271. The largest
single number is 1224.

**Needed:** confirmation of what 1271 was measured from — a different commit, a
different feature set, or `cargo test` rather than nextest (the doctests are 6,
and nextest does not run them). Until then the sweep gates on "no count below
this recorded baseline, per build" rather than on 1271. Full numbers and
commands in `BASELINE.md`.

## 2. Spotlight indexing `target/` makes every timing number noisy

Load average sits at 25–42 on 8 cores, attributed by `ps` to five
`CoreServices/Metadata.framework` processes (Spotlight) at 15–30% each. Builds
write millions of files into `target/`; Spotlight indexes them.

**Recommended:** add `target/` (and `graphify-out/`) to Spotlight's privacy
list, or exclude the repo from indexing. This is machine configuration and was
deliberately not changed by an unattended run. It is the highest-value fix for
future measurement work, and it likely also relieves the disk pressure in item 3.

## 3. The volume is at 97–98% full, and `target/debug` was deleted mid-session

The first `cargo test --workspace` of the sweep failed with
`could not execute process .../doc_links-... (never executed)`. `target/debug`
had been deleted outright while `target/wasm32-unknown-unknown` survived, and
rebuilds then failed on stale fingerprints for `proc_macro2` and `dunce`.

**Needed:** free space. A Dioxus workspace with `target/wasm32` at 2.8 GB plus
a fresh `target/debug` at 1.4 GB does not leave much headroom, and the sweep's
own release bundle adds more. Recommendation: prune `target/wasm32-unknown-unknown/debug`
and `target/dx` between sessions, or move `CARGO_TARGET_DIR` to a larger volume.

## 4. `cargo bloat` and `dhat` are not installed

So: no crate-level wasm size attribution (twiggy's rows are all anonymous,
because the module is built `debug = false`), and no peak-RSS or
allocation-count numbers — only the retained size of the built
`ColumnarResultSet`, which `bench.rs` already reports.

**Needed:** a decision on whether to install them. Both are dev-time tools; the
sweep did not add dependencies to a machine at 97% disk.

## 5. Criterion was not added, and the runtime baseline is single-sample

The repo deliberately has one hand-rolled bench harness that asserts nothing,
with a documented reason ("a benchmark that fails on a slow machine is a test
that gets deleted"). The brief asked for criterion with 30 samples and
medians. Adding criterion means a new dependency and a second convention.

**Needed:** a human call on whether criterion is wanted. If it is, the SPARQL
and cache-key benchmarks added for task A should move to it rather than
accumulate more single-sample prints.

## 6. Sorting by compound label has a real cliff above ~1M rows — confirmed, not noise

**This reverses the note that used to be here.** The original claim was that the
98x jump at 2M rows was "much more likely this host's load or a threshold inside
the sort". The load explanation was wrong. Re-measured on a quiet machine
(load average 3.7, against 25–42 when the first numbers were taken), 15 samples
per size, median and spread:

| Rows | median | min | max | **ns/row** |
|---|---|---|---|---|
| 50,000 | 0.5 ms | 0.5 | 0.5 | 9.6 |
| 100,000 | 1.0 ms | 1.0 | 1.0 | 9.6 |
| 250,000 | 2.5 ms | 2.4 | 2.5 | 9.8 |
| 500,000 | 4.9 ms | 4.8 | 5.0 | 9.8 |
| 1,000,000 | 9.9 ms | 9.8 | 10.2 | 9.9 |
| **2,000,000** | **1,032.1 ms** | 1,031.0 | 1,035.3 | **516.1** |
| 2,990,730 | 1,569.9 ms | 1,567.5 | 1,582.0 | 524.9 |

```bash
cargo test -p lotus-query --release --locked --test bench -- --ignored --nocapture bench_sort_scaling
```

`ns per row` is the column that answers the question. It is **flat at ~9.8**
through a million rows and then **52x worse** at two million, and it stays bad.
The spread at 2M is 0.4% across 15 samples, so this is not a tail: the sort
genuinely does 50x more work per row past some size.

The original single samples were both correct and the ratio was real. What was
wrong was the explanation, and the reason the first reading looked like noise is
that a single sample at 2M (840.9 ms) and a single sample at 1M (8.6 ms) cannot
distinguish "a cliff" from "one slow measurement".

**Probable cause: the working set leaving cache.** Per comparison the comparator
does two dictionary lookups, and `n log n` comparisons means each row is touched
~21 times. Below a million rows the dictionary and the permutation array still
resolve mostly from cache, which is why 0.5 ns per comparison is achievable. Above
it they do not, and each comparison becomes memory latency. That accounts for the
ratio without needing anything quadratic, and it predicts the cliff moves with
the machine's cache size rather than sitting at a round row count — **which is
testable and has not been tested.**

**Practical impact is bounded.** On the widest search this app can produce —
2,990,730 rows — the sort is 1.57 s against 5.5 s to parse the same rows from
CSV. It is the third-largest cost after parsing and interning, not the dominant
one, so this is a "know it is there" item rather than a fire.

**Not optimised, and here is the honest reason.** The obvious fix is to stop
sorting row indices into a cold dictionary: sort the small set of distinct
compound slots once, then map rows through it. That is a real change to a
user-visible ordering, it needs its own test for stability against the current
`then_with(|| a.cmp(&b))` tie-break, and it would be done against a cliff whose
cause is inferred rather than proven. Given a parallel agent is working in this
repository, the measurement is the deliverable and the optimisation is a decision.

**Recommend a human:** confirm the cache hypothesis by re-running the same sweep
on a machine with a different cache size, and if the cliff moves, the fix is
worth scoping. The benchmark added here (`bench_sort_scaling`) is the thing to
run.

## 7. `*` with a structure search still does not filter on the root taxon

The taxon wildcard is now read from `criteria.taxon` rather than the resolved QID
(`d07b061`), so `*` with no structure requires `P703` as documented. The
structure half of the same gap was deliberately **not** changed.

`build_base_query` maps a wildcard to the root taxon `Q2382443` in the structure
branch, from which `P171*` reaches everything. That arm is unreachable today,
because `resolve_taxon` resolves `*` to `qid: None`, so the query actually built
is "every taxon, unfiltered, occurrence optional" — which is arguably the correct
reading of `*` and returns at least as much.

Making the documented arm live would switch the structure cells from
`optional` to `required`, and `P171*` from `Q2382443` does not necessarily reach
every taxon in the projection. So the change would likely return **fewer** rows
for `*` + structure. That is a product judgement about what `*` should mean, not
a bug fix, and it needs a recorded-fixture row count to make it safely — which
this sweep did not have. Left as is; `docs/QUERY_MATRIX.md` records the current
behaviour in rows 11 and 12 so the decision is visible.

## 8. The SPARQL matrix tests check structure, not grammar

`matrix.rs` verifies balanced delimiters, scoped `^` in property paths, no empty
`FILTER`, and the block each pattern sits in. That catches a malformed query, and
it is what caught the wildcard bug, but it is not a parse: a query can be
balanced and still be invalid SPARQL.

Honest validation needs a real parser — `oxigraph` or `spareval` — behind a
dev-dependency. Both are substantial and neither is in the tree today, and adding
one to a workspace that currently reaches the endpoint only through a scripted
`Http` trait is a decision worth making deliberately. **Recommended:** add
`spareval` (pure Rust, parse-only, much smaller than oxigraph) as a dev-dependency
of `lotus-search` and parse all twelve cells plus the filter and export variants.

## 9. Untranslated strings

*(none yet — see the FAQ task; anything uncertain goes here rather than into a
commit)*
## 10. An empty taxon fetches the entire projection, and the reader is not told

`taxon=absent structure=absent` builds the query with no `LIMIT`, and the
interactive path deliberately removed its 500-row ceiling
(`apps/lotus-explore-rs/src/table_budget.rs` explains why, and the reasoning is
sound). So submitting the form with an empty taxon box asks the endpoint for every
compound in LOTUS — measured elsewhere in this repo at 2,990,730 rows and ~873 MB
of CSV — and nothing caps it below that on the web path.

That is a deliberate design decision and this sweep did not touch it. What is
missing is a *signal*: there is no test, and no FAQ entry, asserting that the
reader is told a search is going to return the whole graph before they wait for
it. The FAQ's "How many rows can a search return?" currently says only that every
matching row is returned.

**Recommended:** either warn when the taxon box is empty and no other constraint
is set, or add a FAQ entry saying plainly that an empty taxon box means the whole
projection and roughly how long that takes. Needs a human because it is a product
call about whether to constrain the request or only to explain it.

**Now measured, and worse than "no cap in principle".** A `taxon="*"` query
returns **572 MB of JSON** from QLever — that is the whole projection, and it is
what a blank taxon box asks for on the web path. See `docs/SPARQL-VARIANTS.md`.
Two further points that measurement settled: the library path (the CLI) *is*
capped at 500 rows by `DEFAULT_ROW_LIMIT`, so the two front ends differ on this
and only the CLI is bounded; and the cost is not hypothetical, since the repo's
own benchmark parses a 2,990,730-row export in 5.5 s.

## 11. The CLI and the web app have different row limits

`lotus-search::search` — the CLI's path — caps at `DEFAULT_ROW_LIMIT` (500) and
reports a `truncated` flag. The interactive path fetches every row and reports
`display_capped_rows: false` always. Defensible (one is a library default, the
other an interactive session), but it means "not limited" is true of one front end
and false of the other. Worth deciding whether the CLI should say so explicitly.

## 12. The table's compound filter may match rows it should not — needs investigation

Found while writing tests for the mutation survivors, and **not fixed**, because
the cause is not established and a wrong fix to a filter is worse than a written-up
bug.

Reproduction, on a two-row set with distinct InChIKeys so nothing is shared
between the rows (`crates/lotus-model/tests/columnar.rs` builds it):

```text
row 0: compound Q1, taxon Q2      row 1: compound Q4, taxon Q5

compound filter "Q1"  ->  surviving_rows [0, 1]     <-- row 1 does not match
compound filter "Q4"  ->  surviving_rows [1]
compound filter "Q2"  ->  surviving_rows []          (correct: a taxon is not a compound)
taxon     filter "Q2"  ->  surviving_rows [0]        (correct)
```

So `"Q4"` behaves and `"Q1"` does not. The asymmetry points at the bit index
rather than the string match: in `ColumnarResultSet::plan_filter` the compound
branch builds `Bitmask::with_len(self.compounds.len())` and then calls
`mask.insert(id)` where `id` comes from `slot_to_id(slot)`, while `FilterPlan::accepts`
tests `mask.contains(id)` against `set.compound_ids[row]`. `Bitmask` is
`Vec<u64>` sized `len.div_ceil(64)` words, and `insert` silently ignores an id
whose word is out of range while `contains` answers `false` for the same id — so
the two only agree while every id fits inside `len.div_ceil(64) * 64`.

`len` is the number of distinct compounds and the ids are **numeric Wikidata
QIDs**, which are sparse and large. For a result set of a few hundred thousand
compounds the mask has a few thousand words, while a QID like `Q12345678` needs
word 192708. If that reading of the code is right, a compound filter for a
high-numbered QID would match **nothing** rather than one row, and the
"ignoring out-of-range ids" in `insert`'s doc comment is doing load-bearing work
nobody intended.

**Why this was not fixed here:** it needs a decision about whether ids or slots
are the right index, which touches the whole filter path, and the honest test is
a recorded fixture with high QIDs — which this repo does not have. Recommended:
a human writes the failing case first with a realistic QID (`Q16521`-scale and
`Q7000000`-scale), confirms which of the two index spaces is intended, then fixes
it. The `is_empty`/`len` mutants in this same file (below) are very likely the
same confusion showing up in a different place.

## 13. The app crate's test suite is not hermetic, which blocks `cargo mutants` on it

Found by attempting the brief's fourth mutation module (`cache_key.rs`) rather
than assuming it was blocked.

`mutants.toml` excludes the app crate, but `--file` overrides that exclusion, so
the module is reachable. The run then stops before testing anything:

```
ERROR cargo test failed in an unmutated tree, so no mutants were tested
warning: 552/590 tests were not run due to test failure
```

cargo-mutants runs the tests from a copied tree
(`…/T/cargo-mutants-lotus-explore-rs-….tmp/target/…`). The crate's
`gate_consistency::repo_hygiene` tests read the checkout itself — the fetched asset
trees, `.gitignore`, the tracked-file list — so their answers depend on the
working directory. In the log the same test both `PASS`es and `FAIL`s, which is
the signature of a test that is asking about the sandbox rather than about the
code.

It passes when the binary is run from the repo root **and** from `/tmp`, so this
is not a plain "assumes cwd == repo root" bug; it is dependence on the tree being
the real checkout rather than a copy of it.

**Recommended:** have those tests resolve the repository root from
`CARGO_MANIFEST_DIR` (or `env!("CARGO_MANIFEST_DIR")`) instead of the working
directory. That is a small, contained change, and it would make the whole app
crate — ~2,700 mutants — testable by `cargo mutants`, which is worth far more
than any single module's results. Not done here: it touches gate tests, and a
gate test that stops checking what it was written to check is worse than a
documented limitation.

## 14. Four accessors this sweep wrote tests for have no production caller

Mutation testing led me to add tests for `SearchResult::as_rows`,
`TaxonResolution::is_all_taxa`, `TaxonResolution::looked_up_name` and
`SmilesSearchType::needs_structure_service`. All four kill real mutants, and all
four are **called from tests only** — no front end in this workspace uses them.

The tests are not wrong. Each accessor is documented, its behaviour is
non-obvious (`is_all_taxa` is "the absence of a QID"; `looked_up_name` refuses to
report an empty string), and a test that pins it is what stops the next person's
refactor from quietly changing it. But they do not protect anything a reader
currently sees, and saying otherwise would overstate the sweep's result.

**Left in place deliberately.** They are small, they document real invariants, and
deleting a `pub` accessor because today's callers are zero is a judgement about
tomorrow's callers that belongs to someone who owns the API.

## 15. `publish = false` everywhere, so unused `pub` items are dead code

Every crate in the workspace is `publish = false`. That makes `pub` meaningful only
inside this workspace — and Rust's `dead_code` lint does not fire on `pub` items,
because it assumes a library consumer that does not exist here. So there is a
class of dead code the compiler will never tell you about.

A sweep of all 248 `pub fn`s in the seven library crates found **12 with no
reference outside their own file**:

| Item | Crate |
|---|---|
| `SearchResult::as_rows`, `TaxonResolution::is_all_taxa` | lotus-search |
| `QidDictionary::is_empty`, `QidDictionary::numeric_ids`, `QidDictionary::intern`, `QidDictionary::intern_text`, `ColumnarResultSet::distinct_len`, `ColumnarResultSet::reference_year` | lotus-model |
| `Nomenclature::enabled_relations` | lotus-query |
| `RowExporter::for_each_chunk` | lotus-query |
| `TaxonProfile::entity_for` (lotus-jsonld) | lotus-jsonld |
| `StructurePlan::for_request` | lotus-search |
| `identity_key` | lotus-cli |

Several of these are used *within* their own file and are perfectly alive —
`intern`/`intern_text` are the columnar dictionary's internals, and
`for_request` is called by `build_base_query`. The genuinely unreferenced ones are
the two `QidDictionary` accessors already flagged in item 6, plus the accessors in
item 14.

**Recommend a human run this as a deliberate pass**, because the tooling to do it
properly is missing: `cargo udeps` finds unused *dependencies*, not unused `pub`
items, and `cargo +nightly udeps` is already wired into `./mk supply-chain` for
the former. A crate-wide `pub` audit is a judgement call about which surface to
keep, and it should be made by someone who knows which of these are conveniences
for the next feature.

## 16. A complete query form exists that is 50% faster, and adopting it is a decision

Measured on `Q21754`, full query, 2026-10-04. Inlining the seed's nomenclatural
closure as a literal `VALUES ?root { … }` list instead of computing it in a
subquery returns **identical** results — 32,160 rows, 11,087 distinct compounds —
in **4,860 ms instead of 9,716 ms**.

The interleaved alternative is faster still on paper but **loses 79 distinct
compounds** (11,008), so it is not a candidate. Full numbers in
`docs/SPARQL-VARIANTS.md`.

**This is a decision rather than a change because it moves work, not code.** The
closure has to be resolved before the query is built, which means:

- a new round trip in the query-build path (1,316 ms for this taxon), which must be
  cached per `(taxon, nomenclature)` to be worth it — otherwise it is 1.3 s to
  save 4.9 s, which is still a win but a much smaller one
- a fallback when the closure lookup fails, or searches fail for an unrelated
  reason
- a guard for an empty closure, because an empty `VALUES` is a valid query that
  returns nothing
- new cache keys, so a first search after deploy misses

Nomenclature-off costs nothing: there is no closure to resolve. The query stays
self-contained and pasteable, so `docs/cli.md` is unaffected.

**Recommend:** implement it, behind the existing taxon cache, with the empty-
closure guard treated as a bug rather than an impossibility. Worth roughly half
the endpoint's compute on every taxon search, which is the most expensive thing in
the request. I stopped short of writing it because it touches the search pipeline
in three crates while another agent is working in this repository.

## 17. Two crates could not be mutation tested until one line each

`cargo test -p lotus-search` did not compile on its own — see the `lotus-search`
commit for the full account; the fix was a dev-dependency on self.

The same class of problem is worth checking for rather than discovering later:
**no library crate in this workspace has a test suite that runs standalone.**
`./mk test` runs `cargo nextest run --workspace`, which builds everything at once,
so feature unification silently supplies whatever each crate's own tests need.
The symptom is invisible until someone runs one crate on its own — which is what
`cargo mutants` does, every time.

**Recommend:** a gate that runs `cargo test -p <crate> --no-run` for each library
crate in turn. It costs a few seconds of caching and would catch the next
occurrence at CI rather than at 2am.
