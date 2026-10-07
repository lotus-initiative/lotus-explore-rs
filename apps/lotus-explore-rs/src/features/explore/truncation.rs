// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Recognising the endpoint's own truncation notice, and the reason it carries.
//!
//! `lotus-query` writes one English sentence when a body ends in the notice
//! `QLever` appends to a `200` whose query ran out of time. That sentence is a
//! correct message for a log or a CLI, and it is the *only* thing that says the
//! result set is partial rather than empty -- but it arrives here as a bare
//! `String` through four layers, and every layer treated it as a generic parse
//! failure. So a French or German reader got a translated frame with an English
//! sentence inside it, and no `ErrorKind::Truncated`, so the retry decision and
//! the truncation hint were both wrong as well.
//!
//! One function, used by both the classifier and the presenter, because the two
//! must not be able to disagree: a truncation the presenter recognises as
//! ordinary prose is exactly the bug this exists to fix. This is the same shape
//! as `classify_http_error` reading "timed out" out of a `429` body -- the detail
//! is prose, and the prose is all there is.

/// The clause that identifies the message, before the endpoint's reason.
///
/// Long enough to be unambiguous, and derived from the sentence
/// `lotus_query::parse::stream::truncated_by_endpoint` writes. Both halves of
/// that pair live in one file on purpose: a change to the wording in `lotus-query`
/// has to change this string too, and the tests below fail when it does not.
const NOTICE_CLAUSE: &str = "the endpoint stopped sending before the result was complete";

/// The clause the reason sits in, and what ends it.
///
/// The closing is a semicolon, not a parenthesis: `lotus-query` writes
/// `... (the endpoint said: X); re-run the search ...` and never closes the
/// bracket, so a `")"` terminator reads to the end of the message and returns
/// the trailing advice as though it were the endpoint's reason.
const REASON_OPEN: &str = "(the endpoint said: ";
const REASON_CLOSE: &str = ");";

/// The endpoint's own reason, if this detail is its truncation notice.
///
/// `None` for every other parse failure, which is the case this exists to
/// distinguish.
#[must_use]
pub fn truncation_reason(detail: &str) -> Option<&str> {
    let after_notice = detail.split_once(NOTICE_CLAUSE)?.1;
    let after_open = after_notice.split_once(REASON_OPEN)?.1;
    // `.0`, not `.1`: the reason is what comes BEFORE the terminator, and
    // taking the other half returns the advice that follows it.
    let reason = after_open.split_once(REASON_CLOSE)?.0;
    let reason = reason.trim();
    (!reason.is_empty()).then_some(reason)
}

/// The known reasons, as `QLever` words them.
///
/// Matched case-insensitively and without the trailing stop, because the notice
/// is theirs and their punctuation is not a contract. Anything not in this list
/// is still reported -- as "a reason it did not give" -- rather than dropped,
/// because an unrecognised reason is still a reason.
const KNOWN_REASONS: [(&str, Reason); 2] = [
    ("operation timed out", Reason::TimedOut),
    ("query was canceled", Reason::Cancelled),
];

/// What the endpoint said, reduced to the two cases a reader can act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The query ran out of its time budget. A narrower query finishes.
    TimedOut,
    /// The query was cancelled. Retrying may work; the endpoint decided.
    Cancelled,
    /// The endpoint named a reason this build does not know.
    Other,
}

impl Reason {
    /// Classify the endpoint's own wording.
    #[must_use]
    pub fn of(reason: &str) -> Self {
        let folded = reason.trim().trim_end_matches('.').to_ascii_lowercase();
        KNOWN_REASONS
            .iter()
            .find(|(needle, _)| folded == *needle)
            .map_or(Self::Other, |(_, kind)| *kind)
    }
}

#[cfg(test)]
#[path = "truncation/tests.rs"]
mod tests;
