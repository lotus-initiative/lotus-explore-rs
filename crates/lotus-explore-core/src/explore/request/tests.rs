// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `request`, in their own file.

#![allow(clippy::panic)]

use super::SearchRequest;
use crate::explore::command::SearchCommand;
use lotus_search::SearchCriteria;

#[test]
fn action_preserves_criteria_and_command() {
    let request = SearchRequest::new(
        SearchCriteria {
            taxon: "Fungi".to_string(),
            ..SearchCriteria::up_to_year(2026)
        },
        SearchCommand::StartupDownload,
    );

    let action = request.as_action();
    match action {
        crate::explore::actions::ExploreAction::SearchRequested {
            criteria_snapshot,
            command,
        } => {
            assert_eq!(criteria_snapshot.taxon, "Fungi");
            assert_eq!(command, SearchCommand::StartupDownload);
        }
        _ => panic!("expected SearchRequested action"),
    }
}

#[test]
fn direct_download_follows_the_command_and_the_token_is_the_one_attached() {
    let request = |command| SearchRequest::new(SearchCriteria::up_to_year(2026), command);

    assert!(request(SearchCommand::StartupDownload).direct_download());
    assert!(!request(SearchCommand::Interactive).direct_download());
    assert!(!request(SearchCommand::StartupExecute).direct_download());

    assert_eq!(request(SearchCommand::Interactive).request_token(), 0);
    assert_eq!(
        request(SearchCommand::Interactive)
            .with_request_token(42)
            .request_token(),
        42
    );
}
