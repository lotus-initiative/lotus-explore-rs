# Which query runs when

The reference for the search matrix: three arguments, twelve combinations, and
the query each one builds. Generated from the tests, not maintained by hand --
`crates/lotus-search/tests/matrix.rs` asserts this table cell by cell, so a change
to the builder fails the suite instead of quietly making this file a lie.

```bash
cargo test -p lotus-search --test matrix every_cell_is_well_formed -- --nocapture
```

`bytes` is the length of the generated query and `triples` the number of
statement-terminating triple patterns in it, counted after IRIs and string
literals are removed so the dots in `https://…` are not counted. Neither is a
cost measurement: a longer query is not a slower query, and the endpoint's time
is dominated by the patterns it has to resolve, not by the text.

| cell | bytes | triples | occurrence |
|---|---|---|---|
| taxon=absent structure=absent reference=absent | 2221 | 12 | optional |
| taxon=absent structure=absent reference=present | 2245 | 12 | optional |
| taxon=absent structure=present reference=absent | 2190 | 14 | optional |
| taxon=absent structure=present reference=present | 2214 | 14 | optional |
| taxon=specific structure=absent reference=absent | 2642 | 15 | required |
| taxon=specific structure=absent reference=present | 2666 | 15 | required |
| taxon=specific structure=present reference=absent | 2574 | 16 | required |
| taxon=specific structure=present reference=present | 2598 | 16 | required |
| taxon="*" structure=absent reference=absent | 2273 | 13 | required |
| taxon="*" structure=absent reference=present | 2297 | 13 | required |
| taxon="*" structure=present reference=absent | 2190 | 14 | optional |
| taxon="*" structure=present reference=present | 2214 | 14 | optional |

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