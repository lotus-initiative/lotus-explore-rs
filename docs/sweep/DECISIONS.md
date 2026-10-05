# Decisions: what needs you, in order

One page. `REVIEW.md` has the evidence and the reasoning for all seventeen items;
this is the checklist, ordered by what it costs to leave alone.

**Read this file first.** Then `SUMMARY.md` for what was done, `MUTANTS.md` for
the mutation results, `BASELINE.md` for the measurements.

Legend: **FIX** = a defect I did not fix and you should want fixed.
**DECIDE** = a judgement call only you can make. **DO** = an action only you can
take. **DONE** = resolved since it was written.

---

## Round two: four defects found by auditing shapes against each other

Written after `aaa7333`. These were not in the original seventeen; they came out
of comparing generated queries to each other and to the endpoint, and every one of
them is invisible to a row count.

### 3. ~~FIX — every reference title, DOI and year was blank~~ **DONE** · `6cf36ed`

> `FILTER(BOUND(?ref))` sat **inside** three `OPTIONAL`s. A `FILTER` in an
> `OPTIONAL` is evaluated against *that* OPTIONAL's own solutions, before the join
> with the left side, so `?ref` — bound only outside — was unbound inside, the
> guard was always false, and all three columns were always empty.

Measured on `Q153`, counting rows that actually receive a title: **87** without
the guard, **0** with it, **0** when guarding a variable bound nowhere (the proof
that it is scoping and not the predicate). Replaced with a sentinel,
`BIND(COALESCE(?r, wd:Q0) AS ?_ref_source)`, which keeps the 54,116,026-row scan
fix that motivated the guard.

It survived because an empty metadata cell is indistinguishable from a reference
that genuinely has none, and the commit that introduced it verified by row count —
which did not move.

### 4. ~~FIX — `?ref` deleted from the projection~~ **DONE** · `426801e`, `24f25bf`

> Dropped as "read by nothing". It is the reference **node**
> (`prov:wasDerivedFrom`), not the publication `?ref_qid` names via `pr:P248`, and
> the set-based Turtle exporter needs it. It cannot be integer-cast: it is
> `http://www.wikidata.org/reference/<64 hex>`, and QLever reports that cast error
> as an unbound cell rather than a failed query.

Two exporters exist and only one was ever measured. See `docs/SPARQL-VARIANTS.md`.

### 5. ~~FIX — the Turtle put `prov:wasDerivedFrom` on the compound~~ **DONE** · `24f25bf`

> It emitted `wd:Q103815959 prov:wasDerivedFrom wd:Q35589126 .` — the compound as
> subject, no statement, no reference node. Two occurrences of one compound through
> two references became indistinguishable triples. Now emits the full chain.

### 6. ~~FIX — the carbon counter read the last `|C`~~ **DONE** · `dd101c5`

> Greedy match took the **last** `|C` token, so `C6H5COOH` counted 1 carbon.
> 1 misparse in 3,301 formulas on a real taxon, 0 membership flips. Prefers a
> digit-bearing token now.

### 7. ~~FIX — genes returned as metabolites~~ **DONE** · `2e0945b`

> `P703` is not specific to metabolites. `Q21629845`, a protein-coding gene, came
> back in a taxon search beside the metabolites with nothing in the row to say
> which it was. The optional-occurrence and compound-seeded shapes had no
> compound test; `P235`, the InChIKey, is the discriminator and the `taxon`/`star`
> shapes already required it.

Nothing lost: `Q21754` returns 27,952 rows over 11,087 compounds before and after,
and every row now carries an InChIKey — which also fixed the null SMILES on the
exact route.

### 8. ~~FIX — a taxon on the exact-structure route was not applied~~ **DONE** · `ebce36f`

> `--structure Q23118 --structure-search exact --taxon Gentiana` returned Q23118
> with an empty taxon cell. The taxon sat inside the occurrence `OPTIONAL`, so it
> decided only which occurrences were *shown*; `VALUES ?c` above it was
> unconditional. A compound reported only in some other taxon came back still
> listing that other taxon.

### 9. ~~FIX — the year filter returned nothing for a narrow range~~ **DONE** · `1ff4779`

> `--taxon Q21754 --year-min 2020 --year-max 2026` returned 0 rows over 452
> occurrences. The filter reached the date through a *required* `?r wdt:P577` at the
> outermost level; `?r` is bound by the occurrence block, optional in the taxon-free
> shapes, so an unbound `?r` went fresh and asked for every dated reference in
> Wikidata. The fix is no triple at all: `?ref_date` is already projected.

Worth recording how it was found: by not believing a measurement. An earlier commit
recorded the same query returning 0 as *correct*, on the strength of a group-by I
ran myself, and that group-by was wrong. Several later readings were rate limits
rather than results.

### 10. ~~DECIDE — two result parsers, three consumers~~ **DONE** · `541b170`

> The CLI and the server API read results with `csv::ByteRecord`; the app used the
> streaming reader. They were not equivalent: on a payload with one repeated row
> one returned 3 and the other 4, because only one deduplicated. The browser was
> the odd one out.

There is one reader now, and the decisions it forced are written up in the commit:
no deduplication, garbage refused rather than read as empty, a truncated payload
refused rather than read as complete.

### What the six have in common

Three of them passed every gate that existed. The gates were blind because they
asked questions a timing or a row count cannot answer — *is the query faster*, *is
the row count right* — while the failures were about **which value ends up where**.
Three rules now hold this in place (`tests/contract/structure_rules.rs`), over all
seven shapes rather than the one that broke.

Two of my own checks were non-discriminating and read as "no bug found", twice:
`COUNT(*)` cannot see an optional column change, and a count over an optional
occurrence block is the same with or without the OPTIONAL. A third, a gate whose
helper returned a single `}` instead of a group, asserted nothing at all. All
three were found by trying to make them fail.

---

## The two that matter most

### 1. ~~FIX — compound filter matched the wrong rows~~ **DONE** · REVIEW §12

> **Fixed in `88703e9`.** The compound filter's QID arm was comparing a compound's
> *slot* against the needle instead of its QID, so filtering by `Q3613679` returned
> nothing and filtering by `Q1` returned whichever compound sat at slot 1.

One line. The taxon and reference filters always did the translation; this arm
did not. My original hypothesis in REVIEW §12 — a badly sized bit mask indexed by
sparse QIDs — was wrong and is corrected there, because the ids are dense slots.

Worth keeping from this: the bug survived because the shared test fixture names
rows `"{qid}-name"`, so the *name* arm answered the same question and masked it.
The two new tests name compounds deliberately so the QID arm is the only thing
that can answer, and they assert correctness in **both** interning orders.

### 2. DECIDE — A complete query form is 50% faster · REVIEW §16, SPARQL-VARIANTS.md

> Inlining the nomenclatural closure as `VALUES ?root { … }` returns
> **identical** rows (32,160; 11,087 distinct compounds) in **4,860 ms instead
> of 9,716 ms**.

The interleaved alternative is faster still but **loses 79 compounds**, so it is
disqualified. This one is not.

Cost: one extra round trip in the query-build path (1,316 ms), which must be
cached per `(taxon, nomenclature)` to be worth it; a fallback if that lookup
fails; and a guard for an empty closure, because an empty `VALUES` is a valid
query that returns nothing.

*My recommendation:* **implement it**, behind the existing taxon cache, keeping
the subquery form as the fallback. It is the most expensive thing in a request
and this halves it.

### 3. DECIDE — A blank taxon box asks for the whole graph · REVIEW §10

> A `taxon="*"` query returns **572 MB** of JSON from QLever.

No cap on the web path, and nothing tells the reader before they wait. The CLI
*is* capped at 500 rows, so the two front ends differ.

*Ask, in order of help:* (a) warn when the taxon box is empty and nothing else
constrains; (b) cap this one case; (c) make an empty box mean the narrower
question so it is bounded by construction. All three are product calls.

---

## Also needs you

### DO — `CODECOV_TOKEN` · (wired in `a74383a`)

The workflow, `lcov` output and README badge are all in place and verified
locally. The upload step is a deliberate no-op until you add the secret on
codecov.io (Settings → Repository Access Tokens). **v5+ requires it even for a
public repo.** I cannot create a secret.

### FIX — The app crate's test suite is not hermetic · REVIEW §13

`cargo mutants` cannot run on `apps/lotus-explore-rs` at all: it copies the tree,
and `gate_consistency::repo_hygiene` reads the checkout, so the *unmutated*
baseline fails. This, not the mutant count, is what blocks ~2,700 mutants.

*Recommend:* have those tests resolve the repo root from `CARGO_MANIFEST_DIR`. A
small change, and worth more than any single module's mutants.

### FIX — What "green" means · REVIEW §1

The brief expected **1271** tests. At this commit the largest number is 1224
(browser) / 634 (server) / 598 (desktop) / 6 doctests. I could not reproduce 1271
in any combination, so I gated on "no count below the recorded baseline, per
build".

*Ask:* what was 1271 measured from? Until then the gate is my number, not yours.

### DECIDE — The sort cliff above ~1M rows · REVIEW §6

Confirmed real, three runs, 15 samples each: flat at ~9.9 ns/row to 1M, then
**517 ns/row** at 2M. Cause inferred (working set leaving cache), not proven.

*Cheapest decisive test:* run `bench_sort_scaling` on a machine with a different
L3 size. If the cliff moves, the fix is worth scoping; if it sits at 2M regardless,
it is structural and I should look again.

### DECIDE — Smaller items, all written up in full

| | Item | My lean |
|---|---|---|
| §7 | `*` + structure search doesn't filter on the root taxon. The documented arm is unreachable. | Measure row counts before touching — it may return *fewer* rows. |
| §11 | CLI caps at 500 rows; web fetches all. | Probably fine; worth a sentence in `docs/cli.md`. |
| §8 | Matrix tests check structure, not grammar. | Add `spareval` (pure Rust, small) as a dev-dep. |
| §5 | No criterion; benchmarks are print-don't-assert. | Only if you want 30-sample medians as a standard. |
| §14 | Four accessors I wrote tests for have **no production caller** — my own work. | Keep. They pin real invariants. |
| §15 | 12 `pub` items unreferenced; every crate is `publish = false`, so lint can't see it. | Needs someone who knows which are conveniences for the next feature. |

---

## DONE since these were written

- **§3, disk.** You freed space: **49 GB free**, was 12–17 GB and 94–98% full.
  This also unblocks §4.
- **§4, `cargo bloat` / `dhat` not installed.** Now *actionable* — install them
  and I can finally produce crate-level wasm attribution and peak RSS, both of
  which the baseline says are **not measured**.
- **§2, Spotlight.** Unmeasured whether load is still your indexing; it was 25–42
  at the start of this work and is **1.96** now.

## Not fixed, deliberately, and why

- **The whole-graph fetch (§10)** and the **inlined-query adoption (§16)** are
  both left because they are product and architecture calls, not defects.
- **Nothing was committed that I did not run.** No test was deleted, weakened or
  edited to make something pass. Test counts only went up.
