// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Casing-variant expansion and language tags for compound name lookups.

use super::rewrite::escape_sparql_string;

/// Language tags a compound's name is looked up under.
///
/// A `rdfs:label` or `skos:altLabel` is language-tagged, so matching one means matching the
/// tag too -- see [`compound_label_query`](super::compound_label_query) for why that cannot
/// be worked around. A list rather than a wildcard, a deliberate trade: these are the
/// sixteen languages a chemical name is most likely typed in, and a name in one of the
/// remaining ~280 will not resolve.
///
/// Ordered roughly by how much chemistry is published in each, so truncating costs least.
const COMPOUND_NAME_LANGUAGES: [&str; 16] = [
    "mul", "en", "de", "fr", "es", "nl", "it", "pt", "sv", "pl", "ru", "tr", "ja", "zh", "la", "cs",
];

/// Uppercase the first alphabetic character in an already-lowercased name.
///
/// Keeps leading stereochemical or locant punctuation intact (`"(+)-camphene"` ->
/// `"(+)-Camphene"`).
fn capitalize_first_alphabetic(lower: &str) -> String {
    let mut out = String::with_capacity(lower.len());
    let mut done = false;
    for ch in lower.chars() {
        if !done && ch.is_alphabetic() {
            out.extend(ch.to_uppercase());
            done = true;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Title-case every alphabetic segment in an already-lowercased name.
///
/// Capitalizes after spaces, hyphens, and punctuation (`"citric acid"` -> `"Citric Acid"`,
/// `"l-menthol"` -> `"L-Menthol"`).
fn title_case_segments(lower: &str) -> String {
    let mut out = String::with_capacity(lower.len());
    let mut at_start = true;
    for ch in lower.chars() {
        if ch.is_alphabetic() {
            if at_start {
                out.extend(ch.to_uppercase());
                at_start = false;
            } else {
                out.push(ch);
            }
        } else {
            out.push(ch);
            at_start = true;
        }
    }
    out
}

/// Deduplicated casing variants of a compound name for exact-literal `VALUES` lookup.
///
/// Preserves the verbatim input first, then adds lowercase, sentence case, title case, and
/// (for hyphenated names with a lowercase locant/prefix such as `β-carotin` or
/// `trans-resveratrol`) hyphen-stem case. Non-cased scripts collapse to a single variant.
fn name_case_variants(name: &str) -> Vec<String> {
    let mut variants = Vec::new();
    let mut push_unique = |candidate: String| {
        if !variants.contains(&candidate) {
            variants.push(candidate);
        }
    };

    push_unique(name.to_owned());
    let lower = name.to_lowercase();
    push_unique(lower.clone());
    push_unique(capitalize_first_alphabetic(&lower));
    push_unique(title_case_segments(&lower));
    if let Some((prefix, rest)) = lower.rsplit_once('-') {
        push_unique(format!("{prefix}-{}", capitalize_first_alphabetic(rest)));
    }
    variants
}

/// The `VALUES` rows for `name`'s casing variants under every `COMPOUND_NAME_LANGUAGES` tag.
pub(super) fn tagged_name_values(name: &str) -> String {
    let variants = name_case_variants(name);
    let mut rows = Vec::new();
    for variant in &variants {
        let escaped = escape_sparql_string(variant);
        for tag in COMPOUND_NAME_LANGUAGES {
            rows.push(format!("\"{escaped}\"@{tag}"));
        }
    }
    format!("VALUES ?name {{ {} }}", rows.join(" "))
}
