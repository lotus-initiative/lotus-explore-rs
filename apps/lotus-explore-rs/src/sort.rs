// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! How the results table is ordered.
//!
//! This is presentation state, not domain vocabulary: nothing in a query or a
//! result row knows about it, and the same state travels from the table to
//! the `/v1/search` endpoint. It lives in the app because the app owns both
//! ends of that conversation.

/// Column by which results can be sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    /// The compound's name.
    Name,
    /// Exact mass, which is numeric and so the one worth ordering.
    Mass,
    /// Molecular formula, ordered as written.
    Formula,
    /// The reporting taxon's name.
    TaxonName,
    /// The reference's publication year.
    PubYear,
    /// The reference's title.
    RefTitle,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    /// Smallest first.
    Asc,
    /// Largest first.
    Desc,
}

/// Which column, and which way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortState {
    /// The column to order by.
    pub col: SortColumn,
    /// The direction to order in.
    pub dir: SortDir,
}

impl Default for SortState {
    /// By name, ascending: what a reader expects from an unlabelled table of
    /// names, and the cheapest order to produce.
    fn default() -> Self {
        Self {
            col: SortColumn::Name,
            dir: SortDir::Asc,
        }
    }
}
