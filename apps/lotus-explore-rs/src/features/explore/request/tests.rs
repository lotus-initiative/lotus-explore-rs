// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `request`, in their own file.

#![allow(clippy::panic)]

use super::SearchRequest;
use crate::features::explore::command::SearchCommand;
use lotus_search::SearchCriteria;

#[test]
fn action_preserves_criteria_and_command() {
    let request = SearchRequest::new(
        SearchCriteria {
            taxon: "Fungi".to_string(),
            ..SearchCriteria::up_to_year(crate::clock::current_year())
        },
        SearchCommand::StartupDownload,
    );

    let action = request.as_action();
    match action {
        crate::features::explore::actions::ExploreAction::SearchRequested {
            criteria_snapshot,
            command,
        } => {
            assert_eq!(criteria_snapshot.taxon, "Fungi");
            assert_eq!(command, SearchCommand::StartupDownload);
        }
        _ => panic!("expected SearchRequested action"),
    }
}
