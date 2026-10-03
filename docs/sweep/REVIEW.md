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

## 6. `sort by name` at 2M rows is 98x slower than at 1M rows

8.6 ms at 1,000,000 rows, 840.9 ms at 2,000,000 rows. That is not a plausible
property of a sort and is much more likely this host's load or a threshold
inside the sort. Flagged rather than optimised: it needs a re-measurement on a
quiet machine before anyone treats it as a real cliff.

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
