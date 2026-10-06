// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! How often a row count is worth telling the UI about.
//!
//! Its own module, compiled on every target: a policy living inside a
//! `cfg(target_arch = "wasm32")` file has tests that never run. There is nothing
//! wasm-specific about counting rows.

/// Rows that must arrive between two progress reports.
///
/// The transport reports once per chunk and a wide search is tens of thousands of
/// chunks, so reporting every one would re-render the overlay more often than a
/// reader can see it, costing more than the parse it reports on. At ~16 KB a chunk
/// this is a handful of updates per second.
pub const PROGRESS_ROW_STEP: usize = 5_000;

/// Decides which row counts are worth reporting, and remembers the last one.
///
/// Three properties, each of which is a way the indicator could mislead:
///
/// - **The first report always happens.** A query that has produced nothing yet is
///   precisely when silence is worst, so a count of zero is news.
/// - **Reports only increase.** A chunk arriving out of order -- a retry
///   restarting the fetch, a callback from a superseded attempt -- must not put a
///   smaller number on screen than the one already there.
/// - **The final count is always reported**, even when it falls short of the step,
///   so the last line before the result appears is the real one.
#[derive(Debug, Default)]
pub struct ProgressThrottle {
    step: usize,
    reported: Option<usize>,
}

impl ProgressThrottle {
    /// A throttle reporting every `step` rows.
    #[must_use]
    pub const fn new(step: usize) -> Self {
        Self {
            step,
            reported: None,
        }
    }

    /// The row count to show, or `None` to show nothing new.
    ///
    /// `rows` is the count so far; there is no total to reach.
    #[must_use]
    pub fn offer(&mut self, rows: usize) -> Option<usize> {
        let worth_reporting = match self.reported {
            None => true,
            Some(already) => rows >= already.saturating_add(self.step),
        };
        if !worth_reporting {
            return None;
        }
        self.reported = Some(rows);
        Some(rows)
    }

    /// The count to show when the body has finished.
    ///
    /// Separate from [`Self::offer`] because the last chunk is as likely to fall short
    /// of the step as any other, and it is the one that matters.
    #[must_use]
    pub fn finish(&self) -> Option<usize> {
        self.reported
    }
}

#[cfg(test)]
#[path = "progress/tests.rs"]
mod tests;
