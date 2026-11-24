// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Reading a curation input table (TSV or CSV).
//!
//! Both front-ends -- the web upload and `lotus curate` -- share this parser so
//! column matching by header name and extra spreadsheet columns behave identically.
//!
//! Delimiter (`\t` for TSV or `,` for CSV) is chosen from whichever splits the
//! header line into more columns, and RFC-4180 quoted fields are unescaped. Columns are matched by name,
//! case- and space-insensitively, accepting common natural-product headers such
//! as `compound_name` for `name`, `organism` / `source_organism` / `species`
//! for `taxon`, and `reference_doi` for `doi`. Only `name` and `smiles` are
//! required: a row with no taxon and no DOI still has a compound to curate.

use lotus_model::{non_empty, normalize_doi};

use crate::{CurationError, CurationInputRow};

/// Parses a curation batch table from UTF-8 TSV or CSV text.
///
/// The delimiter (`\t` or `,`) is inferred from the header line outside double
/// quotes. Quoted fields follow RFC-4180 (`""` unescapes to `"`). Leading
/// UTF-8 BOMs, `#`-prefixed comment or template example rows, and empty lines
/// are ignored.
///
/// # Errors
/// [`CurationError::MissingTsvColumn`] if there is no `name` or `smiles`
/// column (or supported alias). An empty input is not an error: it is an empty
/// batch.
pub fn parse_tabular(input: &str) -> Result<Vec<CurationInputRow>, CurationError> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let trimmed_start = input.trim_start_matches(['\r', '\n']);
    if trimmed_start.trim().is_empty() {
        return Ok(Vec::new());
    }

    let tsv_rows = parse_records(trimmed_start, '\t');
    let csv_rows = parse_records(trimmed_start, ',');
    let chosen = if csv_rows.first().map_or(0, Vec::len) > tsv_rows.first().map_or(0, Vec::len) {
        csv_rows
    } else {
        tsv_rows
    };
    let mut records = chosen.into_iter();
    let Some(header) = records.next() else {
        return Ok(Vec::new());
    };

    let columns: Vec<String> = header.iter().map(|h| normalize_header(h)).collect();
    let name_idx = find_col(
        &columns,
        &["name", "compound_name", "compound", "compound_id"],
    )
    .ok_or(CurationError::MissingTsvColumn("name"))?;
    let smiles_idx = find_col(&columns, &["smiles", "isomeric_smiles"])
        .ok_or(CurationError::MissingTsvColumn("smiles"))?;
    let taxon_idx = find_col(
        &columns,
        &[
            "taxon",
            "organism",
            "organism_name",
            "source_organism",
            "species",
        ],
    );
    let doi_idx = find_col(&columns, &["doi", "reference_doi"]);
    let status_idx = find_col(&columns, &["status"]);

    let mut out = Vec::new();
    for row in records {
        if row
            .first()
            .is_some_and(|cell| cell.trim_start().starts_with('#'))
        {
            continue;
        }
        let Some(name_raw) = row.get(name_idx) else {
            continue;
        };
        let Some(smiles_raw) = row.get(smiles_idx) else {
            continue;
        };
        let name = name_raw.trim();
        let smiles = smiles_raw.trim();
        if name.is_empty() || smiles.is_empty() {
            continue;
        }
        let taxon = taxon_idx
            .and_then(|idx| row.get(idx))
            .map(String::as_str)
            .and_then(|v| non_empty(v.trim()).map(ToOwned::to_owned));
        let doi = doi_idx
            .and_then(|idx| row.get(idx))
            .map(String::as_str)
            .and_then(|v| normalize_doi(v.trim()));
        let status = status_idx
            .and_then(|idx| row.get(idx))
            .map(String::as_str)
            .and_then(|v| non_empty(v.trim()).map(ToOwned::to_owned));
        out.push(CurationInputRow {
            name: name.into(),
            smiles: smiles.into(),
            taxon,
            doi,
            status,
        });
    }
    Ok(out)
}

fn find_col(columns: &[String], aliases: &[&str]) -> Option<usize> {
    aliases
        .iter()
        .find_map(|alias| columns.iter().position(|col| col == alias))
}

fn parse_records(input: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut current_row = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut at_field_start = true;
    let mut chars = input.chars();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.clone().next() == Some('"') {
                    chars.next();
                    current_field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                current_field.push(ch);
            }
            continue;
        }

        if ch == '"' && at_field_start {
            in_quotes = true;
            at_field_start = false;
        } else if ch == delimiter {
            current_row.push(std::mem::take(&mut current_field));
            at_field_start = true;
        } else if matches!(ch, '\r' | '\n') {
            finish_row(&mut rows, &mut current_row, &mut current_field);
            at_field_start = true;
        } else {
            current_field.push(ch);
            at_field_start = false;
        }
    }

    finish_row(&mut rows, &mut current_row, &mut current_field);
    rows
}

fn finish_row(rows: &mut Vec<Vec<String>>, row: &mut Vec<String>, field: &mut String) {
    row.push(std::mem::take(field));
    if row.iter().any(|cell| !cell.trim().is_empty()) {
        rows.push(std::mem::take(row));
    } else {
        row.clear();
    }
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
