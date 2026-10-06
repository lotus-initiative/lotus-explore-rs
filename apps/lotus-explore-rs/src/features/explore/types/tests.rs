// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `types`, in their own file.

#![allow(clippy::panic)]

use super::*;

#[test]
fn transport_domain_error_exposes_repository_source() {
    let err = DomainError::transport(
        QueryStage::ResultsQuery,
        crate::repositories::RepositoryError::network("timeout"),
    );

    match err {
        DomainError::Transport { source, .. } => {
            assert_eq!(source.to_string(), "network error: timeout");
        }
        _ => panic!("expected transport error"),
    }
}

#[test]
fn transport_http_4xx_is_classified_as_bad_request() {
    let err = DomainError::transport(
        QueryStage::ResultsQuery,
        crate::repositories::RepositoryError::Http {
            status: 400,
            body: "invalid query".into(),
        },
    );

    assert_eq!(err.kind(), ErrorKind::BadRequest);
}

#[test]
fn transport_network_is_classified_as_network() {
    let err = DomainError::transport(
        QueryStage::ResultsQuery,
        crate::repositories::RepositoryError::network("timeout"),
    );

    assert_eq!(err.kind(), ErrorKind::Network);
}

#[test]
fn transport_parse_is_classified_as_parse() {
    let err = DomainError::transport(
        QueryStage::ResultsQuery,
        crate::repositories::RepositoryError::parse("csv decode failed"),
    );

    assert_eq!(err.kind(), ErrorKind::Parse);
}

#[test]
fn transport_not_configured_is_classified_as_configuration() {
    let err = DomainError::transport(
        QueryStage::ResultsQuery,
        crate::repositories::RepositoryError::NotConfigured,
    );

    assert_eq!(err.kind(), ErrorKind::Configuration);
}

#[test]
fn transport_http_syntax_error_is_classified_as_bad_request() {
    let err = DomainError::transport(
        QueryStage::ResultsQuery,
        crate::repositories::RepositoryError::Http {
            status: 400,
            body: "Invalid SPARQL query: mismatched input 'AS' expecting ','".into(),
        },
    );

    assert_eq!(err.kind(), ErrorKind::BadRequest);
}

#[test]
fn transport_at_closure_maps_repository_error() {
    let mapper = DomainError::transport_at(QueryStage::ResultsQuery);
    let err = mapper(crate::repositories::RepositoryError::network("timed out"));
    assert!(matches!(
        err,
        DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            ..
        }
    ));
}
