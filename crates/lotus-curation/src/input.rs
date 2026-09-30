// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Reading a curation input file.
//!
//! Both curation front-ends -- the web upload and `lotus curate` -- take the
//! same file, and they used to each have their own parser. The two did not
//! agree: this one matches columns by name and tolerates a spreadsheet's extra
//! columns, and the CLI's took whatever was in the first four fields. Someone
//! who prepared a file for the web page and then used the CLI got a different
//! answer, silently.
//!
//! Columns are matched by name, case- and space-insensitively, and `organism`
//! is accepted for `taxon` because that is what a sheet of occurrence data will
//! have in it. Only `name` and `smiles` are required: a row with no taxon
//! and no DOI still has a compound to curate.

use crate::{CurationError, CurationInputRow};

/// Case-insensitive search for a byte string, without allocating.
fn find_ascii_ci(haystack: &str, needle: &[u8]) -> Option<usize> {
    // Fold the needle once rather than both sides of every comparison, and
    // compare windows with a slice equality rather than byte by byte.
    let folded: Vec<u8> = needle.iter().map(u8::to_ascii_lowercase).collect();
    haystack
        .as_bytes()
        .windows(folded.len())
        .position(|w| w == folded)
}

///
/// # Errors
/// [`CurationError::MissingTsvColumn`] if there is no `name` or `smiles`
/// column. An empty input is not an error: it is an empty batch.
pub fn parse_tsv(tsv: &str) -> Result<Vec<CurationInputRow>, CurationError> {
    let mut lines = tsv.lines().map(str::trim).filter(|line| !line.is_empty());

    let Some(header) = lines.next() else {
        return Ok(Vec::new());
    };

    let columns = header.split('\t').map(normalize_header).collect::<Vec<_>>();
    let name_idx = columns
        .iter()
        .position(|c| c == "name")
        .ok_or(CurationError::MissingTsvColumn("name"))?;
    let smiles_idx = columns
        .iter()
        .position(|c| c == "smiles")
        .ok_or(CurationError::MissingTsvColumn("smiles"))?;
    let taxon_idx = columns
        .iter()
        .position(|c| matches!(c.as_str(), "taxon" | "organism"));
    let doi_idx = columns.iter().position(|c| c == "doi");
    let max_needed_idx = [Some(name_idx), Some(smiles_idx), taxon_idx, doi_idx]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(0);

    let mut out = Vec::new();
    for line in lines {
        let mut name: Option<&str> = None;
        let mut smiles: Option<&str> = None;
        let mut taxon_raw: Option<&str> = None;
        let mut doi_raw: Option<&str> = None;

        for (idx, field) in line.split('\t').enumerate() {
            if idx > max_needed_idx {
                break;
            }
            let field = field.trim();
            if idx == name_idx {
                name = Some(field);
            }
            if idx == smiles_idx {
                smiles = Some(field);
            }
            if taxon_idx == Some(idx) {
                taxon_raw = Some(field);
            }
            if doi_idx == Some(idx) {
                doi_raw = Some(field);
            }
        }

        let Some(name) = name else {
            continue;
        };
        let Some(smiles) = smiles else {
            continue;
        };
        if name.is_empty() || smiles.is_empty() {
            continue;
        }
        let taxon = taxon_raw.and_then(|v| non_empty(v).map(ToOwned::to_owned));
        let doi = doi_raw.and_then(normalize_doi);
        out.push(CurationInputRow {
            name: name.into(),
            smiles: smiles.into(),
            taxon,
            doi,
        });
    }
    Ok(out)
}

/// The key that decides whether two rows are the same finding.
///
/// # Errors
/// Never; infallible by construction.
pub fn row_uniqueness_key(row: &CurationInputRow) -> String {
    // Every part is compared case-insensitively, and the structure is
    // upper-cased. A spreadsheet has no opinion about capitalisation, and `CCO`
    // and `cco` are the same molecule, as are `10.1/A` and `10.1/a`. The
    // CLI's own version of this key did all three; the web's did not, so the
    // web treated a re-cased duplicate as two findings.
    let smiles = row.smiles.trim().to_ascii_uppercase();
    let taxon = row
        .taxon
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let doi = row
        .doi
        .as_deref()
        .and_then(normalize_doi)
        .unwrap_or_default()
        .to_ascii_lowercase();
    format!("{smiles}\t{taxon}\t{doi}")
}

fn normalize_header(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

fn normalize_doi(value: &str) -> Option<String> {
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

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn parse_tsv_supports_expected_headers() {
        let tsv = "name\tsmiles\torganism\tdoi\nA\tCCO\tTaxon\thttps://doi.org/10.1/x\n";
        let rows = parse_tsv(tsv).expect("tsv parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "A");
        assert_eq!(rows[0].smiles, "CCO");
        assert_eq!(rows[0].taxon.as_deref(), Some("Taxon"));
        assert_eq!(rows[0].doi.as_deref(), Some("10.1/X"));
    }

    #[test]
    fn parse_tsv_drops_rows_missing_either_required_field() {
        // Both fields are required, and they are required independently: a row
        // with a name but no structure, or a structure but no name, is as
        // unusable as a row with neither. Mutation testing caught the check
        // reading `name.is_empty() && smiles.is_empty()`, which keeps
        // half-empty rows and sends them on to be looked up.
        let tsv = "name\tsmiles\n\
                   A\tCCO\n\
                   \tCCC\n\
                   B\t\n\
                   \t\n\
                   C\tCCN\n";
        let rows = parse_tsv(tsv).expect("tsv parse");
        assert_eq!(
            rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            vec!["A", "C"],
            "only rows with both a name and a structure should survive"
        );
    }

    #[test]
    fn parse_tsv_ignores_a_header_row() {
        // The header is located by name, so a header line is just another row
        // and has to be dropped by the same required-field rule.
        let tsv = "name\tsmiles\nA\tCCO\n";
        let rows = parse_tsv(tsv).expect("tsv parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "A");
    }

    #[test]
    fn row_key_normalizes_structure_taxon_and_doi() {
        // All three are folded, and the structure is upper-cased. This used to
        // assert the opposite for the DOI, pinning behaviour where `10.1/A` and
        // `10.1/a` were two different findings -- which is how a re-cased
        // duplicate got submitted twice.
        let row = CurationInputRow {
            name: "compound A".into(),
            smiles: " cco ".into(),
            taxon: Some("  Voacanga africana ".into()),
            doi: Some("https://doi.org/10.1000/ABC".into()),
        };
        assert_eq!(
            row_uniqueness_key(&row),
            "CCO\tvoacanga africana\t10.1000/abc"
        );
    }

    #[test]
    fn a_row_matches_itself_regardless_of_case() {
        let lower = CurationInputRow {
            name: "Quercetin".into(),
            smiles: "cco".into(),
            taxon: Some("Gentiana lutea".into()),
            doi: Some("10.1/a".into()),
        };
        let shouty = CurationInputRow {
            name: "Also called quercetin".into(),
            smiles: "CCO".into(),
            taxon: Some("GENTIANA LUTEA".into()),
            doi: Some("10.1/A".into()),
        };
        assert_eq!(
            row_uniqueness_key(&lower),
            row_uniqueness_key(&shouty),
            "the name is not part of the identity"
        );
    }

    #[test]
    fn a_row_needs_both_a_name_and_a_smiles() {
        // The row is dropped if either half is missing. Each is dropped on its
        // own here, because the guard is an `||`: turned into `&&`, a row with
        // only one half survives and gets submitted with nothing to label it by.
        //
        // The empty field is in the middle deliberately. `parse_tsv` trims each
        // whole line before splitting it, so a leading tab is gone before the
        // columns are read and the first field can never come out empty -- the
        // only way to see this is a gap between two present fields.
        let tsv = "organism\tname\tsmiles\n\
                   Taxon\tKeep\tCCO\n\
                   Taxon\t\tCCC\n\
                   \tNoStructure\t\n\
                   Taxon\tBoth blank\t \n";
        let rows = parse_tsv(tsv).expect("tsv parse");
        let kept: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(kept, ["Keep"], "only the complete row survives: {kept:?}");
    }
}

#[test]
// Test-only: fixed literal input, index assertions target known positions.
#[allow(clippy::expect_used)]
#[allow(clippy::indexing_slicing)]
fn test_user_example_multiline() {
    let tsv = "name\tsmiles\ttaxon\tdoi\n2'-deoxyguanosine\tC1[C@@H]([C@H](O[C@H]1N2C=NC3=C2N=C(NC3=O)N)CO)O\tIsaria cicadae\t10.1177/1934578X1501001233\nthymidine\tCC1=CN(C(=O)NC1=O)[C@H]2C[C@@H]([C@H](O2)CO)O\tIsaria cicadae\t10.1177/1934578X1501001233\nadenosine\tC1=NC(=C2C(=N1)N(C=N2)[C@H]3[C@@H]([C@@H]([C@H](O3)CO)O)O)N\tIsaria cicadae\t10.1177/1934578X1501001233\n2'-deoxyadenosine\tC1[C@@H]([C@H](O[C@H]1N2C=NC3=C(N=CN=C32)N)CO)O\tIsaria cicadae\t10.1177/1934578X1501001233\ntryptophan\tC1=CC=C2C(=C1)C(=CN2)C[C@@H](C(=O)O)N\tIsaria cicadae\t10.1177/1934578X1501001233\nphenylalanine\tC1=CC=C(C=C1)C[C@@H](C(=O)O)N\tIsaria cicadae\t10.1177/1934578X1501001233\ntyrosine\tC1=CC(=CC=C1C[C@@H](C(=O)O)N)O\tIsaria cicadae\t10.1177/1934578X1501001233\nN-acetylnoradrenaline\tCC(=O)NCC(C1=CC(=C(C=C1)O)O)O\tIsaria cicadae\t10.1177/1934578X1501001233";

    let rows = parse_tsv(tsv).expect("tsv parse");
    assert_eq!(rows.len(), 8, "Expected 8 rows, got {}", rows.len());

    for row in rows {
        assert!(!row.name.is_empty(), "Name should not be empty");
        assert!(!row.smiles.is_empty(), "SMILES should not be empty");
        assert_eq!(
            row.taxon.as_deref(),
            Some("Isaria cicadae"),
            "Taxon should be Isaria cicadae"
        );
        assert_eq!(
            row.doi.as_deref(),
            Some("10.1177/1934578X1501001233"),
            "DOI should match"
        );
    }
}
