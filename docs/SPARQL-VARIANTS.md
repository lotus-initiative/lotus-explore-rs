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
## Projection: `?ref`, dropped and put back

**This section is a reversal. It is kept because the reasoning was sound and the
conclusion was still wrong, which is the more useful thing to record.**

`?ref` was dropped from the outer `SELECT` on the grounds that nothing read it: no
parser column resolved it and no export format emitted it. It was the largest
single column in an unnarrowed result -- 54.3 MB of a 400 MB payload, 13.6% --
computed by QLever, serialised, and sent on all 723,990 rows.

Measured on `Q21754`, both against the live endpoint with `Accept-Encoding: gzip`:

| Form | rows | raw | wire | wall |
|---|---|---|---|---|
| projecting `?ref` | 32,160 | 18,385,046 | 4,215,824 | 4.95 s |
| not projecting it | 32,160 | 15,973,042 | 3,522,695 | 3.31 s |

13.1% off the decompressed payload, 16.4% off the wire, 33% off wall clock, and
the row count unchanged. Every number held up.

And it was still wrong, for two reasons neither of which a timing can show.

**The set-based exporter needs it.** The argument above was that dropping the
projection is free for the Turtle export, because `construct_from_select` replaces
everything from the first `SELECT` to the first `WHERE` with a hardcoded
`CONSTRUCT` template -- so the projection is discarded and `?ref` stays bound by
the innermost `?ref pr:P248 ?r` regardless. That is true, and it is true only of
the *endpoint* path. There is a second Turtle exporter, `RowExporter`, which
builds triples from the parsed `ColumnarResultSet` rather than from a CONSTRUCT.
That one had no `?ref` at all, so it emitted

    wd:Q103815959 prov:wasDerivedFrom wd:Q35589126 .

with the **compound** as the subject -- the statement is what was derived from the
reference -- and no reference node in the graph at all. So a compound reported
through two references produced two identical triples and the occurrences were
indistinguishable. Two exporters, two code paths, one of which the measurement
never exercised.

**It cannot be cast to an integer.** `?ref` is a reference *node*:

    http://www.wikidata.org/reference/9642f33c41767b8a0f7cb06de523c17dda0b921d

not a `Q<digits>`. Projecting it through the `STRAFTER`/`xsd:integer` treatment
every QID column gets is a type error on every row, and QLever reports that as an
unbound cell rather than a failed query -- so the column would look present and be
empty throughout. `?ref_qid` is a different identifier and is unaffected: it is the
publication the node points at via `pr:P248`.

`?ref` is projected again, stored per row, and shown to nobody -- the user-facing
column list is a separate constant.

**And then projected as an identity rather than a URI** (`7aadcb1`). Sent whole and
stripped on arrival, the reference namespace was 35 wasted characters a row and the
statement namespace 39 more. Both now go over the wire as the part that identifies
them:

    (STRAFTER(STR(?ref), "reference/")       AS ?ref_node)
    (STRAFTER(STR(?statement), "statement/") AS ?statement_id)
    (SUBSTR(STR(?ref_date), 1, 4)            AS ?ref_year)

They need new names because QLever refuses an `AS` clause whose target already
appears in the body -- and `?ref_date` *is* in the body whenever the year filter is
on. `RowExporter` puts the namespaces back when it writes Turtle, which is where the
URI is actually wanted.

Measured on `Q21754`, cumulatively:

| step | raw | wire |
|---|---|---|
| both URIs, whole | 18,385,046 | 4,217,812 |
| identities instead of URIs | 15,973,054 | 4,089,281 |
| year instead of `xsd:dateTime` | 15,143,494 | 4,007,601 |

**17.6% off the raw payload and 5.0% off the wire.** The gap between those two
numbers is the whole point of quoting both: gzip already compresses a namespace
repeated on every row to almost nothing, so what this buys is decompression and
parse, not bandwidth. The first change was 13.1% of raw and **3.0% of wire**; a
figure quoted without its compressed counterpart would have been misleading.

Three gates hold the projection in place: `ref_node` is in `SELECT_COLUMNS`, the
namespaces are asserted absent from the query, and the Turtle round-trip is checked
to still emit full URIs for `p:P703` and `prov:wasDerivedFrom`.

The year step also merges rows that `SELECT DISTINCT` had kept apart because they
differed only in the discarded part of a date. That is not a silent content change:
the store already dropped those rows, and the CLI returns the same 27,952 rows over
the same 11,087 compounds either way.

The transferable lesson is narrow and worth stating: **a payload measurement can
tell you a column is unread by the code you measured, and cannot tell you whether
every consumer was measured.** Two of the three real defects in this file were
invisible to row counts and to timings.

### Coalescing the two SMILES columns: measured, and rejected

The obvious next move is the pair `compound_smiles_conn` / `compound_smiles_iso`.
The parser picks the isomeric one when it is there (`stream.rs`, `raw_row`), so
`compound_smiles_conn` crosses the network and is then discarded on every row that
also has the other. On the unnarrowed query that is **40 MB of 400 MB, and 27 MB
of it recoverable**, and it looks like the same kind of free win as `?ref`.

It is not, and the reason is `SELECT DISTINCT`.

Measured on the unnarrowed query: 0 rows carry only the isomeric SMILES, 484,788
carry both, and 239,202 carry only the connection-table one. So 67% of rows send
a `compound_smiles_conn` that is thrown away -- and the projection is what
`DISTINCT` distinguishes on. Sending one coalesced column merges rows that were
distinct *only* in `compound_smiles_conn`:

| Form | rows |
|---|---|
| both columns projected | 32,160 |
| one coalesced column | **31,024** |

1,136 rows fewer, and they are not duplicates in any sense the table can show. An
example, differing in nothing else:

```
compound 418599   taxon 156354 (Coffea)   ref 44023965   statement Q418599-5015D525-...
compound_smiles_iso  (empty)
compound_smiles_conn c1cc(c(cc1C(=O)O)O)O   vs   C1=CC(=C(C=C1C(=O)O)O)O
```

Two spellings of the same structure, aromatic and kekulised, on one Wikidata
item. Because the isomeric column is empty here, the parser was *displaying* the
connection-table value -- so the table showed two rows for one occurrence with two
different SMILES strings, and coalescing deletes one of them.

**So this is a content change and a product decision, not an optimisation.** It is
recorded here so the next person does not spend the afternoon finding it again.
Recovering those 27 MB requires either accepting the merge, or carrying a
discriminator alongside the coalesced column, which gives the bytes back.

The Turtle would have been fine, incidentally: the `CONSTRUCT` template names both
variables and both are bound by the `WHERE`, and the two rewritten queries came out
byte-identical. It was the CSV that could not afford it.

## The sort cliff is smaller than recorded, and ranking does not help

`6b33253` measured `sort by name` at ~9.9 ns/row to a million rows and then 52x
worse at two million, and attributed it to the working set leaving cache. The fix
that follows from that story is to stop resolving values during the sort: rank
each row's dictionary value once, then compare `u32`s.

Implemented and measured on this machine, release build, seven samples, medians,
using this file's own payload generator so the data is the recorded data:

| rows | by value | by rank | |
|---|---|---|---|
| 500,000 | 5.0 ms | 222.3 ms | 44x slower |
| 1,000,000 | 10.2 ms | 463.1 ms | 45x slower |
| 2,000,000 | 50.1 ms | 962.3 ms | 19x slower |

The orders were identical, so the ranking was correct. It is also worthless, for a
reason worth writing down: **ranking has to compare the values too.** Grouping
them into a `BTreeSet` to assign ranks is `O(distinct log distinct)` *string*
comparisons, and filling the rank lookup is the same again. The sort it was meant
to accelerate then does `O(n log n)` integer comparisons instead of `O(n log n)`
string comparisons -- so the string work is paid twice rather than once. The
premise that ranking removes the comparisons is simply false; it only changes
which loop pays for them.

The one shape where it would pay is grouping by *dictionary slot* rather than by
value, which makes the grouping integer-keyed. That was written first, and it is
wrong: absence is a property of the resolved value, not of the slot. A row can
name a taxon slot that holds no name, and it compares equal to a row with no taxon
at all; keyed by slot the two get different ranks, the tie-break that orders equal
values never runs, and the table silently reorders.

**The more useful finding is the first row of the table.** By-value sorting is
10.0 ns/row at 500,000, 10.2 at a million, and 25.1 at two million -- a 2.5x step,
not 52x. The cliff as recorded does not reproduce on quiet hardware; the original
numbers were taken at load average 25-42 and re-measured at 3.7, which is still
busy. Whatever is at two million rows, it is worth a few tens of milliseconds
rather than a second, and a fix aimed at a 52x cliff is not aimed at this.

Both attempts were reverted. The measurement harness is not kept, because a
benchmark of a removed optimisation is a benchmark nobody runs.
