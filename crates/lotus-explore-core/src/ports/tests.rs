// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `ports`, in their own file.

#![allow(clippy::panic)]
#![allow(
    clippy::unused_async_trait_impl,
    reason = "the trait declares these methods `async`; the stub has nothing to await"
)]

use super::*;

/// A transport that implements only the required methods, so every call to a
/// provided method reaches the trait's default.
#[derive(Clone)]
struct BodyOnly;

impl LotusRepository for BodyOnly {
    async fn api_search(
        &self,
        _criteria: &SearchCriteria,
        _limit: usize,
        _include_counts: bool,
    ) -> Option<Result<SearchResponse, RepositoryError>> {
        None
    }

    async fn sparql_body(
        &self,
        _query: &str,
    ) -> Result<lotus_search::ResponseBody, RepositoryError> {
        Err(RepositoryError::network("unused"))
    }
}

#[tokio::test]
async fn the_default_sparql_columnar_refuses_instead_of_answering_empty() {
    let mut calls = 0_usize;
    let outcome = BodyOnly
        .sparql_columnar("SELECT * WHERE {}", &mut |_| calls += 1)
        .await;

    match outcome {
        Err(error) => assert_eq!(
            error,
            RepositoryError::network("this transport cannot read a response body in chunks")
        ),
        Ok(_) => panic!("the default must refuse, not hand back an empty result set"),
    }
    assert_eq!(calls, 0);
}
