// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Search request envelope shared across the explore orchestration pipeline.

use crate::explore::actions::ExploreAction;
use crate::explore::command::SearchCommand;
use lotus_search::SearchCriteria;

/// A search request: the criteria, why it was made, and its request token.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchRequest {
    criteria: SearchCriteria,
    command: SearchCommand,
    request_token: u64,
}

impl SearchRequest {
    /// Create a request for `criteria` with a request token of zero.
    #[must_use]
    pub const fn new(criteria: SearchCriteria, command: SearchCommand) -> Self {
        Self {
            criteria,
            command,
            request_token: 0,
        }
    }

    /// Attach the token that identifies this request among concurrent searches.
    #[must_use]
    pub const fn with_request_token(mut self, request_token: u64) -> Self {
        self.request_token = request_token;
        self
    }

    /// The reducer action that starts this request.
    #[must_use]
    pub fn as_action(&self) -> ExploreAction {
        ExploreAction::SearchRequested {
            criteria_snapshot: self.criteria.clone(),
            command: self.command,
        }
    }

    /// The criteria this request will search with.
    #[must_use]
    pub const fn criteria(&self) -> &SearchCriteria {
        &self.criteria
    }

    /// Whether this request prepares a download differently from an interactive search.
    #[must_use]
    pub const fn direct_download(&self) -> bool {
        self.command.direct_download()
    }

    /// The command that triggered this request.
    #[allow(dead_code)]
    #[must_use]
    pub const fn command(&self) -> SearchCommand {
        self.command
    }

    /// The token that identifies this request among concurrent searches.
    #[must_use]
    pub const fn request_token(&self) -> u64 {
        self.request_token
    }
}

#[cfg(test)]
#[path = "request/tests.rs"]
mod tests;
