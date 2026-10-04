# Decisions: what needs you, in order

One page. `REVIEW.md` has the evidence and the reasoning for all seventeen items;
this is the checklist, ordered by what it costs to leave alone.

**Read this file first.** Then `SUMMARY.md` for what was done, `MUTANTS.md` for
the mutation results, `BASELINE.md` for the measurements.

Legend: **FIX** = a defect I did not fix and you should want fixed.
**DECIDE** = a judgement call only you can make. **DO** = an action only you can
take. **DONE** = resolved since it was written.

---

## The three that matter most

### 1. FIX — The table's compound filter may match rows it should not · REVIEW §12

> On a two-row set, compound filter `"Q1"` returns **both** rows while `"Q4"`
> correctly returns one.

A filter that returns rows which do not match is a wrong table, not a slow one.
Reproducible, minimal case in the write-up. The suspected cause is the filter's
bit index: the mask is sized by dictionary length and indexed by sparse numeric
QIDs, so a high-numbered QID may fall outside the mask.

I did not fix it because it needs a decision about which index space is intended,
and the honest test needs a recorded fixture with realistic QIDs that this repo
does not have. **A wrong fix to a filter is worse than a written-up bug.**

*Ask:* should I write the failing test first with a realistic QID and fix it?

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