// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use serde_json::Value;

// -- SPARQL / QS helpers -------------------------------------------------------

pub(super) fn extract_qid_from_uri(uri: &str) -> Option<&str> {
    uri.rsplit('/').next().filter(|segment| {
        segment.starts_with('Q') && segment[1..].bytes().all(|b| b.is_ascii_digit())
    })
}

pub(super) fn binding_value(binding: &Value, key: &str) -> Option<String> {
    binding
        .get(key)
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

/// Escape a string literal for use inside a SPARQL double-quoted string.
/// Backslashes are doubled and double-quotes are backslash-escaped.
/// A single forward pass avoids two intermediate heap allocations.
pub(super) fn escape_sparql_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 4);
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            other => out.push(other),
        }
    }
    out
}

/// Escape a string literal for use inside a `QuickStatements` statement value.
/// Unlike SPARQL, `QuickStatements` string values (inside double quotes)
pub(super) fn escape_qs_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 4);
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// Format a Wikidata `QuickStatements` mass statement using the dalton unit (Q483261). Unit
/// syntax is `U<QID>` - there is NO leading `Q` after the `U`.
/// Adds S887 reference "inferred from SMILES" (Q113907573).
pub fn qs_mass_statement(subject: &str, mass: f64) -> String {
    format!("{subject}|P2067|+{mass:.6}U483261|S887|Q113907573")
}

/// Reference QIDs for S887 (source/reference) in `QuickStatements`.
pub const QS_REF_INFERRED_FROM_SMILES: &str = "Q113907573"; // inferred from SMILES
pub const QS_REF_INFERRED_FROM_ISOMERIC_SMILES: &str = "Q123282952"; // inferred from isomeric SMILES

/// Build a `QuickStatements` statement with S887 reference(s).
/// Returns a statement like: `subject|prop|"value"|S887|Q113907573|S887|Q123282952`
pub fn qs_statement_with_refs(subject: &str, prop: &str, value: &str, refs: &[&str]) -> String {
    let mut stmt = format!("{subject}|{prop}|\"{value}\"");
    for r in refs {
        stmt.push_str("|S887|");
        stmt.push_str(r);
    }
    stmt
}

/// Build a canonical SMILES statement with appropriate S887 references.
pub fn qs_canonical_smiles_statement(
    subject: &str,
    canonical_smiles: &str,
    has_isomeric: bool,
) -> String {
    if has_isomeric {
        qs_statement_with_refs(
            subject,
            "P233",
            canonical_smiles,
            &[crate::features::curation::services::helpers::QS_REF_INFERRED_FROM_ISOMERIC_SMILES],
        )
    } else {
        // No reference for canonical SMILES when no isomeric present
        format!("{subject}|P233|\"{canonical_smiles}\"")
    }
}

/// Build an InChI statement with "inferred from SMILES" reference.
pub fn qs_inchi_statement(subject: &str, inchi: &str) -> String {
    qs_statement_with_refs(subject, "P234", inchi, &[QS_REF_INFERRED_FROM_SMILES])
}

/// Build an `InChIKey` statement with "inferred from SMILES" reference.
pub fn qs_inchikey_statement(subject: &str, inchikey: &str) -> String {
    qs_statement_with_refs(subject, "P235", inchikey, &[QS_REF_INFERRED_FROM_SMILES])
}

/// Build an isomeric SMILES statement WITHOUT any S887 reference.
/// Isomeric SMILES is the source itself, not inferred from something else.
pub fn qs_isomeric_smiles_statement(subject: &str, isomeric_smiles: &str) -> String {
    format!("{subject}|P2017|\"{isomeric_smiles}\"")
}

// -- Text / chemistry normalization --------------------------------------------

pub(super) fn normalize_doi(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let canonical =
        find_ascii_ci(trimmed, b"doi.org/").map_or(trimmed, |idx| &trimmed[(idx + 8)..]);
    if canonical.is_empty() {
        return None;
    }
    Some(canonical.to_ascii_uppercase())
}

pub(super) fn find_ascii_ci(haystack: &str, needle: &[u8]) -> Option<usize> {
    let hb = haystack.as_bytes();
    if needle.is_empty() || hb.len() < needle.len() {
        return None;
    }
    hb.windows(needle.len())
        .position(|w| w.iter().zip(needle).all(|(a, b)| a.eq_ignore_ascii_case(b)))
}

pub(super) fn has_stereo_marks(smiles: &str) -> bool {
    smiles.contains('@') || smiles.contains('/') || smiles.contains('\\')
}

pub(super) fn has_isomeric_smiles(smiles: &str) -> bool {
    has_stereo_marks(smiles)
}

pub fn extract_formula_from_inchi(inchi: &str) -> Option<String> {
    let cleaned = inchi.trim();
    if cleaned.is_empty() || !cleaned.starts_with("InChI=") {
        return None;
    }
    let right = cleaned.split('/').nth(1)?;
    if right.is_empty() {
        return None;
    }
    Some(right.into())
}

pub fn normalize_formula_for_wikidata(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '0' => '₀',
            '1' => '₁',
            '2' => '₂',
            '3' => '₃',
            '4' => '₄',
            '5' => '₅',
            '6' => '₆',
            '7' => '₇',
            '8' => '₈',
            '9' => '₉',
            _ => ch,
        })
        .collect()
}

// The fixtures are SPARQL prefixes and Turtle, where `#` introduces a
// comment and a literal one would be a syntax error in the thing under test.
#[cfg(test)]
#[allow(clippy::needless_raw_string_hashes)]
mod tests {
    use super::*;

    #[test]
    fn escape_sparql_string_escapes_backslash_and_quote() {
        assert_eq!(escape_sparql_string(r#"C\C"#), r#"C\\C"#);
        assert_eq!(escape_sparql_string(r#"C"C"#), r#"C\"C"#);
        assert_eq!(escape_sparql_string(r#"C\C"D"#), r#"C\\C\"D"#);
    }

    #[test]
    fn escape_qs_string_escapes_only_quote_newline_tab() {
        // SMILES with backslashes (stereo chemistry) should NOT be double-escaped
        // See https://www.wikidata.org/wiki/Q140985706
        let smiles = r#"C/C(=C\CC/C=C(\C)CC/C=C(\C)CCC(=O)O)CC/C=C(\C)CCC(=O)O"#;
        assert_eq!(escape_qs_string(smiles), smiles);

        // Double quotes should be escaped
        assert_eq!(escape_qs_string(r#"C"C"#), r#"C\"C"#);

        // Newlines and tabs should be escaped
        assert_eq!(escape_qs_string("C\nC"), "C\\nC");
        assert_eq!(escape_qs_string("C\tC"), "C\\tC");

        // InChI with backslashes should not be double-escaped
        let inchi = "InChI=1S/C8H10N4O2/c1-10-4-9-6-5(10)7(13)12(3)8(14)11(6)2/h4-6H,1-3H3";
        assert_eq!(escape_qs_string(inchi), inchi);
    }

    #[test]
    fn qs_mass_statement_includes_smiles_reference() {
        // Mass statement should include S887 reference "inferred from SMILES" (Q113907573)
        let stmt = qs_mass_statement("LAST", 495.20268);
        assert!(stmt.contains("P2067"));
        assert!(stmt.contains("U483261"));
        assert!(stmt.contains("S887"));
        assert!(stmt.contains("Q113907573"));
    }
}
