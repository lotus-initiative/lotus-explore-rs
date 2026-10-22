// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! How many rows one request may ask for.
//!
//! One number, and it is a payload ceiling rather than a rendering one. The
//! interactive path has no row ceiling at all: it asks for every row, streams the
//! answer into a columnar set, and takes its counts from that set. The set holds
//! a row as three dictionary ids and the table materialises only the rows on
//! screen, so rendering cost is not coupled to the result size and a phone
//! filtering three million rows is not slower than a desktop filtering five
//! hundred — the filters compile to one bitmap per dictionary.

/// The most rows one API request may return.
///
/// The constraint is bytes in flight, not rows, and it is the caller's to accept:
/// a bulk caller asking for a million rows is asking for them on purpose, and
/// refusing is the server's opinion of a decision that is not the server's. The
/// same million the in-memory export path allows, so the two ceilings do not
/// disagree about what a large export is.
pub const API_MAX_ROWS: usize = 1_000_000;

#[cfg(test)]
#[path = "table_budget/tests.rs"]
mod tests;
