// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![expect(
    clippy::future_not_send,
    reason = "the resolver runs inside the search future, which holds a Dioxus Signal"
)]
//! Reference resolution — turns a QID or a DOI into the Wikidata item to constrain
//! the search by.
//!
//! The third and smallest of the three resolutions. A reference's only usable
//! field is its title, which is prose — and matching prose against several hundred
//! thousand references has no useful answer, since a great many titles are the same
//! three words.
//!
//! What it does have is two identifiers that each name exactly one item, both
//! recognised by shape before any request is made:
//!
//! | Typed | Lookup |
//! | --- | --- |
//! | `Q23118` | [`reference_by_qid_query`] — a `VALUES`, nothing to match |
//! | `10.1002/andp.18280880206` | [`reference_by_doi_query`] — `P356`, uppercased |
//!
//! Anything else is refused rather than guessed at. There is no third route to
//! try, and a search silently ignoring a mistyped reference is the failure this
//! avoids.
//!
//! [`reference_by_qid_query`]: lotus_query::reference_by_qid_query
//! [`reference_by_doi_query`]: lotus_query::reference_by_doi_query

use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::types::{DomainError, LookupNotice, QueryStage, ValidationFault};
use crate::repositories::LotusRepository;
use lotus_model::{looks_like_doi, looks_like_reference_qid};
use lotus_query::ReferenceMatch;

/// A resolved reference, plus whatever is worth telling the reader about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceResolution {
    /// The Wikidata item, or `None` when the field was left empty.
    ///
    /// `None` means "no constraint", not "not found": a reference matching nothing
    /// is an error, not an absent filter.
    pub qid: Option<String>,
    pub notices: Vec<LookupNotice>,
}

/// Whether this field is worth a round trip at all.
///
/// False for an empty field, so a search not mentioning a reference does not pay
/// for one. Both recognised shapes do pay: a QID is confirmed to exist, and a DOI
/// is a lookup in its own right.
#[must_use]
pub fn requires_remote_lookup(reference: &str) -> bool {
    let trimmed = reference.trim();
    !trimmed.is_empty() && (looks_like_reference_qid(trimmed) || looks_like_doi(trimmed))
}

/// Resolve the reference field.
///
/// # Errors
/// Returns [`DomainError::Validation`] with
/// [`ValidationFault::ReferenceNotAnIdentifier`] for input that is neither a QID
/// nor a DOI, and [`ValidationFault::ReferenceNotFound`] for one that is and that
/// Wikidata does not have. A transport failure is a [`DomainError::Transport`],
/// reported rather than swallowed.
pub async fn resolve<R: LotusRepository>(
    reference: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
) -> Result<ReferenceResolution, DomainError> {
    let trimmed = reference.trim();
    if trimmed.is_empty() {
        return Ok(ReferenceResolution {
            qid: None,
            notices: Vec::new(),
        });
    }

    let query = if looks_like_reference_qid(trimmed) {
        lotus_query::reference_by_qid_query(trimmed)
    } else if looks_like_doi(trimmed) {
        lotus_query::reference_by_doi_query(trimmed)
    } else {
        // Nothing else identifies a reference, so there is nothing to look up.
        // Saying so beats quietly dropping the constraint.
        return Err(DomainError::Validation(
            ValidationFault::ReferenceNotAnIdentifier {
                input: reference.to_owned(),
            },
        ));
    };

    // At most one row can come back -- a QID names one item, and a DOI is unique
    // in Wikidata -- so the first match is the whole answer.
    let found = run(&query, repo, metrics).await?.into_iter().next();
    match found {
        Some(ReferenceMatch { qid }) => Ok(ReferenceResolution {
            qid: Some(qid),
            notices: Vec::new(),
        }),
        None => Err(DomainError::Validation(
            ValidationFault::ReferenceNotFound {
                input: reference.to_owned(),
            },
        )),
    }
}

async fn run<R: LotusRepository>(
    query: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
) -> Result<Vec<ReferenceMatch>, DomainError> {
    let timer = crate::perf::start_timer("LOTUS:reference_resolution");
    let result = repo.sparql_body(query).await;
    let elapsed = crate::perf::end_timer("LOTUS:reference_resolution", timer);
    metrics.add_network(elapsed);
    let csv = result.map_err(|source| DomainError::Transport {
        stage: QueryStage::TaxonSearch,
        source,
    })?;
    let parsed = lotus_query::parse_reference_lookup_csv(&csv).map_err(|error| {
        DomainError::Parse(crate::features::explore::types::ParseFault::ResultsCsv {
            details: error.to_string(),
        })
    })?;

    // An empty result is a legitimate answer and `resolve` turns it into "not
    // found"; reading it as a transport failure would tell the reader to retry
    // instead of correcting the DOI.
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    // `LotusRepository` is an async-fn-in-trait, so the stub's methods must be
    // `async fn`; a canned reply has nothing to await, and the lint cannot tell
    // the trait signature apart from a choice in the body.
    #![expect(
        clippy::unused_async_trait_impl,
        reason = "LotusRepository is an AFIT: the `async fn` spelling is fixed by the trait"
    )]
    use super::*;
    use crate::api::SearchResponse;
    use crate::repositories::RepositoryError;
    use lotus_model::SearchCriteria;

    #[derive(Debug, Clone, Default)]
    struct StubRepo {
        replies: Vec<Result<Vec<u8>, RepositoryError>>,
        queries: std::cell::RefCell<Vec<String>>,
    }

    impl StubRepo {
        fn new(replies: Vec<Result<Vec<u8>, RepositoryError>>) -> Self {
            Self {
                replies,
                queries: std::cell::RefCell::new(Vec::new()),
            }
        }

        fn taken(&self) -> usize {
            self.queries.borrow().len()
        }

        fn query(&self, index: usize) -> String {
            self.queries
                .borrow()
                .get(index)
                .cloned()
                .unwrap_or_default()
        }
    }

    impl LotusRepository for StubRepo {
        async fn api_search(
            &self,
            _: &SearchCriteria,
            _: usize,
            _: bool,
        ) -> Option<Result<SearchResponse, RepositoryError>> {
            None
        }

        async fn sparql_body(
            &self,
            query: &str,
        ) -> Result<lotus_search::ResponseBody, RepositoryError> {
            let mut queries = self.queries.borrow_mut();
            queries.push(query.to_owned());
            let index = queries.len().saturating_sub(1);
            let exhausted = Err(RepositoryError::network("stub exhausted"));
            let reply = self
                .replies
                .get(index)
                .or_else(|| self.replies.last())
                .unwrap_or(&exhausted);
            match reply {
                Ok(bytes) => Ok(bytes.clone().into()),
                Err(e) => Err(e.clone()),
            }
        }
    }

    /// One row whose only column is the item URI the lookup projects.
    ///
    /// A `Result` because transport failure is one of the cases under test, and
    /// the stub's replies are one type whatever they hold.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "the stub replies are uniformly `Result`, including the failures under test"
    )]
    fn csv(uri: &str) -> Result<Vec<u8>, RepositoryError> {
        Ok(format!("ref\n{uri}\n").into_bytes())
    }

    fn resolve_on(input: &str, repo: &StubRepo) -> ReferenceResolution {
        futures::executor::block_on(resolve(input, repo, &mut SearchMetrics::default()))
            .expect("resolves")
    }

    #[test]
    fn an_empty_field_is_no_constraint_and_costs_nothing() {
        let repo = StubRepo::new(vec![]);
        for input in ["", "   "] {
            let resolution = resolve_on(input, &repo);
            assert_eq!(resolution.qid, None, "{input:?} means no constraint");
        }
        assert_eq!(
            repo.taken(),
            0,
            "an empty field must not reach the endpoint"
        );
    }

    #[test]
    fn a_qid_is_confirmed_with_one_query_and_nothing_to_match() {
        let repo = StubRepo::new(vec![csv("http://www.wikidata.org/entity/Q23118")]);
        let resolution = resolve_on("Q23118", &repo);
        assert_eq!(resolution.qid.as_deref(), Some("Q23118"));
        assert_eq!(repo.taken(), 1);
        assert!(
            repo.query(0).contains("VALUES ?ref { wd:Q23118 }"),
            "{}",
            repo.query(0)
        );
    }

    #[test]
    fn a_lowercase_qid_is_accepted_the_way_the_taxon_field_accepts_one() {
        let repo = StubRepo::new(vec![csv("http://www.wikidata.org/entity/Q23118")]);
        let resolution = resolve_on("q23118", &repo);
        assert_eq!(resolution.qid.as_deref(), Some("Q23118"));
    }

    #[test]
    fn a_doi_is_asked_for_in_upper_case() {
        // Wikidata stores DOIs uppercased, and a lowercased lookup returns nothing,
        // indistinguishable from a DOI that does not exist.
        let repo = StubRepo::new(vec![csv("http://www.wikidata.org/entity/Q34460861")]);
        let resolution = resolve_on("10.1002/andp.18280880206", &repo);
        assert_eq!(resolution.qid.as_deref(), Some("Q34460861"));
        assert!(
            repo.query(0)
                .contains(r#"wdt:P356 "10.1002/ANDP.18280880206""#),
            "{}",
            repo.query(0)
        );
    }

    #[test]
    fn a_doi_keeps_only_its_resolver_prefix_stripped_and_uppercased() {
        for (typed, expected) in [
            ("10.1002/andp.18280880206", "10.1002/ANDP.18280880206"),
            ("doi:10.1002/andp.18280880206", "10.1002/ANDP.18280880206"),
            (
                "https://doi.org/10.1002/andp.18280880206",
                "10.1002/ANDP.18280880206",
            ),
        ] {
            let repo = StubRepo::new(vec![csv("http://www.wikidata.org/entity/Q34460861")]);
            assert!(resolve_on(typed, &repo).qid.is_some());
            assert!(
                repo.query(0).contains(&format!(r#"wdt:P356 "{expected}""#)),
                "{typed} should be asked for as {expected}: {}",
                repo.query(0)
            );
        }
    }

    #[test]
    fn the_doi_query_keeps_the_shape_the_scholarly_fallback_recognises() {
        // `is_reference_lookup` detects a bare `SELECT ?ref WHERE {` that scans `P356`,
        // and that detection routes a failed lookup to the WDQS scholarly subgraph
        // -- the one WDQS service that answers `P356` quickly. An English label
        // would quietly cost that fast route, so the lookup does not fetch one.
        let doi = lotus_query::reference_by_doi_query("10.1002/andp.18280880206");
        assert!(lotus_query::is_reference_lookup(&doi), "{doi}");
    }

    #[test]
    fn the_qid_query_is_not_mistaken_for_a_doi_scan() {
        // Must *not* match: no `P356` scan here, just a `VALUES`, so the main endpoint
        // answers it and the scholarly subgraph would buy nothing.
        let qid = lotus_query::reference_by_qid_query("Q23118");
        assert!(!lotus_query::is_reference_lookup(&qid), "{qid}");
    }

    #[test]
    fn something_that_is_neither_a_qid_nor_a_doi_is_refused() {
        // No third route: the reader is told the accepted forms rather than having
        // the constraint dropped in silence.
        let repo = StubRepo::new(vec![]);
        for input in [
            "Gentiana lutea",
            "Anti-inflammatory activity of gentianine",
            "10.10/x",
            "11.1000/xyz",
        ] {
            let outcome =
                futures::executor::block_on(resolve(input, &repo, &mut SearchMetrics::default()));
            assert!(
                matches!(
                    outcome,
                    Err(DomainError::Validation(
                        ValidationFault::ReferenceNotAnIdentifier { .. }
                    ))
                ),
                "{input} should be refused, got {outcome:?}"
            );
        }
        assert_eq!(
            repo.taken(),
            0,
            "an input that is not an identifier should not reach the endpoint"
        );
    }

    #[test]
    fn an_identifier_nobody_has_is_refused() {
        let repo = StubRepo::new(vec![csv("")]);
        let outcome = futures::executor::block_on(resolve(
            "10.9999/nothing-here",
            &repo,
            &mut SearchMetrics::default(),
        ));
        assert!(
            matches!(
                outcome,
                Err(DomainError::Validation(
                    ValidationFault::ReferenceNotFound { .. }
                ))
            ),
            "{outcome:?}"
        );
    }

    #[test]
    fn a_transport_failure_is_reported_rather_than_swallowed() {
        let repo = StubRepo::new(vec![Err(RepositoryError::network("timeout"))]);
        let outcome =
            futures::executor::block_on(resolve("Q23118", &repo, &mut SearchMetrics::default()));
        assert!(
            matches!(outcome, Err(DomainError::Transport { .. })),
            "expected Transport, got {outcome:?}"
        );
    }
}
