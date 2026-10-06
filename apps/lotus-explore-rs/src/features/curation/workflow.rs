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
#[path = "workflow/tests.rs"]
mod tests;
