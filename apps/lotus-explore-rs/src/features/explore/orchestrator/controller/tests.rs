// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `controller`, in their own file.

use super::*;

#[test]
fn duplicate_search_is_suppressed_until_the_run_finishes() {
    let controller = SearchTaskController::new();
    let criteria = SearchCriteria::up_to_year(crate::clock::current_year());

    let first = controller.try_begin(&criteria, SearchCommand::Interactive);
    assert!(first.is_some());
    assert!(
        controller
            .try_begin(&criteria, SearchCommand::Interactive)
            .is_none()
    );

    let run_id = first.unwrap_or_default();
    controller.finish(run_id);
    assert!(
        controller
            .try_begin(&criteria, SearchCommand::Interactive)
            .is_some()
    );
}

#[test]
fn a_different_command_starts_a_new_run() {
    let controller = SearchTaskController::new();
    let criteria = SearchCriteria::up_to_year(crate::clock::current_year());

    assert!(
        controller
            .try_begin(&criteria, SearchCommand::Interactive)
            .is_some()
    );
    assert!(
        controller
            .try_begin(&criteria, SearchCommand::StartupDownload)
            .is_some()
    );
}

#[test]
fn a_stale_completion_does_not_clear_the_current_run() {
    let controller = SearchTaskController::new();
    let criteria = SearchCriteria::up_to_year(crate::clock::current_year());
    let first = controller.try_begin(&criteria, SearchCommand::Interactive);
    let second = controller.try_begin(&criteria, SearchCommand::StartupDownload);
    assert!(first.is_some());
    assert!(second.is_some());
    let first = first.unwrap_or_default();
    let second = second.unwrap_or_default();

    controller.finish(first);
    assert!(
        controller
            .try_begin(&criteria, SearchCommand::StartupDownload)
            .is_none()
    );

    controller.finish(second);
    assert!(
        controller
            .try_begin(&criteria, SearchCommand::StartupDownload)
            .is_some()
    );
}
