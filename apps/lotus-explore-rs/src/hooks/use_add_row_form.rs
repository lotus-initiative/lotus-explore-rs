// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Hook for managing the "add one row" form state in the data curation page.

use crate::curation::CurationInputRow;
use crate::features::curation::queue::{append_unique_rows, non_empty_trimmed};
use crate::i18n::{Locale, msg_duplicate_row_skipped, msg_name_smiles_required};
use dioxus::prelude::*;

/// Reactive handle for the "add one row" form signals.
/// Because every field is a [`Signal`] (which is `Copy`), this struct is
/// itself `Copy` and can be captured by `move` closures without cloning.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AddRowForm {
    pub name: Signal<String>,
    pub smiles: Signal<String>,
    pub taxon: Signal<String>,
    pub doi: Signal<String>,
}

impl AddRowForm {
    /// Validate the current inputs and, if valid, append a new
    /// [`CurationInputRow`] to `rows`.
    pub fn try_add(
        mut self,
        locale: Locale,
        mut rows: Signal<Vec<CurationInputRow>>,
        mut status: Signal<Option<String>>,
    ) {
        let name = self.name.read().trim().to_string();
        let smiles = self.smiles.read().trim().to_string();
        if name.is_empty() || smiles.is_empty() {
            *status.write() = Some(msg_name_smiles_required(locale));
            return;
        }
        let row = CurationInputRow {
            name,
            smiles,
            taxon: non_empty_trimmed(&self.taxon.read()),
            doi: non_empty_trimmed(&self.doi.read()),
        };
        if append_unique_rows(&mut rows.write(), vec![row]).skipped > 0 {
            *status.write() = Some(msg_duplicate_row_skipped(locale));
            return;
        }
        *status.write() = None;
        self.name.set(String::new());
        self.smiles.set(String::new());
        self.taxon.set(String::new());
        self.doi.set(String::new());
    }
}

/// Create signals for the "add one row" form.
/// Must be called unconditionally inside a Dioxus component or hook
/// (same rules as all `use_*` hooks).
#[must_use]
pub fn use_add_row_form() -> AddRowForm {
    AddRowForm {
        name: use_signal(String::new),
        smiles: use_signal(String::new),
        taxon: use_signal(String::new),
        doi: use_signal(String::new),
    }
}
