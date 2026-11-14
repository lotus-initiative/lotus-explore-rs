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

use lotus_search::non_empty;
pub use lotus_search::normalize_doi;

use crate::{CurationError, CurationInputRow};

///
/// # Errors
/// [`CurationError::MissingTsvColumn`] if there is no `name` or `smiles`
/// column. An empty input is not an error: it is an empty batch.
pub fn parse_tsv(tsv: &str) -> Result<Vec<CurationInputRow>, CurationError> {
    // `lines()` already strips `\n` and `\r\n`. A bare `\r` at line-end is
    // stripped with `trim_end_matches`. Leading/trailing whitespace on the line
    // as a whole is NOT stripped here, because `str::trim` eats `\t`, which
    // shifts columns when the first cell is empty. Each cell is trimmed
    // individually at `field.trim()` below.
    // A byte-order mark, which spreadsheet programs write by default, is not part
    // of the first column's name.
    let tsv = tsv.strip_prefix('\u{feff}').unwrap_or(tsv);
    let mut lines = tsv
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !line.trim().is_empty());

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
    // The taxon and the DOI are compared case-insensitively: `10.1/A` and
    // `10.1/a` are one DOI, and a spreadsheet has no opinion about how a taxon
    // is capitalised. The structure is not folded, because in SMILES case is
    // chemistry: `c1ccccc1` is benzene and `C1CCCCC1` is cyclohexane.
    let smiles = row.smiles.trim();
    let taxon = row.taxon.as_deref().and_then(non_empty).unwrap_or_default();
    let doi = row.doi.as_deref().and_then(normalize_doi);
    let doi_str = doi.as_deref().unwrap_or_default();
    let mut out = String::with_capacity(smiles.len() + taxon.len() + doi_str.len() + 2);
    out.push_str(smiles);
    out.push('\t');
    for ch in taxon.chars() {
        out.push(ch.to_ascii_lowercase());
    }
    out.push('\t');
    for ch in doi_str.chars() {
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn normalize_header(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

#[cfg(test)]
#[path = "input/tests.rs"]
mod tests;
