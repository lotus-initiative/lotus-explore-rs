# SPARQL variants: what was measured against the live endpoint

Measurements of alternative query forms for the search matrix, taken against
`QLever` (`https://qlever.dev/api/wikidata`). Everything here was run by hand and
recorded; **none of it is a test**, because a test that reaches a live endpoint
fails when the endpoint is busy.

The queries themselves come from `lotus search --explain`, which builds the query
without sending it. That is the only reason this file is possible without adding
code: the harness reads what the application would actually send.

## The measurements

Taxon `Q21754` (*Gentianales*), nomenclature all on. Completeness is measured
first and separately from speed, because a variant that is faster because it
joins less is not an optimisation. 2026-10-04.

### Step 1 — completeness, via `COUNT` (cheap, not subject to result truncation)

| Form | distinct compounds | total rows |
|---|---|---|
| two subqueries (current) | **11,087** | 27,952 |
| interleaved single path | 11,008 | 27,831 |

**The interleaved form loses 79 distinct compounds.** It is disqualified, and
this is the number that decides it. A 19% speed advantage on a form that answers
a different question is not an optimisation.

This corroborates the argument already in `nomenclature_path`, which cites "11009
compounds against 11088 for the subquery form" on this same taxon. Today's figures
are 11,008 and 11,087 — each within one of the recorded pair, so the comment was
accurate and has drifted only with the graph. **Recommend fixing the comment's
figures**, or dropping them and keeping the argument, which stands without them.

### Step 2 — a third form that is both complete and faster

`VALUES ?root { … }` inlined, where the list is the seed's nomenclatural closure
computed once beforehand instead of by a subquery at query time.

The closure for this taxon is **two taxa** — `Q21754` and one synonym,
`Q2102991` — which is what `nomenclature_path` predicts ("a handful of items, not
a subtree; measured at 1 to 2 hops on every taxon tested").

| Form | distinct compounds | total rows | **server ms** | wall |
|---|---|---|---|---|
| two subqueries (current) | 11,087 | 32,160 | 9,716 | 61.85 s |
| interleaved | 11,008 — lossy | — | — | — |
| **inlined `VALUES`** | **11,087** | **32,160** | **4,860** | **41.90 s** |

**This is the answer to the question: the inlined form returns byte-identical row
counts and runs in half the time.** 50% off the endpoint's compute, 32% off wall
clock including result transfer.

The saving is far larger on the full query (4,856 ms) than on the bare `COUNT`
(528 ms), which is the interesting part. Removing a join *early* in a query that
then computes `DISTINCT` over fourteen projected columns saves a great deal of
duplicate intermediate work — the `COUNT` never pays that cost, so a count-based
comparison would have understated this by an order of magnitude. **Compare on the
query the application actually sends.**

### The cost, stated plainly

Computing the closure costs **1,316 ms** — one extra round trip — so a naive
implementation is 1,316 ms to save 4,856 ms: a net win of ~3.5 s on this query,
but only if the closure is not recomputed per search.

It does not have to be. The closure is a function of `(taxon, nomenclature)` alone
and does not depend on any user filter, so it is cacheable per taxon. Amortised,
the cost is ~0 and the saving is the full 4.9 s per search.

**Why this is not simply committed:** it changes what the query *is*. The query
text stops being a pure function of the form input and becomes a function of
`(taxon, nomenclature, closure-at-time-T)`. Consequences, all of which a caller
has to agree to:

1. **A new network round trip in the query-build path**, and therefore a new
   failure mode. If the closure lookup fails there must be a fallback to the
   subquery form, or the search fails for a reason that has nothing to do with the
   search.
2. **An empty closure must be guarded.** A `VALUES` list with nothing in it is a
   valid query that returns nothing — a silent wrong answer rather than an error.
   The seed is always in its own closure, so this should not happen, and "should
   not happen" is not a guard.
3. **Nomenclature off costs nothing** — `nomenclature_path` already returns `None`
   and there is no closure to resolve — so the extra round trip applies only to the
   cases where relationships are enabled.
4. The query remains **self-contained and pasteable** (the `VALUES` are literal in
   the text), so `docs/cli.md`'s promise still holds. This is the one risk that
   turns out not to be real.

Both front ends and the cache keys change, because the query text changes. First
search after a deploy misses cache, which is expected.

## What was not measured, and why

**The whole-graph cells were not measured at all.** `taxon=absent` and `taxon="*"`
have no taxon filter, so they ask for the entire projection. One attempt at
`taxon="*"` returned **572 MB of JSON** and exhausted the client parser before it
finished; the untaxonomised query is larger still.

That is not a reason to shrug. It is the answer to the question this file was
opened to settle, and it is an uncomfortable one:

- On the web path the query carries **no `LIMIT` and no cap** — see
  `docs/QUERY_MATRIX.md`, where the deliberate removal of the old 500-row ceiling
  is recorded. A blank taxon box therefore asks for the whole graph, every time.
- The cost is not hypothetical: the app's own documentation measures a
  2,990,730-row, ~873 MB export as the widest search, and
  `crates/lotus-query/tests/bench.rs` measures 5.5 s to parse it from CSV.
- On the *library* path there is a cap (`DEFAULT_ROW_LIMIT`, 500) that puts a
  `LIMIT` in the query. The two front ends differ here, and only the library one
  is bounded.

**Recommended, in order of how much it would help a reader:** (1) tell the reader
before they wait, (2) put a real cap on the web path for this one case, or (3)
make the empty box mean the narrower question so it is bounded by construction.
All three are product decisions and none is a query-builder change, which is why
they are in `docs/sweep/REVIEW.md` rather than fixed here.

Also not measured: the trimmed form (dropping the `rdfs:label` `OPTIONAL`s and
their `FILTER(LANG(…))`), and `counts_query` against the full select. Both are
cheap to measure on a *bounded* taxon and would be the next thing to run; neither
was attempted because the rate limiting below had already used the budget.

## Limits, and hitting them

`QLever` is a free academic service and this was treated as one:

- Bounded taxa only, never the whole-graph cells.
- Two repeats, six seconds apart, using the server's own reported
  `query-time-ms` rather than many client-side timings.
- Stopped on the first rate limit rather than retrying through it.

**Two `429 Too Many Requests` responses were received** and one 572 MB response.
The work stopped after the single comparison above, with a five-minute back-off
between attempts, because the endpoint was signalling that it was the wrong thing
to keep doing. The remaining variant measurements need either a scheduled slot, a
local QLever, or recorded fixtures — not a tighter loop.

## Reproducing

```bash
# The query the app would send, without sending it.
lotus search --taxon Q21754 --explain

# One timed request. `query-time-ms` is QLever's own compute time, which is a
# better number to compare than wall time on a shared machine.
curl -s -G https://qlever.dev/api/wikidata \
  --data-urlencode "query=$(lotus search --taxon Q21754 --explain)" \
  --data-urlencode "action=json_export" | jq '.meta'
```

The interleaved variant is the same query with the two-subquery closure block
replaced by:

```sparql
?t (wdt:P171*|(wdt:P1420|wdt:P12763|wdt:P566|wdt:P12766|wdt:P1403|wdt:P12765|wdt:P694|wdt:P12764|^wdt:P1420|^wdt:P12763|^wdt:P566|^wdt:P12766|^wdt:P1403|^wdt:P12765|^wdt:P694|^wdt:P12764)*) wd:Q21754 .
```

**Measure the row count as well as the time.** A form that is faster because it
joins less is not an optimisation, and a timing on its own cannot tell the
difference.