// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Reading a curation input file.
//!
//! Both front-ends -- the web upload and `lotus curate` -- take the same file, and
//! each used to have its own parser. They disagreed: this one matches columns by
//! name and tolerates a spreadsheet's extra columns, the CLI's took whatever was
//! in the first four fields. A file prepared for the web page then run through the
//! CLI got a different answer, silently.
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
#[path = "input/tests.rs"]
mod tests;
