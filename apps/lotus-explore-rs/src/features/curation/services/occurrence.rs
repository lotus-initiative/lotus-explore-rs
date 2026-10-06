// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The `P703` statement recording where a compound was found.
//!
//! The statement's shape depends on which of the taxon and the reference
//! resolved, and the same choice is made twice — for an existing compound and for
//! a created one — so it is made once here.

use super::WD_OCCURS_IN_TAXON_PROP;

/// The taxon and reference a row resolved to.
///
/// `row_has_doi` distinguishes "the row named no reference" from "the row named
/// one that has no `Wikidata` item yet", because only the second has to wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved<'a> {
    /// The taxon item, if it was found.
    pub taxon_qid: Option<&'a str>,
    /// The reference item, if it was found.
    pub reference_qid: Option<&'a str>,
    /// Whether the row named a `DOI` at all.
    pub row_has_doi: bool,
}

impl Resolved<'_> {
    /// Whether the reference has to exist before `P703` can name it.
    ///
    /// A row naming a `DOI` with no item yet is the case: the reference's own
    /// statements go in the dependency block and `P703` goes in a later pass.
    fn reference_is_pending(&self) -> bool {
        self.row_has_doi && self.taxon_qid.is_some() && self.reference_qid.is_none()
    }

    /// The `P703` line for a compound that already exists, keyed on `qid`.
    ///
    /// `None` means the statement has to wait for the reference item.
    pub(crate) fn statement_for(&self, qid: &str) -> Option<String> {
        if self.reference_is_pending() {
            return None;
        }
        Some(match (self.taxon_qid, self.reference_qid) {
            (Some(taxon), Some(reference)) => {
                format!("{qid}|{WD_OCCURS_IN_TAXON_PROP}|{taxon}|S248|{reference}")
            }
            (Some(taxon), None) => format!("{qid}|{WD_OCCURS_IN_TAXON_PROP}|{taxon}"),
            // No taxon resolved: the dependency block creates it, so the
            // statement that names it is written on the next pass.
            (None, _) => format!("{qid}|{WD_OCCURS_IN_TAXON_PROP}|[NEW_TAXON_QID]"),
        })
    }

    /// The `P703` line for a compound being created, written against `LAST`.
    ///
    /// `None` means the statement has to wait for the reference item.
    pub(crate) fn statement_for_new(&self) -> Option<String> {
        self.statement_for("LAST")
    }
}

#[cfg(test)]
#[path = "occurrence/tests.rs"]
mod tests;
