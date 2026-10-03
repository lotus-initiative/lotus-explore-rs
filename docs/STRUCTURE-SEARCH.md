# Structure search and compound identity

> **Reader-facing answer lives on the FAQ.**
> The behaviour described here — what a search does, and what each switch changes — is
> now on `/faq`, translated, and is the page a reader is sent to. This file keeps the
> *engineering rationale*: why the query is shaped this way, what was measured, and which
> failure each decision avoids. If you are here to find out how the tool behaves, use the
> FAQ; if you are here to find out why it is built this way, keep reading.
>
> Concretely, these have moved and are not repeated below: the four accepted kinds of
> input, the three modes, the miss rule and its one asymmetry, and the table of what each
> lookup costs. Kept here because they are what the rationale below refers to.


The structure field is the second free-text field in LOTUS Explore. It takes four
kinds of thing in one box, resolves all four to a Wikidata compound, and then
searches that compound in one of three ways:

| Input | Example | Resolved by |
| --- | --- | --- |
| a Wikidata QID | `Q23118` | `compound_by_qid_query` |
| an `InChIKey` | `DBOVHQOUSDWAPQ-WTONXPSSSA-N` | `compound_inchikey_query` (`P235`) |
| a compound name | `amarogentina` | `compound_label_query`, then `compound_alias_query` |
| a structure | `C[C@H](O)CO` | `structure_compound_lookup_query` — the structure service |

## Resolution first, then the mode

The order is the whole design. **Every** input is resolved to a compound before
anything is searched, a structure included, because the default mode needs a QID
and a structure is not one. Only then does the mode decide what happens:

| Mode | Question | Route |
| --- | --- | --- |
| **exact** (default) | this compound, and nothing else | `VALUES ?c { wd:Q… }` — no structure service |
| substructure | every compound containing it | the structure service |
| similarity | every compound at least *n* similar | the structure service, at the reader's cutoff |

Two consequences follow, and both are deliberate.

**The mode is available for every input kind.** A reader who types `amarogentina`
and wants the compounds that contain it is asking a real question, and knowing
which compound it named does not answer it. Reserving the two narrower modes for
SMILES would have made them unreachable from the input people actually use.

**The mode never touches the resolution query.** Resolution asks its own question
— *is this a compound you have?* — and asks it at a cutoff of 1.0 whatever the
reader set. `structure_compound_lookup_query` has no threshold parameter to borrow.
If it borrowed the reader's, a lenient search would resolve a structure to a
near-neighbour and the exact route would then report that near-neighbour as the
compound they named.

The two routes differ by an order of magnitude, which is why exact is the default
rather than a coincidence: an index scan on one QID against a load-and-score pass
over every candidate compound. But when the input resolved to nothing, exact has
no QID to scan, and the closest thing the service can offer is the same molecule —
an identical fingerprint, which is a similarity search at a cutoff of 1.

## The four lookups

| Route | Property | Recognised by | Lookup |
| --- | --- | --- | --- |
| QID | — | `Q` + digits | `compound_by_qid_query` — a `VALUES`, and nothing to match |
| identifier | [`P235`](https://www.wikidata.org/wiki/Property:P235) *InChIKey* | the standard's fixed 14-10-1 shape | `compound_inchikey_query` |
| name | `rdfs:label` | `could_be_a_compound_name` | `compound_label_query` |
| alias | `skos:altLabel` | could be a name, and no label matched | `compound_alias_query` |
| structure | — | anything else, incl. molfiles | `structure_compound_lookup_query` — the service, at a cutoff of 1 |

All four return the same four columns — QID, label, canonical SMILES, and which
route answered — and one parser reads them all. The canonical SMILES is there
because a substructure or similarity search has to hand the service a structure,
and for an input that named a compound the structure it has is that compound's own.

The structure route is the only one that asks the structure service, and only
because nothing else can answer it: there is no property that maps a SMILES string
to an item.

A QID still costs one query, and it is worth being honest about why. `Q18216` is
already Wikidata's own name for a compound, so the lookup learns nothing about
*which* compound is meant. What it does is confirm the item exists: without it a
mistyped QID builds an identity query that matches nothing and returns an empty
table, which reads as "this compound has no results" rather than "that is not a
compound". One `VALUES` turns the second into the first, with the input quoted back.

The `InChIKey` route is the cheapest of the three by a wide margin — one indexed
equality on a value that is unique in practice, against a name lookup's `VALUES`
over fifteen language tags plus a fallback query behind it.

An `InChIKey` and a QID each stop at their own query. An `InChIKey` names one
compound by construction, and a QID names one outright, so if neither is known a
name lookup is not going to find what the reader meant. A name is the only kind
that can be wrong in a way another kind would catch, which is why it is the only
one that falls through: label first, then alias, because a label is the item's
preferred name and so is the one a reader meant.

## A miss is an error — except for a structure

A QID, a name or an `InChIKey` that matched nothing is a claim about a compound
that does not exist, and there is nothing to guess at. The reader gets `Compound
not found`, with the input quoted back, rather than rows for a search they did not
ask for. That is the taxon field's rule, for the same reason.

A **structure** that Wikidata has no compound for is not a miss at all. It is a
good structure that is simply not in the database, and searching it is what the
reader asked for. Nothing about a SMILES is a claim that can turn out to be wrong,
which is why this is the one asymmetry in the resolver — and why refusing it would
break every structure search there ever was.

The miss is remembered along with the candidate list, so a repeat search of the same
typo refuses in the same way and does not spend a second round trip. That
consistency is the point: the alternative is a first search that says "not found" and
a second one that quietly returns a substructure search for the word.

A **structure** never reaches that rule. It is never looked up, so nothing here can
start failing for input that worked before — the structure service's own rejection
of an unparsable structure is still the message a reader gets for bad structures,
which is a message about the structure, which is the right one.

## Why the label and alias lookups are two queries

Measured, not stylistic. QLever will not push a `VALUES` down into both arms of a
`UNION`, so it falls back to scanning each property — and scanning `rdfs:label` means
17.9M rows.

| Query | Result on QLever |
| --- | --- |
| `compound_by_qid_query` | instant — one `VALUES` |
| `compound_inchikey_query` | ~0.2s |
| `compound_label_query` | ~0.15s |
| `compound_alias_query` | ~0.14s |
| `structure_compound_lookup_query` | one service call, capped at `LIMIT 5` |
| label `UNION` alias | **times out** |
| `FILTER(LCASE(STR(?label)) = …)` over `rdfs:label` | **times out** |
| `?label ql:contains-word "…"` | returns nothing — index not loaded |

So the two are separate requests, and the label query is a `VALUES` over
language-tagged literals rather than a filter on lexical forms. A label *is*
language-tagged, so the tag has to be part of the match rather than discarded from
it. `COMPOUND_NAME_LANGUAGES` is the resulting list of fifteen tags.

### What that language list costs

A compound name in one of the remaining ~285 tags will not resolve. This is a real
limitation and it is not hidden: a short list, in one place, with the reason beside
it. The alternative — a lexical-form filter — times out, so the choice is between a
list covering most names and no name lookup at all.

Two further guards on the name routes:

`STRSTARTS(…, "…/entity/Q")` keeps lexemes out. `?compound rdfs:label "aspirin"@en`
also matches three senses of the English lexeme, which are not compounds.

A compound with no `P233` canonical SMILES is still a perfectly good answer to an
exact search, which needs only the QID. A substructure or similarity search does
need a structure, and falls back to the reader's own input when the compound has
none — for a name that is not a structure, and the service will say so, which is
the right place for that to surface.

## Why a SMILES is never silently replaced

The guard is `could_be_a_compound_name` in `lotus-model`, and it refuses input
carrying a digit or SMILES punctuation, and refuses a bare run of uppercase
letters. Without it, every structure search would first cost a name lookup — and
`CC` would go looking for a compound Wikidata happens to have spelled the same
way.

The second rule has a cost worth stating plainly: **`ATP`, `GDP` and `NAD` are
compound names Wikidata knows, and they are refused**, because they are written the
same way as `CC` and `CCC` — both of which are plausible things to type in a
structure box. A structure silently replaced by a same-spelled Wikidata item is a
failure the reader cannot detect; a name they have to type as SMILES or as an
`InChIKey` is a friction they can route around. The rule is written so that changing
it has to be deliberate.

## Notices

An input that resolves reports what it became — the label that was typed and the
compound it named — because that is the actionable part: the search is now running
against a *different compound* than the one in the box, and in the two broader
modes against a different structure as well. Several matching compounds
add an ambiguity notice on top, and the resolution is cached with its candidate list
so a repeat search says the same thing.

## Where the structure lookup lives

| Surface | How |
| --- | --- |
| Model | `names_a_compound` (the one gate the panel and the resolver share), `could_be_a_compound_name`, `looks_like_inchikey`, `looks_like_a_compound_qid`, `STRUCTURE_INPUT_EXAMPLES`, `SmilesSearchType` |
| Query | `compound_by_qid_query`, `compound_inchikey_query`, `compound_label_query`, `compound_alias_query`, `structure_compound_lookup_query`, `COMPOUND_NAME_LANGUAGES`, `exact_compound_query` |
| Parse | `parse_compound_lookup_csv` |
| Search | `resolve_structure::resolve`, called in `build_execution_plan` before the query is built; it returns `ResolvedStructure { compound, structure }`, and `build_sparql_query` picks the route from the mode |
| Cache | `structure_cache::CachedCompound`; `Cached::Unresolved` records a remembered miss, and a remembered miss still refuses |
| Notice | `LookupNotice::CompoundResolved`, formatted by `warn_compound_resolved` in all four locales |

And where the mode itself is carried:

| Surface | How |
| --- | --- |
| Model | `SmilesSearchType::{Exact, Substructure, Similarity}`, `Exact` is `Default` |
| URL | `structure_search_type=exact\|substructure\|similarity`, and `similarity_threshold` for the third |
| API | `SearchRequest.structure_search`, plus `similarity_threshold`; the old `smiles_threshold` is still accepted |
| CLI | `--structure-search exact\|substructure\|similarity`, `--threshold` |
| UI | three radios under the structure field; the cutoff appears only for similarity |

The CLI is the one surface that does **not** resolve. It shares the query builders
with the app but not the resolver, so `--structure` takes a structure — which is
what its own description says, and why `--structure-search exact` there means a
same-molecule search rather than an identity one.

## The example buttons

`STRUCTURE_INPUT_EXAMPLES` in `lotus-model` holds one of each accepted kind, and the
buttons read that constant rather than keeping their own list. The test that checks
each one is classified correctly therefore checks the same strings a reader can
click, so the two cannot drift apart.

| Example | Kind | Route |
| --- | --- | --- |
| `amarogentina` | name | label, then alias |
| `DBOVHQOUSDWAPQ-WTONXPSSSA-N` | `InChIKey` | `P235` |
| `Q23118` | QID | a `VALUES`, and nothing to match |
| `C[C@H](O)CO` | SMILES | the service, at a cutoff of 1 |

The first two are the same compound — [Q3613679](https://www.wikidata.org/wiki/Q3613679),
*amarogentin* — reached by an alias and by its `InChIKey`. That is the point of
having both: they exercise the alias fallback and the identifier route, and a
reader can see that the two agree.
