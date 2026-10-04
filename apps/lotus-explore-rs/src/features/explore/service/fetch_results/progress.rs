// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! How often a row count is worth telling the UI about.
//!
//! Its own module, compiled on every target, because the alternative is a policy
//! that only exists inside a `cfg(target_arch = "wasm32")` file -- and a policy
//! whose tests never run is a policy nobody checks. There is nothing wasm-specific
//! about counting rows.

/// Rows that must arrive between two progress reports.
///
/// The transport reports once per chunk and a wide search is tens of thousands of
/// chunks, so reporting every one would re-render the overlay more often than a
/// reader can see it, and cost more than the parse it is reporting on. At ~16 KB
/// a chunk this is a handful of updates per second, which is more often than the
/// number changes visibly.
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
    /// `total` is the count so far; it is not called with a total to reach,
    /// because there is none.
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
    /// Separate from [`Self::offer`] because the last chunk is as likely to fall
    /// short of the step as anything else, and it is the one that matters.
    #[must_use]
    pub fn finish(&self) -> Option<usize> {
        self.reported
    }
}

#[cfg(test)]
mod tests {
    use super::{PROGRESS_ROW_STEP, ProgressThrottle};

    #[test]
    fn a_wide_search_reports_a_handful_of_times_not_once_per_chunk() {
        let mut throttle = ProgressThrottle::new(PROGRESS_ROW_STEP);
        let mut shown = 0;
        // 100,000 chunks of ten rows each: a million rows, which is the scale
        // this exists for.
        for chunk in 0..100_000usize {
            if throttle.offer(chunk * 10).is_some() {
                shown += 1;
            }
        }
        assert_eq!(
            shown, 200,
            "a million rows should report about once per {PROGRESS_ROW_STEP} rows"
        );
    }

    #[test]
    fn the_first_report_happens_even_with_no_rows() {
        let mut throttle = ProgressThrottle::new(PROGRESS_ROW_STEP);
        assert_eq!(
            throttle.offer(0),
            Some(0),
            "a query that has produced nothing yet is exactly when to say so"
        );
    }

    #[test]
    fn a_count_within_the_step_of_the_last_one_is_not_reported() {
        // The step is measured from the last *reported* count, not from the last
        // chunk, so a burst of small chunks between two reports is silent.
        let mut throttle = ProgressThrottle::new(PROGRESS_ROW_STEP);
        assert_eq!(throttle.offer(10), Some(10));
        for rows in [11, 500, 4_999, 5_009] {
            assert_eq!(
                throttle.offer(rows),
                None,
                "{rows} is within the step of 10"
            );
        }
        assert_eq!(throttle.offer(5_010), Some(5_010));
    }

    #[test]
    fn a_report_smaller_than_the_last_one_is_dropped() {
        // The stale-chunk case: a retry restarting the fetch, or a callback from a
        // superseded attempt, offering a count the reader has already passed.
        let mut throttle = ProgressThrottle::new(100);
        let mut shown = Vec::new();
        for chunk in [0usize, 500, 200, 900, 100_000] {
            if let Some(rows) = throttle.offer(chunk) {
                shown.push(rows);
            }
        }
        assert_eq!(
            shown,
            vec![0, 500, 900, 100_000],
            "200 is behind 500 and must be dropped; 900 is ahead and must not be"
        );
        assert!(
            shown.iter().copied().is_sorted(),
            "what the reader sees must only ever increase: {shown:?}"
        );
    }

    #[test]
    fn the_last_count_is_always_available_even_when_short_of_the_step() {
        let mut throttle = ProgressThrottle::new(1_000);
        let _ = throttle.offer(10);
        assert_eq!(throttle.finish(), Some(10));
    }
}
