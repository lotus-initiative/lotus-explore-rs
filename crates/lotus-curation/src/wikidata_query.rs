// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The queries curation asks Wikidata.
//!
//! Pure string construction, so the queries are testable by comparison and the
//! web client and the CLI cannot drift apart on what a "compound" means. Every
//! one of these was extracted from the web client's curation code unchanged: two
//! copies of "find the item for this `InChIKey`" would eventually answer
//! differently, and the difference would be a curator submitting a duplicate.

use crate::{CURATION_SPARQL_PREFIXES, WD_CHEMICAL_COMPOUND_QID};

/// The Wikidata properties curation reads and writes.
pub mod property {
    /// Canonical SMILES.
    pub const CANONICAL_SMILES: &str = "P233";
    /// Isomeric SMILES, which preserves stereochemistry.
    pub const ISOMERIC_SMILES: &str = "P2017";
    /// `InChIKey`, the identifier a structure is matched on.
    pub const INCHIKEY: &str = "P235";
    /// `InChI`.
    pub const INCHI: &str = "P234";
    /// Molecular formula.
    pub const FORMULA: &str = "P274";
    /// Exact mass.
    pub const EXACT_MASS: &str = "P2067";
    /// Label, the compound's name.
    pub const LABEL: &str = "rdfs:label";
    /// "Found in taxon".
    pub const OCCURS_IN_TAXON: &str = "P703";
    /// "stated in", the reference a statement is sourced to.
    pub const STATED_IN: &str = "P248";
    /// Parent taxon, for resolving a binomial to its genus.
    pub const PARENT_TAXON: &str = "P171";
    /// DOI, the identifier a reference is matched on.
    pub const DOI: &str = "P356";
    /// Scientific name, as a taxon item is named.
    pub const SCIENTIFIC_NAME: &str = "P225";
    /// Instance of, which is how an item says what kind of thing it is.
    pub const INSTANCE_OF: &str = "P31";
}

/// Escape a value for a SPARQL string literal.
///
/// A literal is `"`-delimited, and `\` and `"` are the two characters that end
/// or continue it. A name containing either would otherwise change the shape of
/// the query rather than being looked up.
#[must_use]
pub fn escape_sparql_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// The QID out of a Wikidata entity URI, or `None` for anything else.
///
/// A match that came back as a literal rather than a URI is not a match, and
/// reading the literal as a QID would produce a statement about a nonexistent
/// item.
#[must_use]
pub fn qid_from_uri(value: &str) -> Option<&str> {
    value.rsplit('/').next().filter(|last| {
        last.starts_with('Q') && last.len() > 1 && last[1..].chars().all(|c| c.is_ascii_digit())
    })
}

/// The item for a structure, matched on its `InChIKey`.
///
/// `P235` rather than the SMILES: `InChIKey` is a hash of the normalised
/// structure, so two SMILES that differ only in atom order or ring numbering
/// are the same compound, and matching on SMILES would report the second as
/// absent. The `LIMIT 1` is there because Wikidata has a handful of items for
/// some structures and one is enough to know whether the compound is there.
#[must_use]
pub fn compound_by_inchikey_query(inchikey: &str) -> String {
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?compound ?canonical ?iso ?inchi ?formula ?mass WHERE {{\n  \
           ?compound wdt:{inchikey_prop} \"{value}\" .\n  \
           OPTIONAL {{ ?compound wdt:{canonical} ?canonical . }}\n  \
           OPTIONAL {{ ?compound wdt:{iso} ?iso . }}\n  \
           OPTIONAL {{ ?compound wdt:{inchi} ?inchi . }}\n  \
           OPTIONAL {{ ?compound wdt:{formula} ?formula . }}\n  \
           OPTIONAL {{ ?compound wdt:{mass} ?mass . }}\n\
         }} LIMIT 1",
        inchikey_prop = property::INCHIKEY,
        value = escape_sparql_string(inchikey),
        canonical = property::CANONICAL_SMILES,
        iso = property::ISOMERIC_SMILES,
        inchi = property::INCHI,
        formula = property::FORMULA,
        mass = property::EXACT_MASS,
    )
}

/// The item for a taxon name, or `None` if Wikidata has no such taxon.
///
/// Both name kinds are tried, because "Gentiana lutea" and "yellow gentian" are
/// both written in a spreadsheet and Wikidata stores them under different
/// properties: a scientific name is `P225`, a common name is `P1843`.
///
/// The item is restricted by `P31`, not by walking the taxonomy: the web client
/// resolves taxa this way and has for some time, and a second rule here would be
/// a third answer to "which item is this taxon". A taxon classified only by
/// `P105` and not by `P31` is missed by both, which is a known gap rather than a
/// difference between the two front-ends.
#[must_use]
pub fn taxon_by_name_query(name: &str) -> String {
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?taxon ?taxonName WHERE {{\n  \
           VALUES ?nameProperty {{ wdt:{scientific} wdt:{common} }}\n  \
           ?taxon wdt:P31 wd:{entity} ;\n         \
                 ?nameProperty ?taxonName .\n  \
           FILTER(LCASE(STR(?taxonName)) = LCASE(\"{value}\"))\n\
         }} LIMIT 1",
        scientific = "P225",
        common = "P1843",
        entity = crate::WD_TAXON_QID,
        value = escape_sparql_string(name),
    )
}

/// Whether a taxon name is a binomial, and so can be resolved on its own.
///
/// A genus on its own is a valid taxon but an ambiguous search: "Gentiana"
/// matches hundreds of species, and curating an occurrence against the genus
/// rather than the species is a data error rather than a near miss. The caller
/// reports this rather than guessing.
#[must_use]
pub fn is_binomial(name: &str) -> bool {
    let mut words = name.split_whitespace().filter(|w| !w.is_empty());
    let (Some(first), Some(second), None) = (words.next(), words.next(), words.next()) else {
        return false;
    };
    // A genus is capitalised; a specific epithet is not.
    first.chars().next().is_some_and(char::is_uppercase)
        && second.chars().next().is_some_and(char::is_lowercase)
}

/// The item for a reference DOI, or `None`.
///
/// A DOI is the right key for a reference: it is what the citation carries, and
/// it is what a curator pastes in from a paper.
#[must_use]
pub fn reference_by_doi_query(doi: &str) -> String {
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?ref WHERE {{\n  \
           ?ref wdt:{doi_prop} \"{value}\" .\n\
         }} LIMIT 1",
        doi_prop = property::DOI,
        value = escape_sparql_string(doi),
    )
}

/// Whether a compound already has an occurrence in a taxon.
///
/// `ASK`, not `SELECT`: the only thing a curator needs is the yes or no, and a
/// boolean answer is what tells them whether there is anything to submit. A
/// `SELECT` would return the same row for every compound that matches and
/// nothing for one that does not, which is the harder shape to read.
#[must_use]
pub fn has_occurrence_query(compound_qid: &str, taxon_qid: &str) -> String {
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         ASK {{ wd:{compound} wdt:{occurs} wd:{taxon} }}",
        compound = compound_qid,
        occurs = property::OCCURS_IN_TAXON,
        taxon = taxon_qid,
    )
}

/// The statements that would add a compound to Wikidata, given a structure.
///
/// `P31` is asserted before the structure, because an item with no class is not
/// findable by anything that filters on one, and a `QuickStatements` run stops
/// at the first failure -- so the statements that make the item *usable* come
/// before the ones that fill it in.
#[must_use]
pub fn create_compound_statements(
    name: &str,
    canonical_smiles: &str,
    isomeric_smiles: &str,
) -> String {
    format!(
        "CREATE\n  LAST|{label}|\"{name}\"\n  \
         LAST|P31|wd:{compound_class}\n  \
         LAST|{canonical}|\"{canonical_value}\"\n  \
         LAST|{isomeric}|\"{isomeric_value}\"",
        label = property::LABEL,
        name = crate::escape_quickstatements(name),
        compound_class = WD_CHEMICAL_COMPOUND_QID,
        canonical = property::CANONICAL_SMILES,
        canonical_value = crate::escape_quickstatements(canonical_smiles),
        isomeric = property::ISOMERIC_SMILES,
        isomeric_value = crate::escape_quickstatements(isomeric_smiles),
    )
}

#[cfg(test)]
#[path = "wikidata_query/tests.rs"]
mod tests;
