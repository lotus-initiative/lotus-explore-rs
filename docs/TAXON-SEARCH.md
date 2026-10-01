# Taxon search and nomenclatural names

A taxon search in LOTUS does not ask one question. It asks *"which compounds
were reported from this organism?"*, and the organism a paper names is rarely
the organism Wikidata files the compound under. Taxonomy is revised; the
publications are not. *Leontopodium nivale* was found to be the same species as
the long-known *Leontopodium alpinum*, and every compound ever reported from
either name is a compound from that plant.

So a taxon search follows the taxon's **nomenclatural closure**: the name you
gave, plus every other name Wikidata links to it, plus the descendants of all
of them. There are **four independent relationships** it can follow, all on by
default, and each of them is a separate question.

## The four relationships

Each is stored twice in Wikidata, once from each end, and the four are
independent of each other.

| Relationship | Wikidata pair | Chronological? |
| --- | --- | --- |
| **accepted name ↔ its synonyms** | [`P1420`](https://www.wikidata.org/wiki/Property:P1420) *taxon synonym* / [`P12763`](https://www.wikidata.org/wiki/Property:P12763) *taxon synonym of* | **no** |
| **new combination ↔ its basionym** | [`P566`](https://www.wikidata.org/wiki/Property:P566) *basionym* / [`P12766`](https://www.wikidata.org/wiki/Property:P12766) *basionym of* | yes |
| **current name ↔ its original combination** | [`P1403`](https://www.wikidata.org/wiki/Property:P1403) *original combination* / [`P12765`](https://www.wikidata.org/wiki/Property:P12765) *protonym of* | yes |
| **replacement name ↔ what it replaced** | [`P694`](https://www.wikidata.org/wiki/Property:P694) *replaced synonym (for nom. nov.)* / [`P12764`](https://www.wikidata.org/wiki/Property:P12764) *replaced synonym of* | yes |

### The vocabulary

**A basionym** is the name under which a taxon was *first* described, and which
therefore fixes the type specimen. When the genus is later reassigned, the
specific epithet is carried across and a **new combination** is published — the
basionym is retired, though it stays valid as a synonym. *Houpoea officinalis*
is a new combination; *Magnolia officinalis* is its basionym, the name the
species was originally described under. All 226 LOTUS compounds for that plant
are filed under the basionym.

**An original combination** is the binomial exactly as first published, before
any later reclassification. Its zoological mirror image is a **protonym** —
*original combination* and *protonym* are the botanical and zoological words for
the same idea, and Wikidata keeps a property for each. *Gonyaulax tamarensis*
is the original combination of *Alexandrium tamarense*, a dinoflagellate that
was moved between genera.

**A replacement name** — a ***nomen novum*** — exists because the old name
cannot be used at all. Usually the old name is a **homonym**, already taken by
a different taxon. *Salvia rosmarinus* replaced *Rosmarinus officinalis*, which
had turned out to be illegitimate.

**An accepted name** is the one a taxonomic authority currently endorses, and
its **synonyms** are the other names denoting the same taxon. This is the one
relationship with **no chronology**.

### Why accepted/synonym is not "old/new"

`P1420` links an accepted name to another name for the same taxon, and which of
the two is older is neither recorded nor implied. A synonym is *often* the older
name — that is usually why it stopped being used — but it can equally be a name
coined later and then found to be superfluous.

*Leontopodium nivale* and *Leontopodium alpinum* are joined by `P1420` in
Wikidata, and *nivale* is the **junior** name. Labelling that edge "old → new"
would be exactly backwards. So this pair is called *accepted / synonym* and the
other three are called *old / new*: they are different questions, and a single
"include synonyms" switch over all four cannot tell you which one you just
turned off.

### Why old/new is also not accepted/not-accepted

The two axes are deliberately kept apart. A basionym is usually a synonym now,
but under a different taxonomic opinion that very basionym may be the accepted
name again. And a new combination is sometimes itself a rejected synonym. So:

* **accepted vs. synonym** is a *taxonomic* judgement, and it can change;
* **old vs. new** is a *nomenclatural* fact, and it does not.

Neither determines the other, which is why Wikidata stores them under four
separate property pairs and why this tool offers four separate switches.

### Why `P1531` (*hybrid of*) is excluded

[`P1531`](https://www.wikidata.org/wiki/Property:P1531) looks like it belongs
here and deliberately does not. A hybrid is a **different organism with a
parent of its own** in the taxonomic tree; `P1531` records which parent it was
bred from — provenance, not identity. Every relationship above asserts that
two names denote the same taxon, which is the one thing that makes a compound
filed under one of them a compound from the other. Searching for
*Nepeta x catariensis* and receiving everything reported from *Nepeta cataria*
would answer a different question, and one no botanist would ask for by
accident. It is also *asymmetric in the wrong direction*: the hybrid is the
subject, so following it from a parent would pull in every garden hybrid ever
bred from that parent, compounding at every generation.

A test asserts `P1531` stays out, so adding it later is a deliberate act.

## Each switch changes a real answer

This is the reason the four are separate. For each taxon below, the bolded
column is the **one** switch that matters — turning it off loses compounds that
the other three would have found. Counts are distinct compounds, from the same
`counts_query` the UI's stat cards use.

| Taxon | accepted/synonym off | basionym off | original comb. off | replacement off | all four on |
| --- | --- | --- | --- | --- | --- |
| [Q178265](https://www.wikidata.org/wiki/Q178265) *Leontopodium nivale* | **0** | 33 | 33 | 33 | 33 |
| [Q92823857](https://www.wikidata.org/wiki/Q92823857) *Houpoea officinalis* | 226 | **0** | 226 | 226 | 226 |
| [Q104396170](https://www.wikidata.org/wiki/Q104396170) *Gonyaulax tamarensis* | 10 | 10 | **8** | 10 | 10 |
| [Q102240169](https://www.wikidata.org/wiki/Q102240169) *Salvia rosmarinus* | 304 | 304 | 304 | **25** | 304 |
| [Q122679](https://www.wikidata.org/wiki/Q122679) *Rosmarinus officinalis* | 304 | 304 | 304 | **302** | 304 |

Read across and each row is a different relationship doing the work:

* ***Leontopodium nivale*** has no compounds of its own and no `P171` path to
  any that do. It carries `P1420` to its accepted name *Leontopodium alpinum*,
  which has 33. Only the accepted/synonym switch finds them.
* ***Houpoea officinalis*** is a recent new combination for what was
  *Magnolia officinalis*, and every one of the 226 compounds is filed under the
  basionym. Only the basionym switch finds them — and with it off the search
  returns **nothing at all**, which is the same failure mode as the
  *Leontopodium* case, arrived at from a different direction.
* ***Gonyaulax tamarensis*** was moved to *Alexandrium tamarense*. Two of the
  ten compounds are filed under the new genus and the original combination adds
  them. The smallest effect of the four, and the reason to have a switch for it
  anyway: a user comparing two names of the same organism should not have to
  know that this particular reclassification is recorded as a protonym rather
  than a basionym.
* ***Salvia rosmarinus*** and ***Rosmarinus officinalis*** are the same plant,
  joined by both replacement properties. Off, they return 25 and 302 — disjoint
  halves of one organism. On, both return 304.

## The two names that were returning half a plant

*Salvia rosmarinus* (`Q102240169`) replaced *Rosmarinus officinalis*
(`Q122679`), and the LOTUS data is split along the seam: 31 compounds under the
new name, 595 under the old.

| | rows | taxa matched |
| --- | --- | --- |
| `Q122679` *Rosmarinus officinalis* — replacement off | 595 | *Rosmarinus officinalis* (595) |
| `Q122679` *Rosmarinus officinalis* — on | 626 | both, 595 + 31 |
| `Q102240169` *Salvia rosmarinus* — off | 31 | *Salvia rosmarinus* (31) |
| `Q102240169` *Salvia rosmarinus* — on | 626 | both, 595 + 31 |

With the switch on, **both searches return the same 626 rows over the same two
taxa** and both report 304 distinct compounds. Two spellings, one organism, one
answer.

The genus-level version of this is larger. `Rosmarinus` is nested inside
`Salvia` in the accepted classification, but `P171` alone does not connect them
— Wikidata carries it as a nomenclatural synonym (`P1420`/`P12763`) with a
subgenus node (`Q90595061`, *Salvia subg. Rosmarinus*) in between:

| | rows | compounds | taxa |
| --- | --- | --- | --- |
| *Salvia* — off | 7297 | 2131 | 216 |
| *Salvia* — on | 9440 | 2536 | 246 |
| *Rosmarinus* — off | 649 | 302 | 3 |
| *Rosmarinus* — on | 9440 | 2536 | 246 |

Off, `Rosmarinus` returns 3 taxa and `Salvia` returns 216, for a search that
looks identical in the input box. On, they return identical result sets, because
both closures reach the same 28 names. Stated plainly: with the accepted/synonym
switch on, *Salvia* and *Rosmarinus* are the same query, because in the current
taxonomy they are the same genus.

## The traversal, and why it is shaped this way

The query expands the seed in its own subquery, then descends from the result.
Only the *enabled* relationships contribute properties to the path:

```sparql
{
  SELECT DISTINCT ?root
  WHERE {
    VALUES ?seed { wd:Q102240169 }
    ?seed (wdt:P1420|wdt:P12763|wdt:P566|wdt:P12766|wdt:P1403|wdt:P12765|wdt:P694|wdt:P12764
          |^wdt:P1420|^wdt:P12763|^wdt:P566|^wdt:P12766|^wdt:P1403|^wdt:P12765|^wdt:P694|^wdt:P12764)* ?root .
  }
}
?t (wdt:P171*) ?root .
```

Three decisions are load-bearing.

**The expansion is a separate subquery, not part of the ancestry path.** The
obvious one-line spelling is to let the two closures interleave as
`?t (wdt:P171*|SYN*) wd:Q`. It is wrong for two reasons, and both cost time
rather than correctness. The endpoint evaluates a path from its endpoints, so
that form walks the entire `P171` tree *and* re-walks the nomenclatural closure
at every node it passes, instead of expanding one seed list and descending it
once. And semantically it makes the result a fixpoint of two relations applied
together, so a synonym of a *descendant* silently joins the result — whether
that is right is arguable, but that it was never chosen is not. Expanding the
seed first is both faster and a decision somebody made.

It is also **incomplete**, which settles the question. Measured on the broad
case the design has to survive (`Gentianales`, Q21754, 1457 taxa in the `P171`
closure):

| Form | Compounds |
| --- | --- |
| Interleaved `(P171*\|SYN*)` | 11009 |
| Seed subquery, then `P171*` | 11088 |

**The expansion is bounded.** `*` is transitive closure, which sounds alarming
on a graph where `P566` has 184775 triples. It is not: these properties connect
a taxon to *the other names of that same taxon*, not to a neighbour. Measured
across every taxon tested, the closure is 1–2 hops; *Salvia* is the largest at
28 names. It is deliberately not the same closure as `P171*`: crossing a name
link is a rename, crossing a parent link is descent into a different organism.

**Each property carries its own `^`.** Writing `^a|b` rather than `^a|^b` is a
silent bug — `^` binds to the single path after it, so it means "inverse of a,
or b" and everything after the first is read forwards only. The query still
returns rows and simply returns a different set, which is the worst shape a
query bug can take. A test asserts each enabled property appears both forward
and backward.

### Why the pair is always read in both directions

Wikidata stores each relationship from both ends, but a curator enters
whichever end they are looking at, and both are in active use.
*Rosmarinus officinalis* carries `P12764` toward *Salvia rosmarinus* while
*Salvia rosmarinus* carries `P694` back. A one-way read finds the newer name
from the older one and not the reverse.

`P12764` and `P694` are also not redundant with each other: there are 1583
`P12764` triples against 1313 `P694` ones, and some replacements carry only
`P12764` (*Clerodendrum* → *Siphonanthus*, *Nostocophycideae* →
*Nostocophycidae*, *Ceratium* → *Cylindrolobus*). A search reading only `P694`
would miss all of those, silently. Both travel together under one switch.

## One over-reach worth knowing about

*Salvia*'s closure also reaches *Mentha* (`Q47859`), which contributes 566 rows
via *Mentha spicata* and *Mentha piperita*. That is a data artefact rather than
a taxonomic claim: Wikidata records `Mentha` `P1420` `Audibertia`, and
separately records `Audibertia` `P12763` `Salvia`, so *Mentha* enters at closure
depth 2 through a genus that is a synonym of both. Each of the two statements is
individually defensible — *Audibertia* really has been treated as both — but
their composition is not.

It is left in rather than special-cased. Capping the closure at depth 1 does not
remove *Mentha* (it arrives by a different route at depth 1, via *Preslia* and
*Pulegium*), and a hand-maintained blocklist of genera that some curator once
cross-linked would be worse than the honest result: a silent exclusion nobody
could audit. It is documented here instead.

## Where each switch lives

| Surface | How |
| --- | --- |
| Model | `SearchCriteria::taxon_names`, a `TaxonNomenclature { accepted_synonyms, basionyms, protonyms, replacements }` — all `true` in `up_to_year` |
| Query | `compounds_by_taxon_query_with(qid, &Nomenclature)`, `structure_search_query_with(.., &Nomenclature)` |
| Search | `build_base_query` converts the criteria into a `Nomenclature` |
| Server | `taxon_accepted_synonyms`, `taxon_basionyms`, `taxon_protonyms`, `taxon_replacements` in the `SearchRequest` body; each absent means the default |
| URL | `?taxon_basionyms=false` and friends; each written only when off |
| CLI | `--no-accepted-synonyms`, `--no-basionyms`, `--no-protonyms`, `--no-replacements` |
| UI | four checkboxes grouped under the taxon field, all on |

`TaxonNomenclature` in the model is the single place that knows which boolean is
which relationship, and everything else delegates to it — `Nomenclature` in
`lotus-query` wraps it rather than keeping a parallel copy, and
`for_relation`/`set_for_relation` dispatch on the relation's `slot`. `Nomenclature`
adds `ALL_ON`, `ALL_OFF`, a `*_only()` constructor per relation and
`with`/`without` to flip one at a time. `Nomenclature::from(&criteria)` is the
single place the criteria and the query meet. Turning every switch off
reproduces the pre-feature query exactly.

## Re-running these numbers

The counts came from `lotus_query::counts_query` composed over
`lotus_query::compounds_by_taxon_query_with`, sent to the QLever Wikidata
endpoint, with one relation disabled at a time. The pair to compare is the third
and fourth columns: `n_compounds` and `n_taxa`.

Wikidata is a live graph, so these numbers drift as curation proceeds. The
*durable* part is the shape of the table: that for each of these four taxa a
different switch is the one that matters. The absolute counts are a snapshot.
