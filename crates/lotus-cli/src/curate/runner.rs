// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Curation execution, offline fallback rows, and statement aggregation.

// The escaper lives in the curation crate because the web client builds the same
// statements, and two escapers is one duplicated bug waiting to happen.
use lotus_curation::escape_quickstatements;

use super::{CuratedReport, Finding};

/// The properties a LOTUS row is written against, from the Wikidata projection.
///
/// `P233` is the canonical SMILES and `P2017` the isomeric one. Both are emitted
/// because the explorer queries both: a row with only `P233` matches a
/// structure search by canonical form and not by the form the chemist submitted.
const PROPERTY_CANONICAL_SMILES: &str = "P233";
const PROPERTY_ISOMERIC_SMILES: &str = "P2017";
const PROPERTY_NAME: &str = "Len";

/// Curate one row against Wikidata.
///
/// The structure arrives already converted, because the `InChIKey` is the only
/// thing Wikidata can be asked about: a `SMILES` is not an identity, and
/// matching on one would report a compound as new whenever the row happened to
/// write it in a different valid order.
pub(super) async fn curate_row<H: lotus_search::Http>(
    http: &H,
    finding: &Finding,
    converted: &lotus_curation::ConvertedStructure,
) -> lotus_curation::CurationResultRow {
    match lotus_curation::look_up(http, finding, converted).await {
        Ok(lookup) => lotus_curation::to_result_row(finding, converted, &lookup),
        Err(err) => {
            let mut result = lotus_curation::to_result_row(
                finding,
                converted,
                &lotus_curation::WikidataLookup::default(),
            );
            // A lookup that failed is not a compound that is absent, so the row is
            // an error and not a new item, and must carry no creation statements.
            result.status = lotus_curation::CurationStatus::Error;
            result.note = format!("Wikidata could not be reached: {err}");
            result.dependency_blocks.clear();
            result.quickstatements.clear();
            result
        }
    }
}

/// Report a row that was never looked up.
///
/// The compound statement is still generated, because a curator offline still
/// wants something to read, but the status says the row is unchecked so the
/// bundle is not mistaken for a finished curation. The occurrence and the
/// reference are not: `P703` and `P248` take items, and offline the taxon and
/// the DOI were never resolved to one. The note says so.
pub(super) fn not_checked_row(finding: &Finding) -> lotus_curation::CurationResultRow {
    let raw_smiles = finding.smiles.trim();
    let canonical = lotus_curation::strip_stereo_marks(raw_smiles);
    let isomeric_smiles =
        lotus_curation::has_stereo_marks(raw_smiles).then(|| raw_smiles.to_string());
    if lotus_curation::is_revised_row(finding) {
        return lotus_curation::CurationResultRow {
            input: finding.clone(),
            canonical_smiles: (!canonical.is_empty()).then_some(canonical),
            isomeric_smiles,
            inchikey: None,
            inchi: None,
            formula: None,
            exact_mass: None,
            mass_warning: None,
            wikidata_qid: None,
            status: lotus_curation::CurationStatus::ExistingNeedsUpdates,
            note: lotus_curation::REVISED_ROW_NOTE.to_string(),
            dependency_blocks: Vec::new(),
            quickstatements: vec![lotus_curation::revised_row_statement(finding)],
        };
    }

    let name = escape_quickstatements(finding.name.trim());
    let mut compound_block = format!(
        "## {name}\nCREATE\nLAST|{PROPERTY_NAME}|\"{name}\"\n\
         LAST|P31|{entity_type}\n\
         LAST|P279|{compound_class}\n\
         LAST|{PROPERTY_CANONICAL_SMILES}|\"{canonical}\"",
        entity_type = lotus_curation::WD_TYPE_CHEMICAL_ENTITY_QID,
        compound_class = lotus_curation::WD_CHEMICAL_COMPOUND_QID,
        canonical = escape_quickstatements(&canonical),
    );
    if isomeric_smiles.is_some() {
        use std::fmt::Write as _;
        let _ = write!(
            compound_block,
            "\nLAST|{PROPERTY_ISOMERIC_SMILES}|\"{isomeric}\"",
            isomeric = escape_quickstatements(raw_smiles),
        );
    }
    let statements = vec![compound_block];
    let named = |field: Option<&str>| field.map(str::trim).is_some_and(|v| !v.is_empty());
    let unresolved: Vec<&str> = [
        named(finding.taxon.as_deref()).then_some("taxon"),
        named(finding.doi.as_deref()).then_some("reference"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut note =
        String::from("not looked up: run without --offline to check this against Wikidata");
    if !unresolved.is_empty() {
        use std::fmt::Write as _;
        let _ = write!(
            note,
            "; no statement for the {} until it is resolved to an item",
            unresolved.join(" or ")
        );
    }

    lotus_curation::CurationResultRow {
        input: finding.clone(),
        canonical_smiles: (!canonical.is_empty()).then_some(canonical),
        isomeric_smiles,
        inchikey: None,
        inchi: None,
        formula: None,
        exact_mass: None,
        mass_warning: None,
        wikidata_qid: None,
        status: lotus_curation::CurationStatus::NotChecked,
        note,
        dependency_blocks: Vec::new(),
        quickstatements: statements,
    }
}

/// Whether to say how many rows were not looked up.
///
/// Both halves matter and neither implies the other: a run that checked
/// everything has nothing to report however loud it is, and a quiet run says
/// nothing however much it skipped.
pub(super) const fn should_warn_about_unchecked(unchecked: usize, quiet: bool) -> bool {
    unchecked > 0 && !quiet
}

/// How many rows were never looked up.
///
/// A row ends up `NotChecked` when it was never looked up, which is every row
/// under `--offline`. (A row whose structure could not be read is looked up and
/// reported as an `error` instead.) The count is printed rather than folded into
/// an error: the rows that *were* looked up are still correct, and the ones that
/// were not are still a valid batch to paste into a spreadsheet. A zero is not
/// worth a line on stderr.
pub(super) fn unchecked_count(report: &CuratedReport) -> usize {
    report
        .rows
        .iter()
        .filter(|row| row.status == lotus_curation::CurationStatus::NotChecked)
        .count()
}

/// The SMILES worth sending to the conversion service.
///
/// Trimmed, and blank ones dropped: an empty structure is what the service
/// cannot read, so asking it to convert one spends a request to be told the
/// thing that was already obvious here.
pub(super) fn convertible_smiles(findings: &[Finding]) -> Vec<&str> {
    findings
        .iter()
        .map(|finding| finding.smiles.trim())
        .filter(|smiles| !smiles.is_empty())
        .collect()
}

/// Curate every row, in order.
///
/// Structures convert for the whole file in one batch; converting a row at a time
/// is three requests per row and the public service rate-limits that. Wikidata
/// lookups stay per row: a different service, and a `VALUES` query over a whole
/// file is one enormous answer that loses everything if one row in it is wrong.
pub(super) async fn curate_findings(
    findings: &[Finding],
    offline: bool,
) -> anyhow::Result<CuratedReport> {
    let mut rows = Vec::with_capacity(findings.len());

    if offline {
        rows.extend(findings.iter().map(not_checked_row));
    } else {
        let http = lotus_search::reqwest_client::ReqwestClient::new()?;

        let smiles = convertible_smiles(findings);
        let converted = lotus_curation::convert_structures(&http, &smiles).await?;

        let mut by_smiles: std::collections::HashMap<&str, &lotus_curation::ConvertedStructure> =
            std::collections::HashMap::new();
        for (smiles, structure) in smiles.iter().zip(&converted) {
            by_smiles.insert(smiles, structure);
        }

        for finding in findings {
            let key = finding.smiles.trim();
            match by_smiles.get(key) {
                Some(structure) => rows.push(curate_row(&http, finding, structure).await),
                // No entry means the file had no structure to convert, which
                // `parse_tsv` should have caught; a row that gets here has no
                // structure at all and is reported rather than skipped.
                None => rows.push(
                    curate_row(
                        &http,
                        finding,
                        &lotus_curation::ConvertedStructure::default(),
                    )
                    .await,
                ),
            }
        }
    }

    lotus_curation::merge_new_compound_creates(&mut rows);
    let statements = collect_report_statements(&rows);

    Ok(CuratedReport {
        rows,
        statements,
        citation: "https://doi.org/10.7554/eLife.70780",
    })
}

/// Collect deduplicated statements across `rows`, emitting all dependency blocks
/// before all main statements so prerequisites are created first.
pub(super) fn collect_report_statements(rows: &[lotus_curation::CurationResultRow]) -> Vec<String> {
    let mut statements = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for row in rows {
        for block in &row.dependency_blocks {
            if seen.insert(block.as_str()) {
                statements.push(block.clone());
            }
        }
    }
    for row in rows {
        for block in &row.quickstatements {
            if seen.insert(block.as_str()) {
                statements.push(block.clone());
            }
        }
    }
    statements
}
