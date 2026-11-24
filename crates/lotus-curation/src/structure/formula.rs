// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure `InChI` molecular formula extraction, subscript normalisation, and SMILES stereo stripping.

/// Extract the molecular formula layer from a standard `InChI` string
/// (e.g. `"InChI=1S/C2H6O/c1-2-3/h3H,2H2,1H3"` -> `Some("C2H6O")`).
#[must_use]
pub fn extract_formula_from_inchi(inchi: &str) -> Option<String> {
    let cleaned = inchi.trim();
    if !cleaned.starts_with("InChI=") {
        return None;
    }
    let right = cleaned.split('/').nth(1)?;
    if right.is_empty() {
        return None;
    }
    Some(right.into())
}

/// Convert ASCII digits in a molecular formula to Unicode subscript digits as
/// stored in Wikidata `P274` (e.g. `"C2H6O"` -> `"C₂H₆O"`).
#[must_use]
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

/// Whether a `SMILES` string carries tetrahedral (`@`) or double-bond (`/`, `\`)
/// stereochemistry markers.
#[must_use]
pub fn has_stereo_marks(smiles: &str) -> bool {
    smiles.contains('@') || smiles.contains('/') || smiles.contains('\\')
}

/// Strip tetrahedral (`@`, `@@`) and cis/trans (`/`, `\`) stereochemistry
/// markers and any trailing CXSMILES (`|...|`) extension from a `SMILES` string
/// so it can be asserted as a non-isomeric canonical `SMILES` (`P233`).
#[must_use]
pub fn strip_stereo_marks(smiles: &str) -> String {
    let core = smiles
        .strip_suffix('|')
        .and_then(|prefix| prefix.rsplit_once(" |"))
        .map_or(smiles, |(head, _)| head);
    let unbracketed = core
        .replace("[C@@H]", "C")
        .replace("[C@H]", "C")
        .replace("[C@@]", "C")
        .replace("[C@]", "C")
        .replace("[c@@H]", "c")
        .replace("[c@H]", "c");
    unbracketed
        .chars()
        .filter(|c| !matches!(c, '@' | '/' | '\\'))
        .collect()
}
