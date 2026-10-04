# Which query runs when

The reference for the search matrix: three arguments, twelve combinations, and
the query each one builds. Generated from the tests, not maintained by hand --
`crates/lotus-search/tests/matrix.rs` asserts this table cell by cell, so a change
to the builder fails the suite instead of quietly making this file a lie.

```bash
cargo test -p lotus-search --test matrix every_cell_is_substantial_and_the_table_does_not_drift -- --nocapture
```

`bytes` is the length of the generated query and `triples` the number of
statement-terminating triple patterns in it, counted after IRIs and string
literals are removed so the dots in `https://…` are not counted. Neither is a
cost measurement: a longer query is not a slower query, and the endpoint's time
is dominated by the patterns it has to resolve, not by the text.

| cell | bytes | triples | occurrence |
|---|---|---|---|
| taxon=absent structure=absent reference=absent | 2214 | 12 | optional |
| taxon=absent structure=absent reference=present | 2238 | 12 | optional |
| taxon=absent structure=present reference=absent | 2183 | 14 | optional |
| taxon=absent structure=present reference=present | 2207 | 14 | optional |
| taxon=specific structure=absent reference=absent | 2635 | 15 | required |
| taxon=specific structure=absent reference=present | 2659 | 15 | required |
| taxon=specific structure=present reference=absent | 2567 | 16 | required |
| taxon=specific structure=present reference=present | 2591 | 16 | required |
| taxon="*" structure=absent reference=absent | 2266 | 13 | required |
| taxon="*" structure=absent reference=present | 2290 | 13 | required |
| taxon="*" structure=present reference=absent | 2183 | 14 | optional |
| taxon="*" structure=present reference=present | 2207 | 14 | optional |

## How to read it

**`occurrence` is the column that matters.** `required` means `P703` is a plain
triple, so every row is an occurrence somebody recorded. `optional` means the
occurrence sits inside one `OPTIONAL` block -- taxon and reference bound
together, never per triple -- so compounds with no organism come back with empty
cells instead of being dropped.

Three rules produce the whole column:

1. **A named taxon requires an occurrence**, with or without a structure. A match
   that is not an occurrence in the requested taxon is not an answer.
2. **No named taxon makes it optional**, because a compound nobody has tied to an
   organism is one of the things being asked for.
3. **`*` requires an occurrence unless a structure is given.** `*` is the
   explicit request for what has been reported, so it keeps requiring one. With a
   structure there is no taxon to filter by and "every taxon" is expressed by not
   filtering, which leaves the occurrence optional.

Rule 3 is why the last two rows differ from the two above them.

## A reference is a constraint on `?r`, not a filter on a projected value

`criteria.reference` becomes `VALUES ?r { wd:Q… }` spliced into the base query's
outermost `WHERE`, not `FILTER(?r = wd:Q…)`. Two consequences, only one of which
is obvious:

- A `FILTER` would compare against an unbound `?r` -- a compound with no
  occurrence, which is precisely what a structure search exists to find -- and
  drop it.
- Because the `VALUES` is **outside** the optional block, a reference combined
  with no taxon returns only the compounds that reference actually reported, even
  though the block is optional. An unbound `?r` cannot join.

## `*` is read from the text, not from the resolved QID

`resolve_taxon` resolves `"*"` to `qid: None`, because the wildcard names no
Wikidata entity. By the time the query is built, `*` and an empty box therefore
look identical, and a builder that reads the difference off the QID alone cannot
tell them apart.

That is not hypothetical: `build_base_query` did exactly that, so `*` and an
empty box built the same query and `*` silently answered the broader question
instead of the narrower one it documents. The fix reads `criteria.taxon`, and
`a_wildcard_taxon_requires_an_occurrence_and_an_empty_taxon_does_not` in
`matrix.rs` is the test that fails without it.

The same gap exists in the structure branch and is **not** fixed, on purpose:
there a wildcard is already expressed by not filtering the taxon, and routing it
through the root taxon `Q2382443` would require an occurrence and so could return
fewer compounds than "every taxon" is supposed to. That is a judgement about
result size rather than a bug fix, so it is left for a human — see
`docs/sweep/REVIEW.md`.

## The empty-taxon cells return the whole graph

`taxon=absent structure=absent` asks for every compound in the projection with no
constraint at all. That is the largest answer this app can produce, and the
narrowest reading of "I typed nothing" is that nothing is constrained.

**This is deliberate, and the reasoning is written down** in
`apps/lotus-explore-rs/src/table_budget.rs`. An earlier 500-row ceiling was
removed on purpose, because it was a row-count bound standing in for a memory
bound, and it had three bad consequences:

- It was applied server-side by a `LIMIT`, so the table's filters could only ever
  see the first 500 rows while the toolbar reported the whole set's count. The
  two disagreed by construction.
- It stepped down per device, so a phone was told a smaller lie than a desktop.
- It is why a second `COUNT` query existed at all.

The interactive path now asks for **every** row, streams the answer into a
columnar set, and takes its counts from that set rather than from a second query.
Rendering cost is decoupled from result size: the set holds a row as three
dictionary ids and the table materialises only the thirty rows on screen.

So for these cells there is deliberately **no `LIMIT`** and no `truncated` flag
on the web path — `display_capped_rows` is a constant `false`, with the reason in
`features/explore/outcome.rs`. What remains is a ceiling on a single request's
payload, which belongs to the server rather than to the query.

Two things follow that are worth a human's attention rather than a change here:

- The **CLI** goes through `lotus-search::search`, which *does* cap at
  `DEFAULT_ROW_LIMIT` (500) and sets a `truncated` flag, because it is a library
  default rather than the interactive path. The two front ends therefore have
  genuinely different limits, which is defensible but should be known.
- An empty taxon therefore really does ask the endpoint for the entire
  projection — measured elsewhere in this repo at 2,990,730 rows and ~873 MB of
  CSV. Nothing caps it below that on the web path. Whether the reader is *told*
  that before waiting for it is a UI question; see `docs/sweep/REVIEW.md`.

## What this table does not cover

- **Row counts.** Nothing here was run against a live endpoint, deliberately: a
  test that reaches Wikidata fails when Wikidata is busy. The row counts for these
  twelve cells are **not measured** and no numbers are claimed for them.
- **Real parse validation.** Every cell is checked for balanced braces and
  parentheses and for scoped `^` in property paths, which catches a malformed
  query, but it does not parse the query with a SPARQL grammar. Doing that
  honestly needs a parser dependency (`oxigraph` or `spareval`), which is a
  dependency decision and so is flagged in `docs/sweep/REVIEW.md` rather than
  taken here.

## The distinct shapes, measured

The twelve cells are not twelve queries. They collapse into **six base shapes**
plus **orthogonal filter layers**, and the layers compose with any shape. Measured
from `lotus search --explain`, not asserted:

| Base shape | bytes | taxon closure | occurrence | structure service |
|---|---|---|---|---|
| nothing given | 2,195 | none | **optional** | no |
| `*` | 2,247 | none | required | no |
| named taxon, nomenclature on | 2,616 | nomenclatural subquery, then `P171*` | required | no |
| named taxon, nomenclature off | 2,284 | bare `P171*` | required | no |
| structure only | 2,169 | none | **optional** | yes |
| structure + named taxon | 2,553 | nomenclatural subquery, then `P171*` | required | yes |

| Filter layer | What it adds | Composes with |
|---|---|---|
| reference | `VALUES ?r { wd:… }` in the outer `WHERE` | every shape |
| mass | `?c wdt:P2067` + a `FILTER` on the projected mass | every shape |
| year | `?r wdt:P577` + `YEAR(?ref_date)` | every shape |
| formula | element-count binds + a `FILTER` | every shape |

So the answer to "is it one query with filters bolted on" is **no, and it is not
close to no**: the taxon argument alone selects between three different closure
forms, and the occurrence requirement alone flips between required and optional.

## Completeness is checked structurally, not by result

**Every claim in this file is about the shape of the query. None of it is a
statement that the query returns every row it should.** The structural tests pin
which `OPTIONAL` a pattern sits in and which subquery it lands in; they cannot
tell you a compound was dropped.

That gap is real and it is still open. Settling it needs each shape compared
against an independently written query by **set** of distinct compound QIDs --
not by count, and not by inspection -- and the endpoint makes that awkward: it
rate-limits, and a hand-written control is only worth anything once it is
verified to be equivalent, which is circular if you verify it with the same
endpoint. **A local QLever** removes both problems and is the way to do this
audit properly.

### A false alarm worth keeping, and the mistake behind it

An earlier version of this section reported a bug: for `taxon=Q16521` the
generated query returned zero rows while a hand-written reconstruction returned
one distinct compound. **There was no bug.** `Q16521` is Wikidata's *taxon rank*
item, not a taxon, so zero rows is the correct answer and the reconstruction was
wrong.

The mistake is the one worth recording. `Q16521` was taken from this repository's
own test fixtures -- `crates/lotus-curation/src/constants.rs` calls it
`WD_TAXON_QID`, and `crates/lotus-jsonld` pairs it with a species name in a
synthetic fixture. **Those are scripted test data, not reference data.** A QID
that appears in a fixture is not thereby a real taxon, and using one as though it
were is how a measurement ends up measuring the wrong thing.

It briefly made it into the user-facing documentation too: `docs/cli.md` and the
CLI crate's README used `--taxon Q16521` as the runnable example, which would have
given anyone who copied it an empty result. Both now use `Q21754`, which is
verified to have compounds (27,952 rows, 11,087 distinct compounds, measured).

**The general rule: verify a QID against the graph before using it as an example,
and never promote a fixture identifier into reference data without doing so.**
