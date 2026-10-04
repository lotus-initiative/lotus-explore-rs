# SPARQL variants: what was measured against the live endpoint

Measurements of alternative query forms for the search matrix, taken against
`QLever` (`https://qlever.dev/api/wikidata`). Everything here was run by hand and
recorded; **none of it is a test**, because a test that reaches a live endpoint
fails when the endpoint is busy.

The queries themselves come from `lotus search --explain`, which builds the query
without sending it. That is the only reason this file is possible without adding
code: the harness reads what the application would actually send.

## The measurement that mattered

**Nomenclature closure: two subqueries, or one interleaved property path?**

This is the question `docs/QUERY_MATRIX.md` leaves open, and the one
`nomenclature_path` in `crates/lotus-query/src/query.rs` answers with a comment
rather than a measurement. Taxon `Q21754` (*Gentianales*), nomenclature all on,
2 repeats, 2026-10-04.

| Form | Server `query-time-ms` | Wall | Result rows |
|---|---|---|---|
| **two subqueries (current)** — expand the seed's nomenclatural closure, then `P171*` down from each result | **12,281** | 19.00 s | **32,160** |
| **interleaved** — one path, `?t (wdt:P171*|(SYN…)*) wd:Q21754 .` | **9,911** | 15.42 s | **32,032** |

**The interleaved form is 19% faster and returns 128 fewer rows.** The two forms
do not agree, so this is not a free win and the current form stays. The saving is
real and the disagreement is the reason not to take it: a query form that returns
a slightly different answer for 19% less time is a correctness question wearing a
performance costume, and the difference is invisible to a reader who has no way to
know which set they were shown.

### This corroborates the code comment, with a caveat about units

`nomenclature_path` already argues for the subquery form, and cites
"On `Gentianales` (Q21754) that is 11009 compounds against 11088 for the
subquery form". That is the same taxon and the same direction — interleaved
returns fewer — and this measurement reproduces the direction on current data.

The caveat: **the comment counts compounds and this table counts result rows.**
A row is a compound * taxon * reference, so 32,160 rows is not 32,160 compounds
and the two figures are not comparable to each other. Anyone reading the comment
next to this file will see two numbers for one taxon and reasonably assume they
measure the same thing. **Recommend updating the comment** to say which unit it
is, or to drop the figures and keep the argument, which stands on its own without
them.

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