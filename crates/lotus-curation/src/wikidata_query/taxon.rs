// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Taxon name lookup queries (`taxon_by_name_query`, batched and single candidate lookups).

use super::statements::escape_sparql_string;
use super::taxon_name::extract_canonical_taxon_name;
use crate::CURATION_SPARQL_PREFIXES;

/// The item for a taxon name, or `None` if Wikidata has no such taxon.
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

/// The single-taxon lookup query using `VALUES ?taxonName` over candidate spellings.
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
