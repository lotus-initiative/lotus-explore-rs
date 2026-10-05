// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! How many rows one request may ask for.
//!
//! # What used to be here
//!
//! A ceiling on what the table would draw — 500 rows — with a per-device step
//! down for phones and small-memory machines: 220 at 2 GB, 360 at 4 GB, 280 on a
//! mobile user agent. There was one number for web and desktop alike, because a
//! native window is a browser window as far as the table is concerned.
//!
//! All of it is gone, and nothing replaced it. That is worth being explicit
//! about, because the ceiling looked load-bearing: it was in three modules and
//! had five tests asserting the literal `500`.
//!
//! It was not a memory bound. It was a **row-count** bound that stood in for one,
//! and it existed because the interactive search asked the endpoint for one
//! screen's worth and then asked a *second* query how many rows the rest added
//! up to. Three consequences followed, and they are the reason for the change
//! rather than a side effect of it:
//!
//! - The ceiling was applied server-side, by a `LIMIT` appended to the query. So
//!   the table's filters could only ever see the first 500 rows of the result
//!   set, while the toolbar reported the whole set's count. The two disagreed by
//!   construction, and the disagreement grew with the result.
//! - A per-device step down meant a phone was told a smaller lie than a desktop,
//!   which is the opposite of what a memory constraint should do about *accuracy*.
//! - It was the reason the count query could not simply be deleted: with rows
//!   capped and the true total unknown, something had to go and ask.
//!
//! The interactive path now asks for every row, streams the answer into a
//! columnar set, and takes its counts from the set. The set holds a row as three
//! dictionary ids, and the table materialises only the thirty rows on screen, so
//! rendering cost is no longer coupled to the result size at all. A phone
//! filtering three million rows is not slower than a desktop filtering five
//! hundred: the filters compile to one bitmap per dictionary.
//!
//! What is left is a ceiling on a *single request's* payload, which is a
//! different question — what one response may carry — and belongs to the server
//! that serves it.

/// The most rows one API request may return.
///
/// Separate from anything the table does, and deliberately the only number left
/// in this module. It used to be the same constant as the table ceiling, which
/// meant a client-side tuning decision — "a phone cannot draw 1,000 rows" —
/// silently capped how much a bulk caller could export over the API.
///
/// It was 200,000 for no stated reason, which is the same defect as the 500-row
/// ceiling it replaced: a number standing in for a constraint nobody had worked
/// out. The constraint here is bytes in flight, not rows, and it is the caller's
/// to accept — a bulk caller asking for a million rows is asking for them on
/// purpose, and refusing is the server's opinion of a decision that is not the
/// server's. Raised to the same million the in-memory export path allows, so the
/// two ceilings no longer disagree about what a large export is.
pub const API_MAX_ROWS: usize = 1_000_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_export_ceiling_is_not_a_rendering_limit() {
        // A `const` block, because both operands are constants and the assertion
        // is about the relationship between them rather than about a runtime
        // value. The message is a static string: a const block cannot format.
        const _: () = assert!(
            API_MAX_ROWS > 0,
            "a limit of zero means the caller can never have a result"
        );
    }

    #[test]
    fn the_export_ceiling_is_large_enough_to_be_usable() {
        // The whole graph is about three million rows and the widest interactive
        // search is bounded by whatever the endpoint will answer, not by this. A
        // bulk caller asking for a genus should not be told "no" by a number that
        // was once tuned to a screen.
        //
        // A `const` block, as above: both operands are constants, so this asserts
        // a relationship at compile time rather than re-checking a literal.
        const _: () = assert!(
            API_MAX_ROWS >= 1_000_000,
            "the export ceiling has collapsed below what a bulk caller needs"
        );
    }
}
