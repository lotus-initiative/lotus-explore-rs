// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Search request envelope shared across the explore orchestration pipeline.

use crate::features::explore::actions::ExploreAction;
use crate::features::explore::command::SearchCommand;
use lotus_search::SearchCriteria;

#[derive(Clone, Debug, PartialEq)]
pub struct SearchRequest {
    criteria: SearchCriteria,
    command: SearchCommand,
    request_token: u64,
}

impl SearchRequest {
    #[must_use]
    pub const fn new(criteria: SearchCriteria, command: SearchCommand) -> Self {
        Self {
            criteria,
            command,
            request_token: 0,
        }
    }

    #[must_use]
    pub const fn with_request_token(mut self, request_token: u64) -> Self {
        self.request_token = request_token;
        self
    }

    #[must_use]
    pub fn as_action(&self) -> ExploreAction {
        ExploreAction::SearchRequested {
            criteria_snapshot: self.criteria.clone(),
            command: self.command,
        }
    }

    #[must_use]
    pub const fn criteria(&self) -> &SearchCriteria {
        &self.criteria
    }

    #[must_use]
    pub const fn direct_download(&self) -> bool {
        self.command.direct_download()
    }

    pub(super) const fn command(&self) -> SearchCommand {
        self.command
    }

    #[must_use]
    pub const fn request_token(&self) -> u64 {
        self.request_token
    }
}

#[cfg(test)]
mod tests {
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
}
