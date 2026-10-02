// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]

use crate::curation::{
    CurationError, CurationErrorKind, CurationInputRow, CurationResultRow, CurationStatus,
    QuickStatementsBundle, build_quickstatements_bundle, curate_rows, row_uniqueness_key,
};
use crate::i18n::{
    Locale, msg_curation_failed, msg_curation_rate_limited, msg_done_review_copy,
    msg_prerequisites_pending, msg_second_pass_done, msg_second_pass_still_pending_count,
};
use std::collections::HashMap;
use std::sync::Arc;

pub struct RunCurationOutcome {
    pub result_rows: Arc<[CurationResultRow]>,
    pub quickstatements: QuickStatementsBundle,
    pub awaiting_second_pass: bool,
    pub status_message: String,
}

pub struct SecondPassOutcome {
    pub result_rows: Arc<[CurationResultRow]>,
    pub quickstatements: QuickStatementsBundle,
    pub awaiting_second_pass: bool,
    pub status_message: String,
}

pub fn format_curation_error(locale: Locale, detail: &str) -> String {
    if detail.contains("HTTP 429")
        || detail.contains("rate limited")
        || detail.contains("10 per 1 minute")
    {
        return msg_curation_rate_limited(locale).into();
    }

    msg_curation_failed(locale, detail)
}

pub fn format_curation_error_typed(locale: Locale, err: &CurationError) -> String {
    let detail = err.to_string();
    if matches!(err.kind(), CurationErrorKind::Transport) {
        return format_curation_error(locale, &detail);
    }
    if !err.is_recoverable() {
        return msg_curation_failed(locale, &detail);
    }
    msg_curation_failed(locale, &detail)
}

pub async fn run_curation(
    locale: Locale,
    snapshot: Vec<CurationInputRow>,
) -> Result<RunCurationOutcome, CurationError> {
    let (curated_rows, quickstatements) = curate_rows(locale, snapshot).await?;
    let pending_count = curated_rows
        .iter()
        .filter(|row| matches!(row.status, CurationStatus::PendingDependencies))
        .count();

    let status_message = if pending_count > 0 {
        msg_prerequisites_pending(locale, pending_count)
    } else {
        msg_done_review_copy(locale)
    };

    Ok(RunCurationOutcome {
        awaiting_second_pass: !quickstatements.dependencies.is_empty(),
        result_rows: Arc::from(curated_rows.into_boxed_slice()),
        quickstatements,
        status_message,
    })
}

pub fn second_pass_inputs(rows: &[CurationResultRow]) -> Vec<CurationInputRow> {
    rows.iter()
        .filter(|row| !row.dependency_blocks.is_empty())
        .map(|row| row.input.clone())
        .collect()
}

pub fn apply_second_pass(
    locale: Locale,
    previous_rows: &[CurationResultRow],
    updated_rows: Vec<CurationResultRow>,
) -> SecondPassOutcome {
    let mut by_key = updated_rows
        .into_iter()
        .map(|row| (row_uniqueness_key(&row.input), row))
        .collect::<HashMap<_, _>>();

    let merged_rows = previous_rows
        .iter()
        .map(|row| {
            let key = row_uniqueness_key(&row.input);
            by_key.remove(&key).unwrap_or_else(|| row.clone())
        })
        .collect::<Vec<_>>();

    let quickstatements = build_quickstatements_bundle(&merged_rows);
    let pending_count = merged_rows
        .iter()
        .filter(|row| !row.dependency_blocks.is_empty())
        .count();
    let awaiting_second_pass = !quickstatements.dependencies.is_empty();

    let status_message = if awaiting_second_pass {
        msg_second_pass_still_pending_count(locale, pending_count)
    } else {
        msg_second_pass_done(locale).into()
    };

    SecondPassOutcome {
        result_rows: Arc::from(merged_rows.into_boxed_slice()),
        quickstatements,
        awaiting_second_pass,
        status_message,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    fn row(name: &str, smiles: &str, deps: Vec<&str>, qs: Vec<&str>) -> CurationResultRow {
        CurationResultRow {
            input: CurationInputRow {
                name: name.to_string(),
                smiles: smiles.to_string(),
                taxon: None,
                doi: None,
            },
            canonical_smiles: None,
            inchikey: None,
            inchi: None,
            formula: None,
            exact_mass: None,
            mass_warning: None,
            wikidata_qid: None,
            status: if deps.is_empty() {
                CurationStatus::NewCompound
            } else {
                CurationStatus::PendingDependencies
            },
            note: String::new(),
            dependency_blocks: deps.into_iter().map(str::to_string).collect(),
            quickstatements: qs.into_iter().map(str::to_string).collect(),
        }
    }

    #[test]
    fn format_curation_error_maps_rate_limit_messages() {
        let msg = format_curation_error(Locale::En, "HTTP 429 from service");
        assert!(msg.to_ascii_lowercase().contains("rate"));
    }

    #[test]
    fn second_pass_inputs_only_keeps_rows_with_dependencies() {
        let rows = vec![
            CurationResultRow {
                input: CurationInputRow {
                    name: "A".into(),
                    smiles: "CCO".into(),
                    taxon: None,
                    doi: None,
                },
                canonical_smiles: None,
                inchikey: None,
                inchi: None,
                formula: None,
                exact_mass: None,
                mass_warning: None,
                wikidata_qid: None,
                status: CurationStatus::NewCompound,
                note: String::new(),
                dependency_blocks: vec![],
                quickstatements: vec![],
            },
            CurationResultRow {
                input: CurationInputRow {
                    name: "B".into(),
                    smiles: "CCN".into(),
                    taxon: None,
                    doi: None,
                },
                canonical_smiles: None,
                inchikey: None,
                inchi: None,
                formula: None,
                exact_mass: None,
                mass_warning: None,
                wikidata_qid: None,
                status: CurationStatus::PendingDependencies,
                note: String::new(),
                dependency_blocks: vec!["DEP".into()],
                quickstatements: vec![],
            },
        ];

        let inputs = second_pass_inputs(&rows);
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].name, "B");
    }

    #[test]
    fn apply_second_pass_updates_matching_rows_and_keeps_order() {
        let previous = vec![
            row("A", "CCO", vec!["DEP-A"], vec![]),
            row("B", "CCN", vec![], vec!["MAIN-B"]),
        ];
        let updated = vec![
            row("A", "CCO", vec![], vec!["MAIN-A"]),
            row("C", "CCC", vec![], vec!["MAIN-C"]),
        ];

        let outcome = apply_second_pass(Locale::En, &previous, updated);

        assert_eq!(outcome.result_rows.len(), 2);
        assert_eq!(outcome.result_rows[0].input.name, "A");
        assert_eq!(
            outcome.result_rows[0].dependency_blocks.len(),
            0,
            "expected no entries"
        );
        assert_eq!(outcome.result_rows[0].quickstatements, vec!["MAIN-A"]);
        assert_eq!(outcome.result_rows[1].input.name, "B");
    }

    #[test]
    fn apply_second_pass_marks_done_when_dependencies_cleared() {
        let previous = vec![row("A", "CCO", vec!["DEP-A"], vec![])];
        let updated = vec![row("A", "CCO", vec![], vec!["MAIN-A"])];

        let outcome = apply_second_pass(Locale::En, &previous, updated);

        assert!(!outcome.awaiting_second_pass);
        assert_eq!(outcome.status_message, msg_second_pass_done(Locale::En));
        assert_eq!(
            outcome.quickstatements.dependencies.len(),
            0,
            "expected no entries"
        );
        assert_eq!(outcome.quickstatements.main.as_ref(), "MAIN-A");
    }
}
