// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Query construction.
//!
//! Every query this crate builds is a `SELECT` over the LOTUS projection in
//! Wikidata, shaped as nested subqueries so that `QLever` can filter before it
//! enriches. The nesting is load-bearing and is asserted in
//! `tests/query_contract.rs`; those tests assert the builders' behaviour rather
//! than their bytes, so whitespace may change but structure may not.

use lotus_model::taxon_nomenclature::{self, Relation};
use lotus_model::{ElementState, SearchCriteria, SmilesSearchType, classify_structure};
use std::fmt::Write as _;

/// Which of the four nomenclatural relationships a taxon search follows.
///
/// Four independent booleans rather than one, because the four relationships
/// answer different questions — see [`lotus_nomenclature`] for why "old versus
/// new" and "accepted versus synonym" are two separate axes and not one.
///
/// [`lotus_nomenclature`]: lotus_model::taxon_nomenclature
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nomenclature {
    /// The four toggles, owned by the model.
    ///
    /// A wrapper rather than a parallel set of fields: there is one place in
    /// this workspace that knows which boolean is which relationship, and it is
    /// not here. Everything below delegates, so a fifth relationship is added
    /// to [`taxon_nomenclature::ALL`] and this wrapper picks it up.
    names: lotus_model::TaxonNomenclature,
}

impl Nomenclature {
    /// Every relationship followed. The default, and what a search gets when
    /// the user has not said otherwise.
    pub const ALL_ON: Self = Self {
        names: lotus_model::TaxonNomenclature::ALL_ON,
    };

    /// No relationship followed: the taxon name as typed, and nothing else.
    pub const ALL_OFF: Self = Self {
        names: lotus_model::TaxonNomenclature::ALL_OFF,
    };

    /// Only the accepted/synonym relationship, for a caller that wants
    /// taxonomic synonymy without the nomenclatural history.
    #[must_use]
    pub const fn accepted_synonyms_only() -> Self {
        Self::only(&taxon_nomenclature::ACCEPTED_SYNONYM)
    }

    /// Only the basionym relationship.
    #[must_use]
    pub const fn basionyms_only() -> Self {
        Self::only(&taxon_nomenclature::BASIONYM)
    }

    /// Only the original-combination (protonym) relationship.
    #[must_use]
    pub const fn protonyms_only() -> Self {
        Self::only(&taxon_nomenclature::PROTONYM)
    }

    /// Only the replacement-name relationship.
    #[must_use]
    pub const fn replacements_only() -> Self {
        Self::only(&taxon_nomenclature::REPLACEMENT)
    }

    const fn only(relation: &Relation) -> Self {
        let mut names = lotus_model::TaxonNomenclature::ALL_OFF;
        names.set_for_relation(relation, true);
        Self { names }
    }

    /// The same choice with one relationship switched off.
    ///
    /// The relationships are independent, so this is a normal operation rather
    /// than a special case: a user who wants accepted-name synonyms but not the
    /// nomenclatural history is asking a coherent question.
    #[must_use]
    pub const fn without(mut self, relation: &Relation) -> Self {
        self.names.set_for_relation(relation, false);
        self
    }

    /// The same choice with one relationship switched on.
    #[must_use]
    pub const fn with(mut self, relation: &Relation) -> Self {
        self.names.set_for_relation(relation, true);
        self
    }

    /// Accepted name ↔ its synonyms (`P1420` / `P12763`). Not chronological.
    /// New combination ↔ its basionym (`P566` / `P12766`).
    /// Current name ↔ its original combination, or protonym
    /// (`P1403` / `P12765`).
    /// Replacement name ↔ what it replaced (`P694` / `P12764`).
    /// Whether the given relationship is followed.
    #[must_use]
    pub const fn follows(&self, relation: &Relation) -> bool {
        self.names.for_relation(relation)
    }

    /// The enabled relationships, in the order
    /// [`taxon_nomenclature::ALL`] lists them.
    #[must_use]
    pub fn enabled_relations(&self) -> Vec<Relation> {
        taxon_nomenclature::ALL
            .into_iter()
            .filter(|relation| self.follows(relation))
            .collect()
    }

    /// Every property of every enabled relationship, deduplicated.
    ///
    /// Each relationship's two properties are distinct, so eight in the all-on
    /// case; the dedupe is here so that a relationship added with a property
    /// another one already uses cannot write it into the path twice.
    #[must_use]
    pub fn properties(&self) -> Vec<(&'static str, &'static str)> {
        let mut seen: Vec<(&'static str, &'static str)> = Vec::new();
        for relation in self.enabled_relations() {
            for pair in relation.properties() {
                if !seen.contains(&pair) {
                    seen.push(pair);
                }
            }
        }
        seen
    }
}

impl Default for Nomenclature {
    fn default() -> Self {
        Self::ALL_ON
    }
}

impl From<&SearchCriteria> for Nomenclature {
    fn from(criteria: &SearchCriteria) -> Self {
        Self::from(&criteria.taxon_names)
    }
}

impl From<&lotus_model::TaxonNomenclature> for Nomenclature {
    fn from(names: &lotus_model::TaxonNomenclature) -> Self {
        Self { names: *names }
    }
}

const PREFIXES: &str = "\
PREFIX xsd:    <http://www.w3.org/2001/XMLSchema#>
PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
PREFIX prov:   <http://www.w3.org/ns/prov#>
PREFIX wd:     <http://www.wikidata.org/entity/>
PREFIX wdt:    <http://www.wikidata.org/prop/direct/>
PREFIX p:      <http://www.wikidata.org/prop/>
PREFIX ps:     <http://www.wikidata.org/prop/statement/>
PREFIX pq:     <http://www.wikidata.org/prop/qualifier/>
PREFIX pr:     <http://www.wikidata.org/prop/reference/>
PREFIX wikibase: <http://wikiba.se/ontology#>
PREFIX schema: <http://schema.org/>
";

/// The base prefixes plus the two the structure-search service needs.
const PREFIXES_WITH_STRUCTURE: &str = "\
PREFIX xsd:    <http://www.w3.org/2001/XMLSchema#>
PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
PREFIX prov:   <http://www.w3.org/ns/prov#>
PREFIX wd:     <http://www.wikidata.org/entity/>
PREFIX wdt:    <http://www.wikidata.org/prop/direct/>
PREFIX p:      <http://www.wikidata.org/prop/>
PREFIX ps:     <http://www.wikidata.org/prop/statement/>
PREFIX pq:     <http://www.wikidata.org/prop/qualifier/>
PREFIX pr:     <http://www.wikidata.org/prop/reference/>
PREFIX wikibase: <http://wikiba.se/ontology#>
PREFIX schema: <http://schema.org/>
PREFIX sachem: <http://bioinfo.uochb.cas.cz/rdf/v1.0/sachem#>
PREFIX idsm:   <https://idsm.elixir-czech.cz/sparql/endpoint/>
";

/// The base prefixes plus `skos`, for the alias lookup.
const PREFIXES_WITH_SKOS: &str = "\
PREFIX xsd:    <http://www.w3.org/2001/XMLSchema#>
PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
PREFIX wd:     <http://www.wikidata.org/entity/>
PREFIX wdt:    <http://www.wikidata.org/prop/direct/>
PREFIX skos:   <http://www.w3.org/2004/02/skos/core#>
";

/// The names [`select_clause`] projects, in order.
///
/// The parser finds its columns by name, so this list and the projection have to
/// be the same list. They were kept in step by hand and by nothing else, and the
/// two copies are in different files: a projected name the parser does not know
/// is not an error, it is a column that silently arrives empty. So the names are
/// written down once here, and `the_projection_is_exactly_these_names` fails when
/// the text and the list disagree.
///
/// `compound_smiles_conn` and `compound_smiles_iso` are both here because both
/// are projected and the parser picks between them; a single coalesced column
/// would replace the pair rather than sit beside it.
pub const SELECT_COLUMNS: [&str; 15] = [
    "compound",
    "compoundLabel",
    "compound_inchikey",
    "compound_smiles_conn",
    "compound_smiles_iso",
    "compound_mass",
    "compound_formula",
    "taxon",
    "taxon_name",
    "ref_qid",
    "ref",
    "ref_title",
    "ref_doi",
    "ref_date",
    "statement",
];

/// The result columns. QIDs are projected as integers so that the CSV they come
/// back in is already the bare `Q…` form the parsers expect.
#[must_use]
fn select_clause() -> String {
    format!(
        r#"
SELECT DISTINCT
  (xsd:integer(STRAFTER(STR(?c), "Q")) AS ?compound)
  ?compoundLabel
  ?compound_inchikey
  ?compound_smiles_conn
  ?compound_smiles_iso
  ?compound_mass
  ({} AS ?compound_formula)
  (xsd:integer(STRAFTER(STR(?t), "Q")) AS ?taxon)
  ?taxon_name
  (xsd:integer(STRAFTER(STR(?r), "Q")) AS ?ref_qid)
  ?ref
  ?ref_title
  ?ref_doi
  ?ref_date
  ?statement
"#,
        normalize_digits_expr("?compound_formula_raw")
    )
}

/// The innermost projection: only what the occurrence itself asserts.
const CORE_VARS: &str =
    "?c ?compound_inchikey ?compound_smiles_conn ?t ?taxon_name ?r ?ref ?statement";

/// The middle projection: the core plus everything the display rows show.
const ENRICHED_VARS: &str = "\
?c ?compound_inchikey ?compound_smiles_conn
      ?compound_smiles_iso ?compound_mass ?compound_formula_raw
      ?compoundLabel
      ?t ?taxon_name
      ?r ?ref
      ?ref_title ?ref_doi ?ref_date
      ?statement";

const REFERENCE_METADATA: &str = "
  OPTIONAL { ?r wdt:P1476 ?ref_title. }
  OPTIONAL { ?r wdt:P356 ?ref_doi. }
  OPTIONAL { ?r wdt:P577 ?ref_date. }
";

const COMPOUND_PROPERTIES: &str = r#"
  OPTIONAL { ?c wdt:P2017 ?compound_smiles_iso. }
  OPTIONAL { ?c wdt:P2067 ?compound_mass. }
  OPTIONAL { ?c wdt:P274 ?compound_formula_raw. }
  OPTIONAL { ?c rdfs:label ?compoundLabelMul. FILTER(LANG(?compoundLabelMul) = "mul") }
  OPTIONAL { ?c rdfs:label ?compoundLabelEn. FILTER(LANG(?compoundLabelEn) = "en") }
  BIND(COALESCE(?compoundLabelMul, ?compoundLabelEn) AS ?compoundLabel)
"#;

/// Subscript digits, so that `C₁₇H₁₂O₇` and `C17H12O7` compare equal.
const SUBSCRIPTS: [(char, char); 10] = [
    ('₀', '0'),
    ('₁', '1'),
    ('₂', '2'),
    ('₃', '3'),
    ('₄', '4'),
    ('₅', '5'),
    ('₆', '6'),
    ('₇', '7'),
    ('₈', '8'),
    ('₉', '9'),
];

/// Every compound the LOTUS projection knows, across all taxa.
#[must_use]
pub fn all_compounds_query() -> String {
    compounds_query_with(None, None, Occurrence::Required)
}

/// Every compound, **including those with no occurrence statement**.
///
/// The nothing-provided case, and deliberately not the same query as
/// [`all_compounds_query`]. That one requires `P703`, so it answers "what has been
/// reported, and where" -- which is what `*` asks. This one does not, so it also returns
/// the compounds nobody has tied to an organism.
///
/// Before this existed the two were the same query, and the effect was that submitting
/// the form with an empty taxon box answered a question the reader had not asked.
#[must_use]
pub fn all_compounds_including_untaxonomised_query() -> String {
    compounds_query_with(None, None, Occurrence::Optional)
}

/// Compounds found in `taxon_qid` and its descendants (`P171*`), following
/// every nomenclatural relationship.
#[must_use]
pub fn compounds_by_taxon_query(taxon_qid: &str) -> String {
    compounds_by_taxon_query_with(taxon_qid, &Nomenclature::ALL_ON)
}

/// Compounds found in `taxon_qid` and its descendants, following the
/// nomenclatural relationships that `nomenclature` enables.
///
/// This is the form the rest of the workspace calls, because which
/// relationships to follow is a user-facing choice rather than a builder
/// detail. A `Nomenclature` with all four off produces exactly the query the
/// crate built before any of this existed.
///
/// [`Nomenclature`]: crate::Nomenclature
#[must_use]
pub fn compounds_by_taxon_query_with(taxon_qid: &str, nomenclature: &Nomenclature) -> String {
    compounds_query(None, Some((taxon_qid, nomenclature)))
}

/// Compounds in `taxon_qid` and its descendants, with the nomenclatural closure
/// already resolved by the caller.
///
/// The closure is the set of taxa the seed can be called -- what
/// [`compounds_by_taxon_query_with`] computes in a subquery. Supplying it costs
/// the caller a round trip and saves the endpoint roughly half the query's compute
/// on a wide search; the measurement is in `docs/SPARQL-VARIANTS.md`, and the
/// correctness requirement (both forms must return the same rows) is what
/// disqualified the faster interleaved alternative.
///
/// **Pass an empty slice to get the subquery form.** That is not a silent
/// fallback for a failed lookup: an empty `VALUES` list is a valid query that
/// matches nothing, so a resolution that returned nothing must not be turned into
/// an empty answer. Callers should treat "closure is empty" as "resolve it the
/// other way", which is exactly what passing an empty slice does.
///
/// Nothing in the workspace calls this yet. It exists so the decision in
/// `docs/sweep/REVIEW.md` section 16 is a one-line change at the call site rather
/// than a change to the query builder.
#[must_use]
pub fn compounds_by_taxon_query_with_resolved_closure(
    taxon_qid: &str,
    nomenclature: &Nomenclature,
    roots: &[String],
) -> String {
    compounds_query_with_closure(
        None,
        Some((taxon_qid, nomenclature)),
        Occurrence::Required,
        roots,
    )
}

/// One compound, by identity, with its occurrences.
///
/// This is what [`SmilesSearchType::Exact`] is built on, and the reason it is a
/// separate builder is that it does not call the structure service at all.
/// Everything the structure field holds is resolved to a Wikidata compound first,
/// and once it is, "this compound" is an identity question that Wikidata answers
/// with an index scan. Substructure and similarity both have to load a structure
/// index and score every candidate in it, which is a different order of magnitude
/// for a question whose answer is one row.
///
/// The occurrence triples stay `OPTIONAL`, for the same reason they do in the
/// taxon-less structure search: a compound with no occurrence data is precisely
/// the compound someone is looking for when they search by name, and requiring
/// one would hide it.
#[must_use]
pub fn exact_compound_query(
    compound_qid: &str,
    taxon_qid: Option<(&str, &Nomenclature)>,
) -> String {
    compounds_query(Some(compound_qid), taxon_qid)
}

/// Both seeds are optional and neither implies the other: a query with a
/// compound seed asks for that compound, and one with a taxon seed asks for
/// everything in that taxon. A structure search has the second; an exact search
/// has the first.
fn compounds_query(
    compound_seed: Option<&str>,
    taxon_qid: Option<(&str, &Nomenclature)>,
) -> String {
    compounds_query_with(compound_seed, taxon_qid, Occurrence::Required)
}

/// Whether an occurrence statement (`P703`) has to be present.
///
/// The two are different questions. `Required` asks what was reported and where;
/// `Optional` asks what exists, which includes the compounds nobody has tied to an
/// organism yet. Those are the `*` case and the nothing-provided case respectively, and
/// conflating them meant that asking for everything silently answered the narrower
/// question.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Occurrence {
    Required,
    Optional,
}

fn compounds_query_with(
    compound_seed: Option<&str>,
    taxon_qid: Option<(&str, &Nomenclature)>,
    occurrence: Occurrence,
) -> String {
    compounds_query_with_closure(compound_seed, taxon_qid, occurrence, &[])
}

fn compounds_query_with_closure(
    compound_seed: Option<&str>,
    taxon_qid: Option<(&str, &Nomenclature)>,
    occurrence: Occurrence,
    roots: &[String],
) -> String {
    let ancestry = taxon_qid.map_or_else(String::new, |(qid, nomenclature)| {
        format!(
            "\n          {}",
            taxon_ancestry_pattern_with_roots(qid, *nomenclature, roots)
        )
    });

    let core = compound_seed.map_or_else(
        || {
            // The `;` spelling is kept on the shared prefix: an existing contract test
            // asserts this exact fragment, and changing it would be a rewrite rather
            // than the addition this is.
            let compound = "?c wdt:P235 ?compound_inchikey ;\
          wdt:P233 ?compound_smiles_conn .";
            match occurrence {
                // Every taxon-less row is an occurrence: this query is about what was
                // reported where.
                Occurrence::Required => format!(
                    "{compound}\
          ?c p:P703 ?statement .\
          ?statement ps:P703 ?t ;\
                     prov:wasDerivedFrom ?ref .\
          ?ref pr:P248 ?r .\
          ?t wdt:P225 ?taxon_name .{ancestry}"
                ),
                // Nothing was asked for, so nothing is required. A compound with no
                // occurrence statement is not an error here; it is a compound whose
                // organism nobody has recorded, and excluding it answers a narrower
                // question than the one that was asked.
                //
                // Optional as one block rather than per triple, so a row is either a
                // complete occurrence -- taxon, reference and all -- or an
                // occurrence-less compound with empty cells. Optional per triple would
                // emit rows with a taxon but no reference, a shape the result store has
                // no way to represent.
                Occurrence::Optional => format!(
                    "OPTIONAL {{ \
            ?c p:P703 ?statement .\
            ?statement ps:P703 ?t ;\
                       prov:wasDerivedFrom ?ref .\
            ?ref pr:P248 ?r .\
            ?t wdt:P225 ?taxon_name .\
          }}{ancestry}"
                ),
            }
        },
        |qid| {
            // The occurrence triple becomes optional here, because a compound
            // with no occurrence data is exactly what a name search is looking
            // for. See the doc comment on `exact_compound_query`.
            //
            // The block is wrapped in its own subquery, and the seed repeated
            // inside it, because `OPTIONAL` costs this endpoint twenty times
            // what the same patterns cost on their own. Measured on Q3613679:
            // 4.3s as a bare `OPTIONAL`, 0.2s as a seeded subquery, for the same
            // 20 rows. The block is what makes it slow, not the patterns --
            // `?c p:P703 ?statement . ?statement ps:P703 ?t` and the three hops
            // after it run in 0.1s outside an `OPTIONAL` -- so the subquery is
            // there to hand the planner a left side of its own size instead of
            // one it has to assume. Without the repeated `VALUES` inside, the
            // subquery inherits the binding and the slow plan comes back.
            let seeded = escape_sparql_string(qid);
            format!(
                "VALUES ?c {{ wd:{seeded} }}
          OPTIONAL {{
            {{ SELECT * WHERE {{
              VALUES ?c {{ wd:{seeded} }}
              ?c p:P703 ?statement .\
              ?statement ps:P703 ?t ;\
                         prov:wasDerivedFrom ?ref .\
              ?ref pr:P248 ?r .\
              ?t wdt:P225 ?taxon_name .{ancestry}
            }} }}
          }}"
            )
        },
    );

    format!(
        "{PREFIXES}
{}
WHERE {{
  {{
    SELECT
      {ENRICHED_VARS}
    WHERE {{
      {{
        SELECT {CORE_VARS}
        WHERE {{
          {core}
        }}
      }}
      {REFERENCE_METADATA}
      {COMPOUND_PROPERTIES}
    }}
  }}
}}",
        select_clause(),
    )
}

/// The nomenclatural closure of a taxon, as a property path over the
/// properties of the enabled relationships in [`lotus_model::taxon_nomenclature`].
///
/// Read in **both** directions and closed transitively with `*`.
///
/// The bidirectional read is not optional. Wikidata stores each relationship
/// from both ends, but a curator enters whichever end they are looking at, and
/// both are in active use: `Rosmarinus officinalis` carries `P12764` toward
/// `Salvia rosmarinus`, while `Salvia rosmarinus` carries `P694` back. A search
/// that read only one direction would find the newer name from the older one
/// and not the reverse, which is the same bug in each of the four relations.
///
/// The `*` is bounded in practice by the shape of the graph. These properties
/// connect a taxon to the *other names of that same taxon*, so the closure is a
/// handful of items, not a subtree — measured at 1 to 2 hops on every taxon
/// tested. It is deliberately **not** the same closure as `P171*`: crossing
/// from a name to a name is a rename, whereas crossing a parent link is
/// descent into a different organism.
///
/// # Why this is a separate subquery and not one big path
///
/// The obvious spelling is to let the two closures interleave, as
/// `?t (wdt:P171*|SYN*) wd:Q` — one path, and no subquery to reason about. It
/// is wrong here for two reasons, and both cost time rather than correctness:
///
/// 1. The endpoint evaluates a path from its endpoints, so `(P171*|SYN*)`
///    anchored at `wd:Q` walks the whole `P171` tree *and* re-walks the
///    nomenclatural closure at every node it passes, instead of expanding one
///    seed list and then descending it once. On `Gentianales` (Q21754) that is
///    11009 compounds against 11088 for the subquery form, reached more slowly.
/// 2. Interleaving makes the result a fixpoint of two relations applied
///    together, which means a synonym of a *descendant* silently joins the
///    result. Whether that is right is arguable; that it is a decision nobody
///    chose deliberately is not.
///
/// So the seed is expanded first, on its own, and only the resulting handful of
/// QIDs is handed to `P171*`. The expansion subquery is cheap because it is
/// evaluated against one constant.
fn nomenclature_path(nomenclature: Nomenclature) -> Option<String> {
    let mut alternatives: Vec<String> = Vec::new();
    for (property, _) in nomenclature.properties() {
        alternatives.push(property.to_string());
    }
    if alternatives.is_empty() {
        return None;
    }
    // The inverses are appended afterwards, each with its own `^`.
    //
    // Writing `^a|b` instead of `^a|^b` would be a silent bug: `^` binds to
    // the single path that follows it, so that is "inverse of a, or b", and
    // the properties after the first would be read forwards only. The expansion
    // still returns rows, so the query looks fine and returns a different set —
    // the worst shape a query bug can take.
    for (property, _) in nomenclature.properties() {
        alternatives.push(format!("^{property}"));
    }
    Some(format!("({})*", alternatives.join("|")))
}

/// The taxon filter for the innermost subquery: a descendant of any taxon in the
/// nomenclatural closure of `qid`.
///
/// With every relationship off this is the `P171*` path the query has always
/// used, unchanged, so a search that does not want them costs exactly what it
/// used to.
fn taxon_ancestry_pattern(qid: &str, nomenclature: Nomenclature) -> String {
    taxon_ancestry_pattern_with_roots(qid, nomenclature, &[])
}

/// The taxon filter, optionally from a closure resolved before the query was built.
///
/// `roots` is the seed's nomenclatural closure -- the taxa the seed can be called,
/// which [`nomenclature_path`] would otherwise compute in a subquery. Passing them
/// in trades that subquery for a `VALUES` list, which is the cheaper of the two by
/// about half on the widest search this app can produce; see `docs/SPARQL-VARIANTS.md`
/// for the measurement and for why the faster *interleaved* alternative is not
/// used (it answers a different question, losing ~79 of 11,087 distinct compounds).
///
/// **Empty means "not resolved", and that is load-bearing.** An empty `VALUES` is a
/// valid query that matches nothing, so a caller who resolved the closure and got
/// nothing back would silently get an empty result rather than the full subtree.
/// Falling back to the subquery makes the failure mode "slower" instead of
/// "wrong", which is the only acceptable trade for a missing value.
fn taxon_ancestry_pattern_with_roots(
    qid: &str,
    nomenclature: Nomenclature,
    roots: &[String],
) -> String {
    let escaped = escape_sparql_string(qid);
    let Some(path) = nomenclature_path(nomenclature) else {
        return format!("?t (wdt:P171*) wd:{escaped} .");
    };

    // Resolved, and resolved to something.
    if !roots.is_empty() {
        let values = roots
            .iter()
            .map(|root| format!("wd:{}", escape_sparql_string(root)))
            .collect::<Vec<_>>()
            .join(" ");
        return format!("VALUES ?root {{ {values} }}\n          ?t (wdt:P171*) ?root .");
    }
    format!(
        "{{\n            SELECT DISTINCT ?root\n            WHERE {{\n              VALUES ?seed {{ wd:{escaped} }}\n              ?seed {path} ?root .\n            }}\n          }}\n          ?t (wdt:P171*) ?root ."
    )
}

/// A `VALUES` lookup of taxa by scientific name (`P225`).
///
/// An exact match, with no `FILTER` and no `LCASE`: `VALUES` is served from an
/// index, and a case-folding scan of every label is the one thing that makes
/// this slow. This is the path every taxon search takes first, so it stays the
/// fast one — the common-name fallback is a second round trip precisely so this
/// one does not have to become a scan.
///
/// Known limit: `wdt:` keeps only the preferred-rank statement, so the 293
/// `P225` values that are not preferred are invisible here. A taxon has one
/// accepted scientific name, so in practice this is a rounding error, but it is
/// a real gap rather than a guarantee and it is written down rather than
/// discovered. See [`taxon_common_name_lookup_query`] for the far larger
/// equivalent gap on `P1843`.
#[must_use]
pub fn taxon_lookup_query(name: &str) -> String {
    format!(
        "PREFIX wdt: <http://www.wikidata.org/prop/direct/>
SELECT ?taxon ?taxon_name ?matched_by
WHERE {{
  VALUES ?taxon_name {{ \"{}\" }}
  ?taxon wdt:P225 ?taxon_name .
  BIND(\"scientific\" AS ?matched_by)
}}",
        escape_sparql_string(name)
    )
}

/// A lookup of taxa by **common** name (`P1843`).
///
/// The fallback for [`taxon_lookup_query`], and it cannot be shaped like it.
/// Three things force a different query, each of which was found by a name that
/// returned nothing when it should not have:
///
/// **The statement path, not `wdt:`.** `wdt:P1843` collapses a taxon to its
/// preferred-rank value. *Gentiana lutea* (`Q158572`) carries 65 common names
/// and `wdt:` returns exactly one of them — so a search for `bitterwort`, which
/// is one of the other 64, found nothing. `p:P1843/ps:P1843` returns all 65.
/// The same collapse costs `P225` 293 values out of four million, which is why
/// the scientific path is left on the fast index and only `P1843` pays for the
/// statement walk.
///
/// **The language tag has to go.** `P225` values are untagged literals, so a
/// bare `"Gentiana lutea"` matches. Every `P1843` value is tagged — `bitterwort`
/// is `bitterwort@en` — and a bare literal is not equal to a tagged one in
/// SPARQL. Matching `STR()` against the lexical form drops the tag, which is the
/// point: a common name is the word a reader reaches for in *their* language, so
/// restricting the match to `@en` would break the feature for most of its users.
///
/// **Case has to go too.** `P1843` stores whatever a curator typed, so the same
/// taxon carries `bitterwort`, `Common wormwood` and `Bijvoet`. `LCASE` on both
/// sides makes the search indifferent to that.
///
/// The cost is a scan of every `P1843` statement — measured at ~2.5s on
/// `QLever`, against ~0.3s for the indexed scientific lookup. That is why this is
/// a fallback rather than a second branch of the first query: a search that hits
/// a scientific name never pays it.
#[must_use]
pub fn taxon_common_name_lookup_query(name: &str) -> String {
    format!(
        "PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX p: <http://www.wikidata.org/prop/>
PREFIX ps: <http://www.wikidata.org/prop/statement/>
SELECT ?taxon ?taxon_name ?matched_by
WHERE {{
  ?taxon p:P1843/ps:P1843 ?taxon_name .
  FILTER(LCASE(STR(?taxon_name)) = LCASE(\"{}\"))
  BIND(\"common\" AS ?matched_by)
}}",
        escape_sparql_string(name)
    )
}

/// Language tags a compound's name is looked up under.
///
/// A `rdfs:label` or `skos:altLabel` is language-tagged, so matching one means
/// matching the tag too -- see [`compound_label_query`] for why that cannot be
/// worked around. This is therefore a list rather than a wildcard, and it is a
/// deliberate trade: these are the languages a chemical name is most likely to
/// be typed in, and a name in one of the remaining ~280 will not resolve.
///
/// Ordered roughly by how much chemistry is published in each, so truncating the
/// list costs the least.
const COMPOUND_NAME_LANGUAGES: [&str; 15] = [
    "en", "de", "fr", "es", "nl", "it", "pt", "sv", "pl", "ru", "tr", "ja", "zh", "la", "cs",
];

/// The `VALUES` rows for `name` under every `COMPOUND_NAME_LANGUAGES` tag.
fn tagged_name_values(name: &str) -> String {
    let escaped = escape_sparql_string(name);
    let rows = COMPOUND_NAME_LANGUAGES
        .iter()
        .map(|tag| format!("\"{escaped}\"@{tag}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("VALUES ?name {{ {rows} }}")
}

/// Resolve a structure to the Wikidata compound it is.
///
/// The one lookup the structure service answers, and it is asked the narrowest
/// question that has an answer: *is this structure a compound Wikidata has?* The
/// cutoff is 1.0, so it only comes back for a structure that matches an indexed
/// compound exactly.
///
/// This runs for every structure input, including in an exact search, because the
/// exact route needs a QID and a structure is not one. It is still much cheaper
/// than the searches it precedes: the service is asked to identify one structure
/// rather than to rank an index, and the query that follows is an index scan on a
/// single QID rather than a scan of every candidate compound.
///
/// A miss is not an error here, unlike the other three lookups. A SMILES that
/// Wikidata does not have is a perfectly good structure — it is simply not a
/// compound in the database, and the search carries on with the reader's own
/// structure. That is the asymmetry with a name: a name that matches nothing is a
/// claim about a compound that does not exist, and a structure is not a claim at
/// all.
///
/// `LIMIT` is there because a structure can match more than one compound at 1.0 —
/// stereoisomers and salts of the same thing. The cap keeps the resolution cheap;
/// the caller reports the extras as an ambiguity rather than hiding them.
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
  OPTIONAL {{ ?compound rdfs:label ?compound_label . FILTER(LANG(?compound_label) = \"en\") }}
  OPTIONAL {{ ?compound wdt:P233 ?canonical_smiles . }}
  BIND(\"structure\" AS ?matched_by)
}}
LIMIT {STRUCTURE_LOOKUP_LIMIT}",
        literal = escape_structure_literal(structure),
    )
}

/// How many compounds one structure resolution may report.
const STRUCTURE_LOOKUP_LIMIT: usize = 5;

/// Resolve a DOI to the reference that carries it.
///
/// **The DOI is uppercased before it is asked for, because that is how Wikidata
/// stores them.** `?ref wdt:P356 "10.1002/andp.18280880206"` returns nothing;
/// `?ref wdt:P356 "10.1002/ANDP.18280880206"` returns the item, in about 0.3s on
/// `QLever`. A DOI is case-insensitive by specification, so lowercasing here would
/// be as wrong as uppercasing a chemical formula, and the failure is silent —
/// a lookup that matches nothing looks exactly like a DOI that does not exist.
///
/// Resolver prefixes are stripped first, since `doi:10.1002/andp.18280880206` and
/// `https://doi.org/10.1002/andp.18280880206` are the same DOI and both are things a
/// reader pastes out of a paper.
///
/// Deliberately bare: a `SELECT ?ref WHERE {` with no `SERVICE` and no `OPTIONAL`.
/// That is the shape [`is_reference_lookup`] recognises, which is what routes it to
/// the WDQS scholarly subgraph when `QLever` is unreachable — the one WDQS service
/// that answers `P356` quickly. Adding an English label would break that
/// detection, and a title is not worth losing the fast route over: the rows that
/// come back have the reference in them.
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
/// [`is_reference_lookup`] detection cover both routes; a `VALUES` is answered by
/// any service, so the scholarly fallback is harmless here.
#[must_use]
pub fn reference_by_qid_query(qid: &str) -> String {
    // The whole QID goes in, prefix included: `wd:` takes a local name, so
    // `wd:Q23118` is the item and `wd:23118` is a name nothing is bound to.
    format!(
        "{PREFIXES}
SELECT ?ref WHERE {{
  VALUES ?ref {{ wd:{} }}
}}",
        escape_sparql_string(&qid.trim().to_ascii_uppercase())
    )
}

/// Look a compound up by the QID the reader typed.
///
/// A QID is already an identifier, so this is the cheapest of the four lookups: a
/// `VALUES` and an optional label, with no matching anywhere. It runs anyway,
/// for one reason — a mistyped QID otherwise produces an identity query that
/// matches nothing and returns an empty table, which reads as "this compound has
/// no results" rather than "that is not a compound". Confirming the item exists
/// turns one of those into the other, with the input quoted back.
///
/// The same shape as the other three, so all four return the same columns and the
/// parser reads them the same way.
///
/// A lowercase `q` is accepted, the way the taxon field accepts one: a reader who
/// types `q18216` means Q18216.
#[must_use]
pub fn compound_by_qid_query(qid: &str) -> String {
    let qid = qid.trim();
    let uppercase = qid.to_ascii_uppercase();
    format!(
        "{PREFIXES}
SELECT (xsd:integer(STRAFTER(STR(?compound), \"Q\")) AS ?compound_qid) ?compound_label ?canonical_smiles ?matched_by
WHERE {{
  VALUES ?compound {{ wd:{uppercase} }}
  OPTIONAL {{ ?compound rdfs:label ?compound_label . FILTER(LANG(?compound_label) = \"en\") }}
  OPTIONAL {{ ?compound wdt:P233 ?canonical_smiles . }}
  BIND(\"qid\" AS ?matched_by)
}}"
    )
}

/// Look a compound up by `InChIKey` (`P235`).
///
/// The one unambiguous identifier route: an `InChIKey`'s shape is fixed by the
/// standard, so there is no question of a structure string being mistaken for a
/// name. And it is the easiest of the three name-ish lookups by a wide margin —
/// one indexed equality on a value that is unique in practice, against a name
/// lookup's `VALUES` over fifteen language tags with a fallback query behind it.
///
/// `P233` (canonical SMILES) comes back alongside. An exact search of the result
/// does not need it, but a substructure or similarity search does: those ask
/// about *other* compounds, and this is the structure they have to go to the
/// service with.
///
/// Indexed and cheap: measured at ~0.2s on `QLever`.
#[must_use]
pub fn compound_inchikey_query(inchikey: &str) -> String {
    format!(
        "{PREFIXES}
SELECT (xsd:integer(STRAFTER(STR(?compound), \"Q\")) AS ?compound_qid) ?compound_label ?canonical_smiles ?matched_by
WHERE {{
  VALUES ?name {{ \"{}\" }}
  ?compound wdt:P235 ?name .
  OPTIONAL {{ ?compound rdfs:label ?compound_label . FILTER(LANG(?compound_label) = \"en\") }}
  OPTIONAL {{ ?compound wdt:P233 ?canonical_smiles . }}
  BIND(\"inchikey\" AS ?matched_by)
}}",
        escape_sparql_string(inchikey)
    )
}

/// Look a compound up by its label (`rdfs:label`).
///
/// The shape is forced by measurement. Three ways to match a language-tagged
/// literal were tried against the live endpoint:
///
/// - `FILTER(LCASE(STR(?label)) = ...)` — a scan of 17.9M labels. **Times out.**
/// - `?label ql:contains-word "..."` — accepted by `QLever`, but the text index is
///   not loaded on this endpoint, so it silently returns nothing.
/// - `VALUES ?name { "aspirin"@en ... }` — index-served, **~0.15s**. This one.
///
/// So the language tag has to be enumerated rather than discarded, which is why
/// `COMPOUND_NAME_LANGUAGES` exists.
///
/// `?compound_qid` is projected as an integer so the CSV carries a bare `Q…`,
/// and `STRSTARTS` keeps lexemes out: `?compound rdfs:label "aspirin"@en` also
/// matches three senses of the English lexeme, which are not compounds.
#[must_use]
pub fn compound_label_query(name: &str) -> String {
    format!(
        "{PREFIXES}
SELECT (xsd:integer(STRAFTER(STR(?compound), \"Q\")) AS ?compound_qid) ?compound_label ?canonical_smiles ?matched_by
WHERE {{
  {values}
  ?compound rdfs:label ?name .
  FILTER(STRSTARTS(STR(?compound), \"http://www.wikidata.org/entity/Q\"))
  OPTIONAL {{ ?compound rdfs:label ?compound_label . FILTER(LANG(?compound_label) = \"en\") }}
  OPTIONAL {{ ?compound wdt:P233 ?canonical_smiles . }}
  BIND(\"label\" AS ?matched_by)
}}",
        values = tagged_name_values(name),
    )
}

/// Look a compound up by alias (`skos:altLabel`), on the terms of
/// [`compound_label_query`].
///
/// A separate query rather than a `UNION` with the label one, and the reason is
/// measured rather than stylistic: `QLever` will not push a `VALUES` down into both
/// arms of a union, so it scans each property instead. `label UNION altLabel`
/// **times out** where each alone answers in under a quarter of a second.
#[must_use]
pub fn compound_alias_query(name: &str) -> String {
    format!(
        "{PREFIXES_WITH_SKOS}
SELECT (xsd:integer(STRAFTER(STR(?compound), \"Q\")) AS ?compound_qid) ?compound_label ?canonical_smiles ?matched_by
WHERE {{
  {values}
  ?compound skos:altLabel ?name .
  FILTER(STRSTARTS(STR(?compound), \"http://www.wikidata.org/entity/Q\"))
  OPTIONAL {{ ?compound rdfs:label ?compound_label . FILTER(LANG(?compound_label) = \"en\") }}
  OPTIONAL {{ ?compound wdt:P233 ?canonical_smiles . }}
  BIND(\"alias\" AS ?matched_by)
}}",
        values = tagged_name_values(name),
    )
}

/// A structure search through the IDSM/Sachem service.
///
/// This is the route for [`SmilesSearchType::Substructure`] and
/// [`SmilesSearchType::Similarity`], and — for input that resolved to no compound
/// at all — for [`SmilesSearchType::Exact`] too. Exact has its own builder,
/// [`exact_compound_query`], which is cheaper and is used whenever a QID is
/// available.
///
/// Without `taxon_qid` the occurrence triple is optional, so a compound with
/// no occurrence data is still returned — structure search is how such
/// compounds are found in the first place.
#[must_use]
pub fn structure_search_query(
    structure: &str,
    search: SmilesSearchType,
    threshold: f64,
    taxon_qid: Option<&str>,
) -> String {
    structure_search_query_with(
        structure,
        search,
        threshold,
        taxon_qid,
        &Nomenclature::ALL_ON,
    )
}

/// [`structure_search_query`], with the nomenclatural expansion of the taxon
/// filter under the caller's control.
///
/// The choice means the same thing here as it does for
/// [`compounds_by_taxon_query_with`]: a structure search is run *within* a
/// taxon, and the taxon's nomenclatural closure is the same set of organisms
/// whether the compound was found by name or by substructure.
#[must_use]
pub fn structure_search_query_with(
    structure: &str,
    search: SmilesSearchType,
    threshold: f64,
    taxon_qid: Option<&str>,
    nomenclature: &Nomenclature,
) -> String {
    let literal = escape_structure_literal(structure);
    let is_multiline = literal.starts_with("'''");

    // A structure only reaches this function when it did *not* resolve to a
    // Wikidata compound; an input that named one is answered by
    // `exact_compound_query` without a service at all. Nothing that reaches here
    // has an identity to search for, so the substructure service is the only thing
    // that can answer. Documented rather than silent: see
    // `docs/STRUCTURE-SEARCH.md`.
    let service = match search {
        // `Exact` is answered by `exact_compound_query` before it gets here. It
        // reaches this function only when the input named nothing, so the closest
        // answer the service can give is the same molecule: an identical
        // fingerprint, which is a similarity search at a cutoff of 1.
        SmilesSearchType::Exact => format!(
            "SERVICE idsm:wikidata {{
    ?c sachem:similarCompoundSearch [
      sachem:query {literal};
      sachem:cutoff \"1\"^^xsd:double
    ].
  }}"
        ),
        SmilesSearchType::Substructure if is_multiline => format!(
            "SERVICE idsm:wikidata {{
    [ sachem:compound ?c; sachem:score ?_sachem_score ]
      sachem:scoredSubstructureSearch [
        sachem:query {literal};
        sachem:searchMode sachem:substructureSearch;
        sachem:chargeMode sachem:defaultChargeAsAny;
        sachem:isotopeMode sachem:ignoreIsotopes;
        sachem:aromaticityMode sachem:aromaticityDetectIfMissing;
        sachem:stereoMode sachem:ignoreStereo;
        sachem:tautomerMode sachem:ignoreTautomers;
        sachem:radicalMode sachem:ignoreSpinMultiplicity;
        sachem:topn \"-1\"^^xsd:integer;
        sachem:internalMatchingLimit \"1000000\"^^xsd:integer
      ].
  }}"
        ),
        SmilesSearchType::Similarity => format!(
            "SERVICE idsm:wikidata {{
    ?c sachem:similarCompoundSearch [
      sachem:query {literal};
      sachem:cutoff \"{threshold}\"^^xsd:double
    ].
  }}"
        ),
        SmilesSearchType::Substructure => format!(
            "SERVICE idsm:wikidata {{
    ?c sachem:substructureSearch [
      sachem:query {literal}
    ].
  }}"
        ),
    };

    // With a taxon, the service runs in its own subquery so that `QLever` can
    // pre-filter before enriching, and the occurrence triple is *required* —
    // a match outside the requested taxon should not come back.
    //
    // Without one, the service joins the body directly and the occurrence
    // triple is `OPTIONAL`: a structure search is how a compound with no
    // occurrence data gets found in the first place, and requiring a taxon
    // would hide exactly those.
    let body = taxon_qid.map_or_else(
        || {
            format!(
                "
  {service}

  ?c wdt:P235 ?compound_inchikey ;
     wdt:P233 ?compound_smiles_conn .

  OPTIONAL {{
    ?c p:P703 ?statement .
    ?statement ps:P703 ?t ;
               prov:wasDerivedFrom ?ref .
    ?ref pr:P248 ?r .
    ?t wdt:P225 ?taxon_name .
    {REFERENCE_METADATA}
  }}

  {COMPOUND_PROPERTIES}
"
            )
        },
        |qid| {
            format!(
                "
  {{
    SELECT DISTINCT ?c
    WHERE {{
      {service}
    }}
  }}

  ?c wdt:P235 ?compound_inchikey ;
     wdt:P233 ?compound_smiles_conn .
  ?c p:P703 ?statement .
  ?statement ps:P703 ?t ;
             prov:wasDerivedFrom ?ref .
  ?ref pr:P248 ?r .
  ?t wdt:P225 ?taxon_name .
  {}

  {REFERENCE_METADATA}
  {COMPOUND_PROPERTIES}
",
                taxon_ancestry_pattern(qid, *nomenclature)
            )
        },
    );

    format!(
        "{PREFIXES_WITH_STRUCTURE}
{}
WHERE {{
{body}
}}",
        select_clause(),
    )
}

/// Wrap a `SELECT` in a `COUNT`, keeping the filters but dropping everything the
/// count does not depend on.
///
/// The display `OPTIONAL`s are stripped, and that is the point: `rdfs:label`
/// with two `FILTER(LANG(…))` passes is the most expensive thing in the query,
/// and a count that walked it would be slow enough to be rate-limited. The
/// variables it leaves unbound project as `NULL`, which the `COUNT`s ignore.
///
/// The deletion is by literal text, so it is only correct because nothing a
/// filter needs lives inside the two blocks being deleted. [`with_filters`] binds
/// what a filter needs outside them: `?c wdt:P2067 ?compound_mass .` for a mass
/// filter, `?r wdt:P577 ?ref_date .` for a year one, and `?c wdt:P274
/// ?compound_formula_raw .` for a formula one. A filter reading a variable bound
/// only inside a deleted block would survive here as a `FILTER` over an unbound
/// variable and count nothing at all -- which is what the formula filter did
/// until the third of those bindings was added, and every count on the page read
/// zero for any search that used one.
#[must_use]
pub fn counts_query(base: &str) -> String {
    let Some(select_at) = base.find("SELECT") else {
        return base.to_string();
    };
    let prefixes = &base[..select_at];
    let stripped = base[select_at..]
        .replace(REFERENCE_METADATA, "")
        .replace(COMPOUND_PROPERTIES, "");

    format!(
        "{prefixes}
SELECT
  (COUNT(*) AS ?n_entries)
  (COUNT(DISTINCT CONCAT(
    STR(?compound), \"\\u001F\", COALESCE(STR(?taxon), \"\"), \"\\u001F\", COALESCE(STR(?ref_qid), \"\")
  )) AS ?n_entries_unique)
  (COUNT(DISTINCT ?compound) AS ?n_compounds)
  (COUNT(DISTINCT ?taxon) AS ?n_taxa)
  (COUNT(DISTINCT ?ref_qid) AS ?n_references)
WHERE {{
  {{
    {}
  }}
}}",
        stripped.trim()
    )
}

/// Append a `LIMIT`.
#[must_use]
pub fn limit_query(base: &str, limit: usize) -> String {
    format!("{}\nLIMIT {limit}", base.trim_end())
}

/// Apply the active filters to a base query.
///
/// The fragments go after the base query's closing brace, not inside the
/// innermost subquery. That is where `inject` puts them and where the endpoint
/// has always been asked to evaluate them; moving them would change the query
/// plan, and the plan is tuned.
#[must_use]
pub fn with_filters(base: &str, criteria: &SearchCriteria, year_max: u16) -> String {
    let mut required = String::new();
    let mut filters = String::new();

    if criteria.has_mass_filter() {
        let _ = write!(
            filters,
            "FILTER(?compound_mass >= {:.6} && ?compound_mass <= {:.6})",
            criteria.mass_min, criteria.mass_max
        );
        let _ = writeln!(required, "?c wdt:P2067 ?compound_mass .");
    }

    if criteria.has_year_filter(year_max) {
        let _ = write!(
            filters,
            "FILTER(YEAR(?ref_date) >= {} && YEAR(?ref_date) <= {})",
            criteria.year_min, criteria.year_max
        );
        let _ = writeln!(required, "?r wdt:P577 ?ref_date .");
    }

    // A reference constrains which *reported* compounds are wanted, so it binds
    // `?r` rather than filtering a projected value -- the same shape the year
    // filter above uses, and for the same reason: `?r` is the item the reference
    // is.
    //
    // It is a `VALUES` and not a `FILTER(?r = wd:Q…)` because that is the
    // indexed form, and because an unbound `?r` -- a compound with no occurrence,
    // which a structure search exists to find -- simply fails to join rather than
    // comparing against an error.
    if !criteria.reference.trim().is_empty() {
        let _ = writeln!(
            required,
            "VALUES ?r {{ wd:{} }}",
            escape_sparql_string(criteria.reference.trim())
        );
    }

    formula_filter(criteria, &mut required, &mut filters);

    // One buffer decides this, not both. Every branch above writes to `required`
    // and `filters` together or to neither, so they cannot disagree about whether a
    // filter was added, and testing the second one is a second opinion about a
    // question the first already answered.
    //
    // The `debug_assert` is the invariant that makes it safe, and it is there
    // because the two are written independently: a branch that filtered without
    // binding, or bound without filtering, would leave a filter that silently
    // never applies rather than one that fails loudly.
    debug_assert!(
        filters.is_empty() || !required.is_empty(),
        "a filter branch must bind what it filters on, or every row is dropped"
    );
    if required.is_empty() {
        return base.to_string();
    }

    // The base query's own closing brace becomes the filters' closing brace, so
    // the fragments land in its outermost WHERE rather than in a new block.
    let Some(without_closing_brace) = trimmed_strip_closing_brace(base) else {
        return format!("{base}\n{required}{filters}");
    };
    format!("{without_closing_brace}\n{required}{filters}\n}}")
}

fn trimmed_strip_closing_brace(base: &str) -> Option<&str> {
    base.trim_end().strip_suffix('}')
}

/// The formula filter: normalise the formula once, then count elements against
/// the normalised string.
///
/// Takes `required` as well as `out` because the filter needs a binding that has
/// to survive into the counts query, and the counts query is built by deleting
/// text from this one -- see [`counts_query`].
fn formula_filter(criteria: &SearchCriteria, required: &mut String, out: &mut String) {
    if !criteria.has_formula_filter() {
        return;
    }

    // Without the BOUND guard, an unbound `?compound_formula_raw` makes every
    // comparison below evaluate against an error rather than a value.
    //
    // The variable names here are part of the query text, and a query's bytes
    // are what a cache key and a shared link are derived from, so they are not
    // renamed for tidiness.
    //
    // This triple is the reason the counts were wrong. `?compound_formula_raw` is
    // bound by an `OPTIONAL` inside `COMPOUND_PROPERTIES`, and `counts_query`
    // deletes that whole block because it is expensive and display-only -- except
    // that when a formula filter is active it is neither display-only nor
    // optional. The counts query kept `FILTER(BOUND(?compound_formula_raw))` over
    // a variable nothing bound, so it matched no rows and every card on the page
    // read zero for any search with a formula filter. Re-binding it here, outside
    // the deleted block and in the same way the mass and year filters bind their
    // own variables, is what makes it survive.
    let _ = writeln!(required, "?c wdt:P274 ?compound_formula_raw .");

    let _ = writeln!(out, "FILTER(BOUND(?compound_formula_raw))");
    let _ = writeln!(out, "BIND(STR(?compound_formula_raw) AS ?_formula_raw)");
    let _ = writeln!(
        out,
        r#"BIND(REPLACE(?_formula_raw, " ", "") AS ?_formula_nospace)"#
    );
    let _ = writeln!(
        out,
        "BIND({} AS ?_formula_norm)",
        normalize_digits_expr("?_formula_nospace")
    );
    // Split before each capital so element symbols become addressable tokens.
    let _ = writeln!(
        out,
        r#"BIND(REPLACE(?_formula_norm, "([A-Z])", "|$1") AS ?_formula_tokens)"#
    );

    // Every BIND first, then every FILTER: a group pattern's FILTERs see the
    // BINDs regardless of order, but interleaving them makes the query harder
    // to read and to diff.
    let mut ranged = Vec::new();
    for (symbol, min, max, default_max) in criteria.element_ranges() {
        if min > 0 || max < default_max {
            let var = bind_element_count(out, symbol);
            ranged.push(format!("FILTER({var} >= {min} && {var} <= {max})"));
        }
    }

    let mut halogens = Vec::new();
    for (symbol, state) in criteria.halogen_states() {
        if state == ElementState::Allowed {
            continue;
        }
        let var = bind_element_count(out, symbol);
        halogens.push(match state {
            ElementState::Required => format!("FILTER({var} > 0)"),
            ElementState::Excluded => format!("FILTER({var} = 0)"),
            ElementState::Allowed => unreachable!("the `Allowed` case is skipped above"),
        });
    }

    for filter in ranged.iter().chain(&halogens) {
        let _ = writeln!(out, "{filter}");
    }

    if let Some(exact) = normalized_exact_formula(criteria) {
        let _ = writeln!(out, "FILTER(?_formula_norm = \"{exact}\")");
    }
}

/// The exact formula to compare against, with subscripts folded to ASCII and
/// whitespace removed. The comparison happens against the endpoint's normalised
/// formula, so a subscript in the input would never match.
fn normalized_exact_formula(criteria: &SearchCriteria) -> Option<String> {
    let normalized: String = criteria
        .formula_exact
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '₀' => '0',
            '₁' => '1',
            '₂' => '2',
            '₃' => '3',
            '₄' => '4',
            '₅' => '5',
            '₆' => '6',
            '₇' => '7',
            '₈' => '8',
            '₉' => '9',
            other => other,
        })
        .collect();
    (!normalized.is_empty()).then(|| escape_sparql_string(&normalized))
}

/// A `BIND` yielding the count of `symbol` in the tokenised formula, or `0`.
/// Append a `BIND` yielding `symbol`'s count in the tokenised formula, or `0`,
/// and return the variable it binds.
///
/// The token separator is emitted as `\\|` because the pattern goes into a SPARQL
/// string literal, where a backslash is itself escaped. That is the pattern the
/// endpoint has always been sent, and changing it changes what matches.
fn bind_element_count(out: &mut String, symbol: &str) -> String {
    let var = format!("?_count_{}", symbol.to_ascii_lowercase());
    let pattern = format!(r"\\|{symbol}([0-9]*)(\\||$)");
    let captured = format!(r#"REPLACE(?_formula_tokens, ".*{pattern}.*", "$1")"#);
    let _ = writeln!(
        out,
        "BIND(IF(REGEX(?_formula_tokens, \"{pattern}\"), IF(STRLEN({captured}) = 0, 1, xsd:integer({captured})), 0) AS {var})"
    );
    var
}

/// Prepare a `SELECT` for export in `format`.
#[must_use]
pub fn export_query(select: &str, as_rdf: bool) -> String {
    if as_rdf {
        construct_from_select(select)
    } else {
        select.to_string()
    }
}

/// Rewrite a `SELECT` as a `CONSTRUCT` that emits the provenance graph.
#[must_use]
pub fn construct_from_select(select: &str) -> String {
    let Some(select_at) = select.find("SELECT") else {
        return select.to_string();
    };
    let Some(where_at) = select[select_at..].find("WHERE") else {
        return select.to_string();
    };
    let where_at = select_at + where_at;
    let prefixes = &select[..select_at];
    // Splice in the source's whole trailing block, minus its final brace: the
    // formula BIND goes inside it, and the brace is re-emitted below.
    let where_block = select[where_at..].trim();
    let Some(where_block) = where_block.strip_suffix('}') else {
        return select.to_string();
    };

    // `where_block` already begins with `WHERE`, so only the CONSTRUCT template
    // goes above it; the formula BIND is appended inside that same block.
    format!(
        "{prefixes}
CONSTRUCT {{
  ?c wdt:P235 ?compound_inchikey .
  ?c wdt:P233 ?compound_smiles_conn .
  ?c wdt:P2017 ?compound_smiles_iso .
  ?c wdt:P2067 ?compound_mass .
  ?c wdt:P274 ?compound_formula .
  ?c rdfs:label ?compoundLabel .
  ?c p:P703 ?statement .
  ?statement ps:P703 ?t ;
             prov:wasDerivedFrom ?ref .
  ?ref pr:P248 ?r .
  ?t wdt:P225 ?taxon_name .
  ?r wdt:P1476 ?ref_title .
  ?r wdt:P356 ?ref_doi .
  ?r wdt:P577 ?ref_date .
}}
{where_block}
  BIND({} AS ?compound_formula)
}}
",
        normalize_digits_expr("?compound_formula_raw")
    )
}

/// Which Wikidata service a fallback query needs.
///
/// This is a routing decision about the query, not about the network, so it
/// belongs here rather than in the crate that holds the URLs. Naming the
/// service instead of the endpoint also means the URLs stay overridable: a
/// caller running its own mirror picks the URL and still gets the right
/// rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackService {
    /// The main `WDQS` endpoint.
    Main,
    /// The scholarly subgraph, which answers `P356` quickly.
    Scholarly,
}

/// Route a query to `WDQS` after `QLever` failed, returning the service to use
/// and the rewritten query.
///
/// Reference lookups go to the scholarly subgraph, which is the only WDQS
/// service that answers P356 quickly; everything else goes to the main
/// endpoint with its reference `OPTIONAL`s moved inside a `SERVICE` block, so
/// that the bibliographic properties are still fetched.
#[must_use]
pub fn wdqs_fallback(query: &str) -> (FallbackService, String) {
    const SCHOLARLY_REF: &str = "
  SERVICE <https://query-scholarly.wikidata.org/sparql> {
    OPTIONAL { ?r wdt:P1476 ?ref_title. }
    OPTIONAL { ?r wdt:P356 ?ref_doi. }
    OPTIONAL { ?r wdt:P577 ?ref_date. }
  }
";

    if is_reference_lookup(query) {
        (
            FallbackService::Scholarly,
            query.replace("{CURATION_SPARQL_PREFIXES}\n", ""),
        )
    } else {
        let rewritten = query
            .replace(
                REFERENCE_METADATA,
                &REFERENCE_METADATA.replace("?r ", "?ref "),
            )
            .replace(REFERENCE_METADATA, SCHOLARLY_REF);
        (FallbackService::Main, rewritten)
    }
}

/// A bare `SELECT ?ref … wdt:P356` with no `SERVICE` or `OPTIONAL`, i.e. a
/// DOI-to-reference lookup.
#[must_use]
pub fn is_reference_lookup(query: &str) -> bool {
    query.contains("SELECT ?ref WHERE {")
        && query.contains("wdt:P356")
        && !query.contains("SERVICE")
        && !query.contains("OPTIONAL")
}

/// A nested `REPLACE` chain folding subscript digits to ASCII.
#[must_use]
pub fn normalize_digits_expr(var: &str) -> String {
    SUBSCRIPTS
        .iter()
        .fold(format!("STR({var})"), |acc, (from, to)| {
            format!(r#"REPLACE({acc}, "{from}", "{to}")"#)
        })
}

/// Escape a value for a SPARQL string literal.
#[must_use]
pub fn escape_sparql_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// Escape a structure for a SPARQL literal: triple-quoted if it spans lines
/// (a molfile), double-quoted otherwise.
#[must_use]
pub fn escape_structure_literal(structure: &str) -> String {
    let normalized = structure.replace("\r\n", "\n").replace('\r', "\n");
    let is_molfile = classify_structure(&normalized).is_molfile();
    let body = if is_molfile {
        normalized
    } else {
        normalized.trim().to_string()
    };

    let escaped = body.replace('\\', r"\\");
    if is_molfile || body.contains('\n') {
        format!("'''{escaped}'''")
    } else {
        let quoted = escaped.replace('"', "\\\"");
        format!("\"{quoted}\"")
    }
}
