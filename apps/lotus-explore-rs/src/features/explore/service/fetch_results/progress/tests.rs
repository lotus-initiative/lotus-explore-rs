// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `progress`, in their own file.

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
    // superseded attempt, offering a count already passed.
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
