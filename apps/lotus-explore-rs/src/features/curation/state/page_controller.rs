// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::curation::{
    CurationError, CurationInputRow, CurationResultRow, QuickStatementsBundle, parse_tsv_rows,
};
use crate::features::curation::queue::append_unique_rows;
use crate::features::curation::workflow;
use crate::hooks::use_add_row_form;
use crate::hooks::use_add_row_form::AddRowForm;
use crate::i18n::{
    Locale, msg_no_valid_tsv_rows, msg_running_checks, msg_tsv_import_complete,
    msg_tsv_missing_column,
};
use dioxus::prelude::*;
use std::sync::Arc;

use crate::curation::{
    curate_rows, example_rows, initial_curation_autorun_from_url, initial_curation_rows_from_url,
};
use crate::i18n::{msg_add_row_before_generate, msg_examples_loaded, msg_second_pass_running};

#[derive(Clone, Copy)]
pub struct CurationPageController {
    pub locale: Locale,
    pub form: AddRowForm,
    pub tsv_input: Signal<String>,
    pub rows: Signal<Vec<CurationInputRow>>,
    pub processing: Signal<bool>,
    pub status_message: Signal<Option<String>>,
    pub result_rows: Signal<Arc<[CurationResultRow]>>,
    pub quickstatements: Signal<QuickStatementsBundle>,
    pub awaiting_second_pass: Signal<bool>,
    autorun_pending: Signal<bool>,
}

impl CurationPageController {
    pub fn has_tsv_input(self) -> bool {
        !self.tsv_input.read().trim().is_empty()
    }

    pub fn add_row(self) {
        self.form
            .try_add(self.locale, self.rows, self.status_message);
    }

    pub fn parse_tsv(self) {
        let content = self.tsv_input.read();
        import_tsv_rows(self.locale, &content, self.rows, self.status_message);
    }

    pub fn process(mut self) {
        if self.rows.read().is_empty() {
            self.status_message
                .set(Some(msg_add_row_before_generate(self.locale)));
            return;
        }
        let snapshot = self.rows.read().clone();
        start_curation_run(
            self.locale,
            snapshot,
            self.processing,
            self.status_message,
            self.result_rows,
            self.quickstatements,
            self.awaiting_second_pass,
        );
    }

    pub fn maybe_autorun(mut self) {
        if !should_autorun(
            *self.autorun_pending.read(),
            self.rows.read().len(),
            *self.processing.read(),
            self.result_rows.read().len(),
        ) {
            return;
        }

        let snapshot = self.rows.read().clone();
        self.autorun_pending.set(false);
        start_curation_run(
            self.locale,
            snapshot,
            self.processing,
            self.status_message,
            self.result_rows,
            self.quickstatements,
            self.awaiting_second_pass,
        );
    }

    pub fn load_example_rows(mut self) {
        let samples = example_rows();
        self.tsv_input.set(rows_to_tsv(&samples));
        let outcome = append_unique_rows(&mut self.rows.write(), samples);
        self.status_message.set(Some(msg_examples_loaded(
            self.locale,
            outcome.added,
            outcome.skipped,
        )));
    }

    pub fn import_uploaded_tsv(mut self, content: String) {
        import_tsv_rows(self.locale, &content, self.rows, self.status_message);
        self.tsv_input.set(content);
    }

    pub fn run_second_pass(mut self) {
        let pending_inputs = workflow::second_pass_inputs(self.result_rows.read().as_ref());
        if pending_inputs.is_empty() {
            self.awaiting_second_pass.set(false);
            return;
        }

        let previous_rows = self.result_rows.read().clone();
        self.processing.set(true);
        self.status_message
            .set(Some(msg_second_pass_running(self.locale).into()));

        spawn(async move {
            match curate_rows(self.locale, pending_inputs).await {
                Ok((updated_rows, _)) => {
                    let outcome =
                        workflow::apply_second_pass(self.locale, &previous_rows, updated_rows);
                    self.result_rows.set(outcome.result_rows);
                    self.quickstatements.set(outcome.quickstatements);
                    self.awaiting_second_pass.set(outcome.awaiting_second_pass);
                    self.processing.set(false);
                    self.status_message.set(Some(outcome.status_message));
                }
                Err(err) => {
                    self.processing.set(false);
                    self.status_message
                        .set(Some(workflow::format_curation_error_typed(
                            self.locale,
                            &err,
                        )));
                }
            }
        });
    }
}

#[must_use]
pub fn use_curation_page_controller(locale: Locale) -> CurationPageController {
    CurationPageController {
        locale,
        form: use_add_row_form(),
        tsv_input: use_signal(String::new),
        rows: use_signal(initial_curation_rows_from_url),
        processing: use_signal(|| false),
        status_message: use_signal(|| Option::<String>::None),
        result_rows: use_signal(|| Arc::<[CurationResultRow]>::from([])),
        quickstatements: use_signal(QuickStatementsBundle::default),
        awaiting_second_pass: use_signal(|| false),
        autorun_pending: use_signal(initial_curation_autorun_from_url),
    }
}

pub const fn should_autorun(
    autorun_pending: bool,
    queued_rows: usize,
    processing: bool,
    result_rows: usize,
) -> bool {
    autorun_pending && queued_rows > 0 && !processing && result_rows == 0
}

/// Snapshot of commonly-queried UI state flags. Used to reduce signal reads and prevent
/// unnecessary component re-renders.
/// The booleans are independent flags read by separate components; packing them into a
/// state-machine enum would couple unrelated rendering concerns.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CurationUiState {
    pub processing: bool,
    pub awaiting_second_pass: bool,
    pub has_rows: bool,
    pub has_results: bool,
    pub has_status_message: bool,
}

impl CurationUiState {
    // `CurationPageController` is a bundle of Dioxus `Signal`s (Copy handles);
    // passing it by value is the framework idiomatic style and avoids a borrow
    // of a signal `Readable` guard.
    #[allow(clippy::large_types_passed_by_value)]
    pub fn from_controller(controller: CurationPageController) -> Self {
        Self {
            processing: *controller.processing.read(),
            awaiting_second_pass: *controller.awaiting_second_pass.read(),
            has_rows: !controller.rows.read().is_empty(),
            has_results: !controller.result_rows.read().is_empty(),
            has_status_message: controller.status_message.read().is_some(),
        }
    }
}

pub fn start_curation_run(
    locale: Locale,
    snapshot: Vec<CurationInputRow>,
    mut processing: Signal<bool>,
    mut status_message: Signal<Option<String>>,
    mut result_rows: Signal<Arc<[CurationResultRow]>>,
    mut quickstatements: Signal<QuickStatementsBundle>,
    mut awaiting_second_pass: Signal<bool>,
) {
    processing.set(true);
    status_message.set(Some(msg_running_checks(locale)));
    spawn(async move {
        match workflow::run_curation(locale, snapshot).await {
            Ok(outcome) => {
                awaiting_second_pass.set(outcome.awaiting_second_pass);
                result_rows.set(outcome.result_rows);
                quickstatements.set(outcome.quickstatements);
                processing.set(false);
                status_message.set(Some(outcome.status_message));
            }
            Err(err) => {
                processing.set(false);
                status_message.set(Some(workflow::format_curation_error_typed(locale, &err)));
            }
        }
    });
}

pub fn import_tsv_rows(
    locale: Locale,
    content: &str,
    mut rows: Signal<Vec<CurationInputRow>>,
    mut status_message: Signal<Option<String>>,
) {
    match parse_tsv_rows(content) {
        Ok(parsed) => {
            if parsed.is_empty() {
                status_message.set(Some(msg_no_valid_tsv_rows(locale)));
            } else {
                let outcome = append_unique_rows(&mut rows.write(), parsed);
                status_message.set(Some(msg_tsv_import_complete(
                    locale,
                    outcome.added,
                    outcome.skipped,
                )));
            }
        }
        Err(CurationError::MissingTsvColumn(column)) => {
            status_message.set(Some(msg_tsv_missing_column(locale, column)));
        }
        Err(err) => {
            status_message.set(Some(err.to_string()));
        }
    }
}

pub fn rows_to_tsv(rows: &[CurationInputRow]) -> String {
    // Pre-allocate: header + proportional estimate per row
    let mut out = String::with_capacity(24 + rows.len() * 64);
    out.push_str("name\tsmiles\ttaxon\tdoi\n");
    for row in rows {
        push_tsv_cell(&mut out, &row.name);
        out.push('\t');
        push_tsv_cell(&mut out, &row.smiles);
        out.push('\t');
        push_tsv_cell(&mut out, row.taxon.as_deref().unwrap_or(""));
        out.push('\t');
        push_tsv_cell(&mut out, row.doi.as_deref().unwrap_or(""));
        out.push('\n');
    }
    out
}

/// Append a sanitized TSV cell into `buf`. Replaces tab/CR/LF with spaces,
/// then trims the result — without any intermediate heap allocation.
fn push_tsv_cell(buf: &mut String, value: &str) {
    // Trim ASCII whitespace on both ends first, avoiding inner allocations
    let trimmed = value.trim();
    for ch in trimmed.chars() {
        if matches!(ch, '\t' | '\r' | '\n') {
            buf.push(' ');
        } else {
            buf.push(ch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_tsv_cell(value: &str) -> String {
        let mut buf = String::with_capacity(value.len());
        push_tsv_cell(&mut buf, value);
        buf
    }

    #[test]
    fn clean_tsv_cell_normalizes_multiline_and_tabs() {
        assert_eq!(clean_tsv_cell("  A\tB\nC\r "), "A B C");
    }

    #[test]
    fn rows_to_tsv_writes_expected_header_and_cells() {
        let rows = vec![CurationInputRow {
            name: "A\nname".into(),
            smiles: "CCO".into(),
            taxon: Some("Tax\ton".into()),
            doi: Some("10.1/ABC".into()),
        }];

        let tsv = rows_to_tsv(&rows);
        assert!(tsv.starts_with("name\tsmiles\ttaxon\tdoi\n"));
        assert!(tsv.contains("A name\tCCO\tTax on\t10.1/ABC\n"));
    }

    #[test]
    fn should_autorun_only_when_pending_with_rows_and_no_results() {
        assert!(should_autorun(true, 2, false, 0));
        assert!(!should_autorun(false, 2, false, 0));
        assert!(!should_autorun(true, 0, false, 0));
        assert!(!should_autorun(true, 2, true, 0));
        assert!(!should_autorun(true, 2, false, 1));
    }
}
