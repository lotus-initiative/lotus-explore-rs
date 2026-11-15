// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The queries curation asks Wikidata.
//!
//! Pure string construction, so the queries are testable by comparison and the
//! web client and the CLI cannot drift apart on what a "compound" means. Every
//! one of these was extracted from the web client's curation code unchanged: two
//! copies of "find the item for this `InChIKey`" would eventually answer
//! differently, and the difference would be a curator submitting a duplicate.

use crate::{CURATION_SPARQL_PREFIXES, WD_CHEMICAL_COMPOUND_QID, WD_TYPE_CHEMICAL_ENTITY_QID};

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
    /// English label in `QuickStatements` syntax.
    pub const LABEL: &str = "Len";
    /// "Found in taxon".
    pub const OCCURS_IN_TAXON: &str = "P703";
    /// DOI, the identifier a reference is matched on.
    pub const DOI: &str = "P356";
    /// Scientific name, as a taxon item is named.
    pub const SCIENTIFIC_NAME: &str = "P225";
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
         }} ORDER BY ?compound LIMIT 1",
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
/// When `name` carries a botanical/mycological author citation (for example
/// `"Cortinarius subtortus (Pers.) Fr."`), the stripped canonical name is also
/// matched because Wikidata `P225` rarely includes author strings.
///
/// The item is restricted by `P31`, not by walking the taxonomy: the web client
/// resolves taxa this way and has for some time, and a second rule here would be
/// a third answer to "which item is this taxon". A taxon classified only by
/// `P105` and not by `P31` is missed by both, which is a known gap rather than a
/// difference between the two front-ends.
#[must_use]
pub fn taxon_by_name_query(name: &str) -> String {
    let trimmed = name.trim();
    let filter_expr = match extract_canonical_taxon_name(trimmed) {
        Some(canonical) if canonical != trimmed => format!(
            "LCASE(STR(?taxonName)) = LCASE(\"{canonical_val}\") || LCASE(STR(?taxonName)) = LCASE(\"{raw_val}\")",
            canonical_val = escape_sparql_string(&canonical),
            raw_val = escape_sparql_string(trimmed),
        ),
        _ => format!(
            "LCASE(STR(?taxonName)) = LCASE(\"{value}\")",
            value = escape_sparql_string(trimmed),
        ),
    };
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?taxon ?taxonName WHERE {{\n  \
           VALUES ?nameProperty {{ wdt:{scientific} wdt:{common} }}\n  \
           ?taxon wdt:P31 wd:{entity} ;\n         \
                 ?nameProperty ?taxonName .\n  \
           FILTER({filter_expr})\n\
         }} ORDER BY ?taxon LIMIT 1",
        scientific = "P225",
        common = "P1843",
        entity = crate::WD_TAXON_QID,
    )
}

/// Extracts a canonical binomial or infraspecific scientific name.
///
/// Strips trailing botanical/mycological author citations and normalises
/// infraspecific rank abbreviations (`ssp.` -> `subsp.`, `forma` -> `f.`).
/// Returns `None` for bare genera (`"Gentiana"`), uncapitalised genera
/// (`"gentiana lutea"`), open nomenclature (`"Gentiana sp."`, `"Gentiana cf. lutea"`),
/// or malformed lowercase trailing words (`"Homo sapiens extra"`).
#[must_use]
pub fn extract_canonical_taxon_name(name: &str) -> Option<String> {
    let tokens: Vec<&str> = name.split_whitespace().filter(|w| !w.is_empty()).collect();
    let first = *tokens.first()?;
    let mut hybrid = false;
    let genus_clean = first.strip_prefix('×').map_or(first, |rest| {
        hybrid = true;
        rest
    });
    if !is_valid_genus(genus_clean) {
        return None;
    }

    let mut idx = 1usize;
    if tokens.get(idx).copied() == Some("×") {
        hybrid = true;
        idx += 1;
    }
    let epithet_raw = *tokens.get(idx)?;
    let epithet = epithet_raw.strip_prefix('×').map_or(epithet_raw, |rest| {
        hybrid = true;
        rest
    });
    if !is_valid_epithet(epithet) {
        return None;
    }

    let genus_out = if first.starts_with('×') {
        first.to_string()
    } else {
        genus_clean.to_string()
    };
    let base = if hybrid && !genus_out.starts_with('×') {
        format!("{genus_out} × {epithet}")
    } else {
        format!("{genus_out} {epithet}")
    };

    let rest = tokens.get(idx + 1..).unwrap_or_default();
    if rest.is_empty() {
        return Some(base);
    }

    // Either an infraspecific rank immediately follows the epithet, or an
    // author citation comes first (possibly followed by an infraspecific rank).
    if let Some((rank, infra)) = parse_infraspecific_tail(rest) {
        return Some(format!("{base} {rank} {infra}"));
    }
    parse_after_author_citation(&base, rest)
}

fn is_valid_genus(tok: &str) -> bool {
    let mut chars = tok.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_uppercase()
        && tok.len() >= 2
        && chars.all(|c| c.is_ascii_lowercase() || c == '-')
        && !is_open_nomenclature(tok)
}

fn is_valid_epithet(tok: &str) -> bool {
    let mut chars = tok.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|c| c.is_ascii_lowercase() || c == '-')
        && !is_open_nomenclature(tok)
}

fn is_open_nomenclature(tok: &str) -> bool {
    matches!(
        tok.trim_end_matches(',').to_ascii_lowercase().as_str(),
        "sp" | "sp."
            | "spp"
            | "spp."
            | "cf"
            | "cf."
            | "aff"
            | "aff."
            | "indet"
            | "indet."
            | "sect"
            | "sect."
            | "subg"
            | "subg."
            | "gr"
            | "gr."
            | "agg"
            | "agg."
            | "nr"
            | "nr."
            | "s.l"
            | "s.l."
            | "s.s"
            | "s.s."
            | "sensu"
    )
}

fn normalize_infraspecific_rank(tok: &str) -> Option<&'static str> {
    match tok.to_ascii_lowercase().as_str() {
        "subsp." | "ssp." | "subsp" | "ssp" => Some("subsp."),
        "var." | "var" => Some("var."),
        "f.sp." => Some("f. sp."),
        "f." | "forma" => Some("f."),
        "subvar." => Some("subvar."),
        "subf." => Some("subf."),
        _ => None,
    }
}

fn starts_with_author_token(tok: &str, next: Option<&str>) -> bool {
    tok.starts_with('(')
        || tok.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        || (matches!(
            tok.to_ascii_lowercase().as_str(),
            "van" | "von" | "de" | "da" | "del" | "du" | "ex" | "et" | "in"
        ) && next.is_some_and(|n| {
            n.starts_with('(') || n.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        }))
}

fn parse_infraspecific_tail<'a>(rest: &[&'a str]) -> Option<(&'static str, &'a str)> {
    let first = *rest.first()?;
    let (rank, infra, after) = if first.eq_ignore_ascii_case("f.")
        && rest.get(1).is_some_and(|s| s.eq_ignore_ascii_case("sp."))
    {
        ("f. sp.", *rest.get(2)?, rest.get(3..).unwrap_or_default())
    } else {
        (
            normalize_infraspecific_rank(first)?,
            *rest.get(1)?,
            rest.get(2..).unwrap_or_default(),
        )
    };
    if !is_valid_epithet(infra) {
        return None;
    }
    if after.is_empty() || starts_with_author_token(after.first()?, after.get(1).copied()) {
        Some((rank, infra))
    } else {
        None
    }
}

fn parse_after_author_citation(base: &str, rest: &[&str]) -> Option<String> {
    let first = *rest.first()?;
    if !starts_with_author_token(first, rest.get(1).copied()) {
        return None;
    }
    for idx in 1..rest.len() {
        let Some(slice) = rest.get(idx..) else {
            break;
        };
        if let Some((rank, infra)) = parse_infraspecific_tail(slice) {
            return Some(format!("{base} {rank} {infra}"));
        }
    }
    Some(base.to_string())
}

/// Whether a taxon name resolves to a species binomial or infraspecific taxon
/// (with optional botanical/mycological author citation stripped).
///
/// A genus on its own (`"Gentiana"`) or open nomenclature (`"Gentiana sp."`) is
/// ambiguous and reported rather than guessed.
#[must_use]
pub fn is_binomial(name: &str) -> bool {
    extract_canonical_taxon_name(name).is_some()
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
         }} ORDER BY ?ref LIMIT 1",
        doi_prop = property::DOI,
        value = escape_sparql_string(doi),
    )
}

/// Whether a compound already has an occurrence in a taxon.
///
/// `ASK`, not `SELECT`: the only thing a curator needs is the yes or no, and a
/// boolean answer is what tells them whether there is anything to submit. A
/// `SELECT` would return the same row for every compound that matches and
/// nothing for one that does not, which is the harder shape to read. Because
/// truthy `wdt:P703` triples exclude deprecated-rank statements, deprecated
/// occurrences do not suppress submission.
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

/// Whether a compound already has an occurrence in a taxon stated in a specific
/// reference (`P248` on the `P703` statement's reference node), excluding
/// `wikibase:DeprecatedRank` statements.
#[must_use]
pub fn has_occurrence_with_ref_query(compound_qid: &str, taxon_qid: &str, ref_qid: &str) -> String {
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         ASK {{\n  \
           wd:{compound} p:{occurs} ?stmt .\n  \
           ?stmt ps:{occurs} wd:{taxon} ;\n        \
                 wikibase:rank ?rank ;\n        \
                 prov:wasDerivedFrom ?refnode .\n  \
           ?refnode pr:P248 wd:{reference} .\n  \
           FILTER(?rank != wikibase:DeprecatedRank)\n\
         }}",
        compound = compound_qid,
        occurs = property::OCCURS_IN_TAXON,
        taxon = taxon_qid,
        reference = ref_qid,
    )
}

/// The statements that would add a compound to Wikidata, given a structure.
///
/// The item is an instance of "type of chemical entity" (`P31 Q113145171`) and
/// a subclass of "chemical compound" (`P279 Q11173`), the model the web app
/// writes too. `P31` is asserted before the structure, because an item with no
/// class is not
/// findable by anything that filters on one, and a `QuickStatements` run stops
/// at the first failure -- so the statements that make the item *usable* come
/// before the ones that fill it in. `P2017` (isomeric SMILES) is emitted only
/// when `isomeric_smiles` is non-empty, so achiral structures do not assert an
/// isomeric SMILES property.
#[must_use]
pub fn create_compound_statements(
    name: &str,
    canonical_smiles: &str,
    isomeric_smiles: &str,
) -> String {
    use std::fmt::Write as _;
    let mut out = format!(
        "CREATE\n  LAST|{label}|\"{name}\"\n  \
         LAST|P31|{entity_type}\n  \
         LAST|P279|{compound_class}\n  \
         LAST|{canonical}|\"{canonical_value}\"",
        label = property::LABEL,
        name = crate::escape_quickstatements(name),
        entity_type = WD_TYPE_CHEMICAL_ENTITY_QID,
        compound_class = WD_CHEMICAL_COMPOUND_QID,
        canonical = property::CANONICAL_SMILES,
        canonical_value = crate::escape_quickstatements(canonical_smiles),
    );
    if !isomeric_smiles.trim().is_empty() {
        let _ = write!(
            out,
            "\n  LAST|{isomeric}|\"{isomeric_value}\"",
            isomeric = property::ISOMERIC_SMILES,
            isomeric_value = crate::escape_quickstatements(isomeric_smiles),
        );
    }
    out
}

/// Fetch every compound in a run with **one** query, keyed by the `InChIKey` the
/// caller computed locally.
///
/// No `LIMIT`: Wikidata has a handful of items for some structures, and that
/// ambiguity is per key, so `MIN(?compound)` picks deterministically instead of
/// letting plan order decide. The aggregate projects as **`?compound_item`** —
/// an `AS` clause may not target a variable the body already binds.
#[must_use]
pub fn build_compound_lookup_query(keys: &[String]) -> String {
    let values = keys
        .iter()
        .map(|key| format!("\"{}\"", escape_sparql_string(key)))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?key (MIN(?compound) AS ?compound_item) ?canonical ?iso ?inchi ?formula ?mass WHERE {{\n  \
           VALUES ?key {{ {values} }}\n  \
           ?compound wdt:P235 ?key .\n  \
           OPTIONAL {{ ?compound wdt:P233 ?canonical . }}\n  \
           OPTIONAL {{ ?compound wdt:P2017 ?iso . }}\n  \
           OPTIONAL {{ ?compound wdt:P234 ?inchi . }}\n  \
           OPTIONAL {{ ?compound wdt:P274 ?formula . }}\n  \
           OPTIONAL {{ ?compound wdt:P2067 ?mass . }}\n  \
         }} GROUP BY ?key ?canonical ?iso ?inchi ?formula ?mass"
    )
}

/// The batched `(compound, taxon)` occurrence query (`wdt:P703` excludes
/// `wikibase:DeprecatedRank` statements via truthy triples).
#[must_use]
pub fn build_occurrence_query(pairs: &[(String, String)]) -> String {
    let values = pairs
        .iter()
        .map(|(compound, taxon)| format!("(wd:{compound} wd:{taxon})"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT DISTINCT ?compound ?taxon WHERE {{\n  \
           VALUES (?compound ?taxon) {{ {values} }}\n  \
           ?compound wdt:{occurs} ?taxon .\n\
         }}",
        occurs = property::OCCURS_IN_TAXON,
    )
}

/// The batched `(compound, taxon, reference)` occurrence query.
///
/// Checks the `prov:wasDerivedFrom / pr:P248` reference node on the reified
/// `p:P703` statement rather than a direct `wdt:P248` on `?compound`, because
/// Wikidata records the supporting publication as a reference on the taxon
/// occurrence statement, and excludes `wikibase:DeprecatedRank` statements.
#[must_use]
pub fn build_occurrence_with_ref_query(triples: &[(String, String, String)]) -> String {
    let values = triples
        .iter()
        .map(|(compound, taxon, reference)| format!("(wd:{compound} wd:{taxon} wd:{reference})"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT DISTINCT ?compound ?taxon ?ref WHERE {{\n  \
           VALUES (?compound ?taxon ?ref) {{ {values} }}\n  \
           ?compound p:{occurs} ?stmt .\n  \
           ?stmt ps:{occurs} ?taxon ;\n        \
                 wikibase:rank ?rank ;\n        \
                 prov:wasDerivedFrom ?refnode .\n  \
           ?refnode pr:P248 ?ref .\n  \
           FILTER(?rank != wikibase:DeprecatedRank)\n\
         }}",
        occurs = property::OCCURS_IN_TAXON,
    )
}

/// The batched reference lookup query for normalized DOIs.
#[must_use]
pub fn build_reference_lookup_query(dois: &[String]) -> String {
    let values = dois
        .iter()
        .map(|doi| {
            let escaped = escape_sparql_string(doi);
            format!("(\"{escaped}\" \"{escaped}\")")
        })
        .collect::<Vec<_>>()
        .join("\n    ");
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?lookup ?ref WHERE {{\n  \
           VALUES (?lookup ?doi) {{\n    \
             {values}\n  \
           }}\n  \
           ?ref wdt:P356 ?doi .\n\
         }}"
    )
}

/// Normalize a taxon name for case-insensitive lookup caching.
#[must_use]
pub fn normalize_taxon_lookup(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

fn canonicalize_taxon_label(name: &str) -> String {
    let mut words = name.split_whitespace();
    let Some(first) = words.next() else {
        return String::new();
    };

    let mut rebuilt = String::with_capacity(name.len());
    let mut chars = first.chars();
    if let Some(ch) = chars.next() {
        rebuilt.push(ch.to_ascii_uppercase());
        rebuilt.extend(chars.map(|c| c.to_ascii_lowercase()));
    }
    for word in words {
        rebuilt.push(' ');
        rebuilt.push_str(&word.to_ascii_lowercase());
    }
    rebuilt
}

/// Candidate spellings for a taxon lookup (both the raw trimmed name and its
/// stripped canonical or binomial-capitalised form when they differ).
#[must_use]
pub fn taxon_name_candidates(name: &str) -> Vec<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut out = vec![trimmed.to_string()];
    if let Some(canonical_taxon) = extract_canonical_taxon_name(trimmed) {
        if canonical_taxon != trimmed {
            out.push(canonical_taxon);
        }
        return out;
    }

    let canonical = canonicalize_taxon_label(trimmed);
    if canonical != trimmed {
        out.push(canonical);
    }
    out
}

/// The single-taxon lookup query using `VALUES ?taxonName` over the candidate
/// spellings of `name`.
#[must_use]
pub fn build_single_taxon_lookup_query(name: &str) -> Option<String> {
    let candidates = taxon_name_candidates(name);
    if candidates.is_empty() {
        return None;
    }

    let values = candidates
        .iter()
        .map(|candidate| format!("\"{}\"", escape_sparql_string(candidate)))
        .collect::<Vec<_>>()
        .join(" ");

    Some(format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?taxon WHERE {{\n  \
           VALUES ?taxonName {{ {values} }}\n  \
           ?taxon wdt:P225 ?taxonName ;\n        \
                  wdt:P31 wd:Q16521 .\n\
         }} ORDER BY ?taxon LIMIT 1"
    ))
}

/// The batched taxon lookup query for `(lookup_key, candidate_name)` pairs.
#[must_use]
pub fn build_taxon_lookup_query(lookups: &[(String, String)]) -> String {
    let values = lookups
        .iter()
        .map(|(lookup, taxon_name)| {
            format!(
                "(\"{}\" \"{}\")",
                escape_sparql_string(lookup),
                escape_sparql_string(taxon_name)
            )
        })
        .collect::<Vec<_>>()
        .join("\n    ");
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?lookup ?taxon WHERE {{\n  \
           VALUES (?lookup ?taxonName) {{\n    \
             {values}\n  \
           }}\n  \
           ?taxon wdt:P225 ?taxonName ;\n        \
                  wdt:P31 wd:Q16521 .\n\
         }}"
    )
}

#[cfg(test)]
#[path = "wikidata_query/tests.rs"]
mod tests;
