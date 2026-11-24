// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Top-level public base `SELECT` query builders (non-structure shapes).

use super::Nomenclature;
use super::builders_body::{Occurrence, compounds_query_with, compounds_query_with_closure};

/// Every compound the LOTUS projection knows, across all taxa.
#[must_use]
pub fn all_compounds_query() -> String {
    compounds_query_with(None, None, Occurrence::Required)
}

/// Every compound, **including those with no occurrence statement**.
///
/// The nothing-provided case, deliberately not the same query as
/// [`all_compounds_query`]. That one requires `P703`, so it answers "what has been
/// reported, and where", which is what `*` asks. This one also returns the compounds nobody
/// has tied to an organism: while the two were the same query, an empty taxon box answered
/// a question the reader had not asked.
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
/// The form the rest of the workspace calls, because which relationships to follow is a
/// user-facing choice rather than a builder detail. A `Nomenclature` with all four off
/// produces exactly the pre-nomenclature query.
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
/// [`compounds_by_taxon_query_with`] computes in a subquery. Supplying it costs the caller a
/// round trip and saves the endpoint roughly half the query's compute on a wide search
/// (`docs/SPARQL-VARIANTS.md`); the correctness requirement that both forms return the same
/// rows is what disqualified the faster interleaved alternative.
///
/// **Pass an empty slice to get the subquery form.** Not a silent fallback for a failed
/// lookup: an empty `VALUES` list is a valid query matching nothing, so a resolution that
/// returned nothing must not become an empty answer. Treat "closure is empty" as "resolve
/// it the other way", which is what passing an empty slice does.
///
/// Nothing calls this yet; it exists so adopting it is a one-line change at the call site.
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
/// What [`SmilesSearchType::Exact`](lotus_model::SmilesSearchType::Exact) is built on,
/// separate because it does not call the structure service at all. The structure field is
/// resolved to a Wikidata compound first, and then "this compound" is an identity question
/// answered by an index scan; substructure and similarity must load a structure index and
/// score every candidate, a different order of magnitude for a question whose answer is
/// one row.
///
/// Occurrence triples stay `OPTIONAL` for the same reason as in the taxon-less structure
/// search: a compound with no occurrence data is precisely what a name search looks for,
/// and requiring one would hide it.
#[must_use]
pub fn exact_compound_query(
    compound_qid: &str,
    taxon_qid: Option<(&str, &Nomenclature)>,
) -> String {
    exact_compounds_query(std::slice::from_ref(&compound_qid.to_owned()), taxon_qid)
}

/// The same question for **every** compound a structure resolved to.
///
/// A structure input names a molecule, and the structure service answers with everything
/// within the cutoff: for `C[C@H](O)CO` that is the (R), (S) and achiral (RS) compounds.
/// Asking about only the first asks an arbitrary question -- measured, the first happens to
/// be the achiral form, which Wikidata has no occurrence of, so the search returned nothing
/// while the compound actually typed had six rows.
///
/// All of them are therefore asked at once, in one request: a `VALUES` list over `?c` rather
/// than a seed of one. Occurrences of the (R) and (S) forms are different facts about
/// different items and both belong in the answer.
///
/// Empty is not a special case: it means the structure named nothing, and the caller sends
/// it down the service path instead.
#[must_use]
pub fn exact_compounds_query(
    compound_qids: &[String],
    taxon_qid: Option<(&str, &Nomenclature)>,
) -> String {
    compounds_query(Some(compound_qids), taxon_qid)
}

/// Both seeds are optional and neither implies the other: a query with a
/// compound seed asks for that compound, and one with a taxon seed asks for
/// everything in that taxon. A structure search has the second; an exact search
/// has the first.
fn compounds_query(
    compound_seed: Option<&[String]>,
    taxon_qid: Option<(&str, &Nomenclature)>,
) -> String {
    compounds_query_with(compound_seed, taxon_qid, Occurrence::Required)
}

/// Select the base 15-column `SELECT` query for `criteria` given normalized
/// `structure` text and an optional resolved `taxon_qid` (`Some("*")` means any
/// recorded occurrence).
#[must_use]
pub fn base_query_for_criteria(
    criteria: &lotus_model::SearchCriteria,
    structure: &str,
    taxon_qid: Option<&str>,
) -> String {
    let nomenclature = Nomenclature::from(criteria);
    if !structure.is_empty() {
        return super::structure_search_query_with(
            structure,
            criteria.structure_search,
            criteria.structure_threshold,
            taxon_qid,
            &nomenclature,
        );
    }
    match taxon_qid {
        Some(qid) if qid != "*" => compounds_by_taxon_query_with(qid, &nomenclature),
        Some(_) => all_compounds_query(),
        // Nothing given is not `*`: without a taxon or reference filter, keep
        // untaxonomised compounds; `*` or a reference asks for reported occurrences.
        None if criteria.taxon.trim() == "*" || !criteria.reference.trim().is_empty() => {
            all_compounds_query()
        }
        None => all_compounds_including_untaxonomised_query(),
    }
}
