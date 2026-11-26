// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Reference and compound resolution queries (QID, DOI, `InChIKey`, label, alias, structure).

use super::compound_names::tagged_name_values;
use super::filters::sanitize_qid_or_sentinel;
use super::prefixes::{PREFIXES, PREFIXES_WITH_SKOS, PREFIXES_WITH_STRUCTURE};
use super::rewrite::{escape_sparql_string, escape_structure_literal};

/// Label (`@mul` preferred over `@en`) and SMILES (`P2017` preferred over `P233`)
/// bindings shared by all five compound lookup queries.
const COMPOUND_LOOKUP_PROPERTIES: &str = "\
  OPTIONAL { ?compound rdfs:label ?_label_en . FILTER(LANG(?_label_en) = \"en\") }
  OPTIONAL { ?compound rdfs:label ?_label_mul . FILTER(LANG(?_label_mul) = \"mul\") }
  BIND(COALESCE(?_label_mul, ?_label_en) AS ?compound_label)
  OPTIONAL { ?compound wdt:P233 ?_smiles_conn . }
  OPTIONAL { ?compound wdt:P2017 ?_smiles_iso . }
  BIND(COALESCE(?_smiles_iso, ?_smiles_conn) AS ?canonical_smiles)";

/// How many compounds one structure resolution may report.
const STRUCTURE_LOOKUP_LIMIT: usize = 5;

/// Resolve a structure to the Wikidata compound it is.
///
/// The one lookup the structure service answers, asked the narrowest question with an
/// answer: *is this structure a compound Wikidata has?* The cutoff is 1.0, so it returns
/// only for a structure matching an indexed compound exactly.
///
/// Runs for every structure input, including in an exact search, because the exact route
/// needs a QID and a structure is not one. Still much cheaper than the searches it
/// precedes: the service identifies one structure rather than ranking an index, and the
/// query that follows is an index scan on a single QID rather than over every candidate.
///
/// A miss is not an error here, unlike the other three lookups. A SMILES Wikidata does not
/// have is a good structure that is absent from the database, and the search
/// carries on with the reader's own structure. A name matching nothing is a claim about a
/// compound that does not exist; a structure is not a claim at all.
///
/// `LIMIT` is there because a structure can match more than one compound at 1.0 --
/// stereoisomers and salts of the same thing. The cap keeps resolution cheap and the caller
/// reports the extras as an ambiguity rather than hiding them.
#[must_use]
pub fn structure_compound_lookup_query(structure: &str) -> String {
    format!(
        "{PREFIXES_WITH_STRUCTURE}
SELECT DISTINCT (xsd:integer(STRAFTER(STR(?compound), \"Q\")) AS ?compound_qid) ?compound_label ?canonical_smiles ?matched_by
WHERE {{
  SERVICE idsm:wikidata {{
    ?compound sachem:similarCompoundSearch [
      sachem:query {literal};
      sachem:cutoff \"1\"^^xsd:double
    ].
  }}
{COMPOUND_LOOKUP_PROPERTIES}
  BIND(\"structure\" AS ?matched_by)
}}
LIMIT {STRUCTURE_LOOKUP_LIMIT}",
        literal = escape_structure_literal(structure),
    )
}

/// Resolve a DOI to the reference that carries it.
///
/// **The DOI is uppercased before it is asked for, because that is how Wikidata
/// stores them.** `?ref wdt:P356 "10.1002/andp.18280880206"` returns nothing;
/// `?ref wdt:P356 "10.1002/ANDP.18280880206"` returns the item, in about 0.3s on
/// `QLever`. A DOI is case-insensitive by specification, so lowercasing here would be as
/// wrong as uppercasing a chemical formula, and the failure is silent -- a lookup matching
/// nothing looks exactly like a DOI that does not exist.
///
/// Resolver prefixes are stripped first: `doi:10.1002/andp.18280880206` and
/// `https://doi.org/10.1002/andp.18280880206` are the same DOI, and both are things a reader
/// pastes out of a paper.
///
/// Deliberately bare: `SELECT ?ref WHERE {` with no `SERVICE` and no `OPTIONAL`. That is
/// the shape [`is_reference_lookup`](super::is_reference_lookup) recognises, routing it to
/// the WDQS scholarly subgraph when `QLever` is unreachable -- the one WDQS service that
/// answers `P356` quickly. An English label would break that detection, and a title is not
/// worth losing the fast route over: the rows carry the reference.
#[must_use]
pub fn reference_by_doi_query(doi: &str) -> String {
    format!(
        "{PREFIXES}
SELECT ?ref WHERE {{
  ?ref wdt:P356 \"{}\" .
}}",
        escape_sparql_string(&lotus_model::strip_doi_prefix(doi).to_ascii_uppercase())
    )
}

/// Resolve a Wikidata QID to the reference it names.
///
/// A `VALUES` and nothing to match, so it is as cheap as a compound lookup by QID.
/// Same bare shape as [`reference_by_doi_query`] so one parser and one
/// [`is_reference_lookup`](super::is_reference_lookup) detection cover both routes; a `VALUES`
/// is answered by any service, so the scholarly fallback is harmless here.
#[must_use]
pub fn reference_by_qid_query(qid: &str) -> String {
    // The whole QID goes in, prefix included: `wd:` takes a local name, so
    // `wd:Q23118` is the item and `wd:23118` is a name nothing is bound to.
    format!(
        "{PREFIXES}
SELECT ?ref WHERE {{
  VALUES ?ref {{ wd:{} }}
}}",
        sanitize_qid_or_sentinel(qid)
    )
}

/// Look a compound up by the QID the reader typed.
///
/// A QID is already an identifier, so this is the cheapest of the four lookups: a `VALUES`
/// and an optional label, no matching anywhere. It runs anyway, because a mistyped QID
/// otherwise produces an identity query matching nothing and an empty table, which reads as
/// "this compound has no results" rather than "that is not a compound". Confirming the item
/// exists turns one into the other, with the input quoted back.
///
/// Same shape as the other three, so all four return the same columns and the parser reads
/// them alike.
///
/// A lowercase `q` is accepted, as in the taxon field: `q18216` means Q18216.
#[must_use]
pub fn compound_by_qid_query(qid: &str) -> String {
    let qid = sanitize_qid_or_sentinel(qid);
    format!(
        "{PREFIXES}
SELECT (xsd:integer(STRAFTER(STR(?compound), \"Q\")) AS ?compound_qid) ?compound_label ?canonical_smiles ?matched_by
WHERE {{
  VALUES ?compound {{ wd:{qid} }}
{COMPOUND_LOOKUP_PROPERTIES}
  BIND(\"qid\" AS ?matched_by)
}}"
    )
}

/// Look a compound up by `InChIKey` (`P235`).
///
/// The one unambiguous identifier route: an `InChIKey`'s shape is fixed by the standard, so
/// a structure string cannot be mistaken for a name. Also the easiest of the three
/// name-ish lookups by a wide margin -- one indexed equality on a value unique in practice,
/// against a name lookup's `VALUES` over sixteen language tags (and casing variants) with a
/// fallback query behind it. Measured ~0.2s on `QLever`.
///
/// `P2017` / `P233` SMILES comes back alongside. An exact search does not need it, but
/// substructure and similarity ask about *other* compounds, and this is the structure they
/// must take to the service.
#[must_use]
pub fn compound_inchikey_query(inchikey: &str) -> String {
    format!(
        "{PREFIXES}
SELECT (xsd:integer(STRAFTER(STR(?compound), \"Q\")) AS ?compound_qid) ?compound_label ?canonical_smiles ?matched_by
WHERE {{
  VALUES ?name {{ \"{}\" }}
  ?compound wdt:P235 ?name .
{COMPOUND_LOOKUP_PROPERTIES}
  BIND(\"inchikey\" AS ?matched_by)
}}",
        escape_sparql_string(&inchikey.trim().to_ascii_uppercase())
    )
}

/// Look a compound up by its label (`rdfs:label`).
///
/// The shape is forced by measurement. Three ways to match a language-tagged literal,
/// measured against the live endpoint:
///
/// - `FILTER(LCASE(STR(?label)) = ...)` -- a scan of 17.9M labels. **Times out.**
/// - `?label ql:contains-word "..."` -- accepted by `QLever`, but the text index is not
///   loaded here, so it silently returns nothing.
/// - `VALUES ?name { "aspirin"@en "Aspirin"@en ... }` -- index-served, **~0.15–0.5s**. This one.
///
/// Hence the language tag and common casing variants are enumerated in `VALUES` rather than
/// discarded with `LCASE`, which is why language-tag and casing expansion helpers exist.
///
/// `?compound wdt:P235 ?_inchikey .` restricts matches to items carrying an `InChIKey` (the same
/// chemical-entity discriminator used by the main result queries), preventing capitalized names
/// such as `"Aspirin"`, `"Caffeine"`, `"Morphine"`, or `"Taxol"` from resolving to trade names,
/// scholarly articles, bands, songs, or disambiguation pages -- and letting `Taxol` fall through
/// to [`compound_alias_query`] where `paclitaxel` (`Q423762`) carries it as `skos:altLabel`.
///
/// `SELECT DISTINCT` collapses rows when several language tags or casing variants match the same
/// compound, `?compound_qid` is projected as an integer so the CSV carries a bare `Q…`, and
/// `STRSTARTS` keeps lexemes out: `?compound rdfs:label "aspirin"@en` also matches three
/// senses of the English lexeme, which are not compounds.
#[must_use]
pub fn compound_label_query(name: &str) -> String {
    compound_name_lookup_query(PREFIXES, "rdfs:label", "label", name)
}

/// Look a compound up by alias (`skos:altLabel`), on the terms of
/// [`compound_label_query`] (expanding casing variants via `VALUES`, requiring `wdt:P235`, and
/// projecting `SELECT DISTINCT`).
///
/// A separate query rather than a `UNION` with the label one, and the reason is
/// measured rather than stylistic: `QLever` will not push a `VALUES` down into both
/// arms of a union, so it scans each property instead. `label UNION altLabel`
/// **times out** where each alone answers in under half a second.
#[must_use]
pub fn compound_alias_query(name: &str) -> String {
    compound_name_lookup_query(PREFIXES_WITH_SKOS, "skos:altLabel", "alias", name)
}

fn compound_name_lookup_query(
    prefixes: &str,
    predicate: &str,
    matched_by: &str,
    name: &str,
) -> String {
    format!(
        "{prefixes}
SELECT DISTINCT (xsd:integer(STRAFTER(STR(?compound), \"Q\")) AS ?compound_qid) ?compound_label ?canonical_smiles ?matched_by
WHERE {{
  {values}
  ?compound {predicate} ?name .
  ?compound wdt:P235 ?_inchikey .
  FILTER(STRSTARTS(STR(?compound), \"http://www.wikidata.org/entity/Q\"))
{COMPOUND_LOOKUP_PROPERTIES}
  BIND(\"{matched_by}\" AS ?matched_by)
}}",
        values = tagged_name_values(name),
    )
}
