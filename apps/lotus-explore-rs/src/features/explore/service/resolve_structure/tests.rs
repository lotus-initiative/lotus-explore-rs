// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `resolve_structure`, in their own file.

#![allow(clippy::expect_used, clippy::unwrap_used)]
// `LotusRepository` is an async-fn-in-trait, so the stub's methods must be
// `async fn`; a canned response has nothing to await, and the lint cannot
// tell the trait signature apart from a choice in the body.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "LotusRepository is an AFIT: the `async fn` spelling is fixed by the trait"
)]

use super::*;
use crate::api::SearchResponse;
use crate::repositories::RepositoryError;
use lotus_search::SearchCriteria;

#[derive(Clone)]
struct StubRepo {
    /// One response per query, consumed in order; the last one repeats.
    responses: Vec<Result<Vec<u8>, RepositoryError>>,
    queries: std::cell::RefCell<Vec<String>>,
}

impl StubRepo {
    fn new(responses: Vec<Result<Vec<u8>, RepositoryError>>) -> Self {
        Self {
            responses,
            queries: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn taken(&self) -> usize {
        self.queries.borrow().len()
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
        let response = self
            .responses
            .get(index)
            .or_else(|| self.responses.last())
            .unwrap_or(&exhausted);
        match response {
            Ok(bytes) => Ok(bytes.clone().into()),
            Err(e) => Err(e.clone()),
        }
    }
}

fn csv(rows: &str) -> Vec<u8> {
    format!("compound_qid,compound_label,canonical_smiles\n{rows}").into_bytes()
}

fn queries(repo: &StubRepo) -> Vec<String> {
    repo.queries.borrow().clone()
}

fn resolve_on(input: &str, repo: &StubRepo) -> StructureResolution {
    futures::executor::block_on(resolve(input, repo, &mut SearchMetrics::default()))
        .expect("resolution")
}

// ── Which lookup ─────────────────────────────────────────────────────────

#[test]
fn a_structure_resolves_through_the_structure_service() {
    // A structure is not a QID, so the exact route needs one, and only the
    // service can supply it.
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("Q18216,aspirin,CC(=O)OC1=CC=CC=C1C(=O)O"))]);
    let resolution = resolve_on("C[C@H](O)CO", &repo);
    assert_eq!(resolution.resolved.compound.as_deref(), Some("Q18216"));
    let sent = queries(&repo);
    assert!(
        sent.first()
            .is_some_and(|q| q.contains("sachem:similarCompoundSearch")
                && q.contains(r#"sachem:cutoff "1"^^xsd:double"#)),
        "{sent:?}"
    );
}

#[test]
fn a_structure_wikidata_does_not_have_is_not_an_error() {
    // The one asymmetry with the other three routes: nothing in a SMILES can
    // turn out wrong, and refusing it would break every structure search
    // there ever was.
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv(""))]);
    let resolution = resolve_on("C[C@H](O)CO", &repo);
    assert_eq!(resolution.resolved.compound, None);
    assert_eq!(resolution.resolved.structure, "C[C@H](O)CO");
    assert_eq!(resolution.notices, []);
}

#[test]
fn a_repeat_structure_search_does_not_ask_the_service_again() {
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("Q18216,aspirin,CC(=O)O"))]);
    let _ = resolve_on("C[C@H](O)CO", &repo);
    let _ = resolve_on("C[C@H](O)CO", &repo);
    assert_eq!(repo.taken(), 1);
}

#[test]
fn a_qid_is_confirmed_by_one_query_and_names_its_compound() {
    // Still a lookup, because a QID that names nothing has to say so
    // rather than return an empty table.
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("Q18216,aspirin,CC(=O)O"))]);
    let resolution = resolve_on("Q18216", &repo);
    assert_eq!(resolution.resolved.compound.as_deref(), Some("Q18216"));
    assert_eq!(repo.taken(), 1, "one query, and no name lookup behind it");
    assert!(
        queries(&repo)
            .first()
            .is_some_and(|q| q.contains("VALUES ?compound { wd:Q18216 }")),
        "{:?}",
        queries(&repo)
    );
}

#[test]
fn a_qid_that_names_nothing_is_refused() {
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv(""))]);
    let outcome =
        futures::executor::block_on(resolve("Q999999999", &repo, &mut SearchMetrics::default()));
    assert!(
        matches!(
            outcome,
            Err(DomainError::Validation(
                ValidationFault::CompoundNotFound { .. }
            ))
        ),
        "{outcome:?}"
    );
}

#[test]
fn a_qid_is_uppercased_the_way_a_taxon_qid_is() {
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("Q18216,aspirin,CC(=O)O"))]);
    assert_eq!(
        resolve_on("q18216", &repo).resolved.compound.as_deref(),
        Some("Q18216")
    );
}

#[test]
fn an_inchikey_resolves_in_one_query_and_names_its_compound() {
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("Q18216,aspirin,CC(=O)O"))]);
    let resolution = resolve_on("BSYNRYMUTXBXSQ-UHFFFAOYSA-N", &repo);
    assert_eq!(resolution.resolved.compound.as_deref(), Some("Q18216"));
    assert_eq!(repo.taken(), 1, "one query, and no name lookup behind it");
    assert!(
        queries(&repo)
            .first()
            .is_some_and(|q| q.contains("wdt:P235"))
    );
}

#[test]
fn a_name_missing_a_label_falls_through_to_the_alias_lookup() {
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("")), Ok(csv("Q18216,aspirin,CC(=O)O"))]);
    let resolution = resolve_on("acetylsalicylic acid", &repo);
    assert_eq!(resolution.resolved.compound.as_deref(), Some("Q18216"));
    assert_eq!(repo.taken(), 2);
    assert!(
        queries(&repo)
            .get(1)
            .is_some_and(|q| q.contains("skos:altLabel ?name"))
    );
}

// ── What is worth looking up ────────────────────────────────────────────

// ── A miss ──────────────────────────────────────────────────────────────

#[test]
fn a_name_that_resolves_to_nothing_is_an_error() {
    // The taxon field's rule: a name claims which compound is meant, and there
    // is nothing to guess at if it matched nothing. A structure cannot
    // reach here, so no existing search can start failing.
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("")), Ok(csv(""))]);
    let outcome = futures::executor::block_on(resolve(
        "nonexistent compound",
        &repo,
        &mut SearchMetrics::default(),
    ));
    assert!(
        matches!(
            outcome,
            Err(DomainError::Validation(
                ValidationFault::CompoundNotFound { .. }
            ))
        ),
        "{outcome:?}"
    );
}

#[test]
fn a_repeat_miss_still_refuses_rather_than_becoming_a_structure() {
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("")), Ok(csv(""))]);
    let first = futures::executor::block_on(resolve(
        "nonexistent compound",
        &repo,
        &mut SearchMetrics::default(),
    ));
    let second = futures::executor::block_on(resolve(
        "nonexistent compound",
        &repo,
        &mut SearchMetrics::default(),
    ));
    assert!(matches!(
        first,
        Err(DomainError::Validation(
            ValidationFault::CompoundNotFound { .. }
        ))
    ));
    assert!(
        matches!(
            second,
            Err(DomainError::Validation(
                ValidationFault::CompoundNotFound { .. }
            ))
        ),
        "the remembered miss must refuse too, not hand the name to the structure \
         service: {second:?}"
    );
    assert_eq!(
        repo.taken(),
        2,
        "only the first search may reach the endpoint; the miss is remembered"
    );
}

// ── What is handed to the service ───────────────────────────────────────

#[test]
fn a_resolved_compound_hands_its_own_structure_to_the_service() {
    // The whole reason the lookups fetch `P233`: a substructure or similarity
    // search needs a structure, and the text that named the compound is not
    // one.
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("Q18216,aspirin,CC(=O)OC1=CC=CC=C1C(=O)O"))]);
    let resolution = resolve_on("Aspirin", &repo);
    assert_eq!(resolution.resolved.compound.as_deref(), Some("Q18216"));
    assert_eq!(
        resolution.resolved.structure, "CC(=O)OC1=CC=CC=C1C(=O)O",
        "the compound's canonical SMILES, not the word that named it"
    );
}

#[test]
fn a_compound_with_no_canonical_smiles_falls_back_to_what_was_typed() {
    // Nothing better is available, and the service rejects a name as a structure
    // with a message about the structure — the right place for that to
    // surface, rather than an error raised here.
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("Q18216,aspirin,"))]);
    let resolution = resolve_on("Aspirin", &repo);
    assert_eq!(resolution.resolved.compound.as_deref(), Some("Q18216"));
    assert_eq!(resolution.resolved.structure, "Aspirin");
}

// ── Notices ─────────────────────────────────────────────────────────────

#[test]
fn a_name_that_resolves_says_what_it_resolved_to() {
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("Q18216,aspirin,CC(=O)O"))]);
    let resolution = resolve_on("Aspirin", &repo);
    assert!(matches!(
        resolution.notices.as_slice(),
        [LookupNotice::CompoundResolved { .. }]
    ));
}

#[test]
fn several_compounds_are_reported_rather_than_silently_picked() {
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("Q1,Aspirin,A\nQ2,Acetylsalicylic acid,B"))]);
    let resolution = resolve_on("Aspirin", &repo);
    assert_eq!(resolution.resolved.compound.as_deref(), Some("Q1"));
    assert_eq!(resolution.notices.len(), 2, "resolved, and ambiguous");
    assert!(
        matches!(
            resolution.notices.last(),
            Some(LookupNotice::AmbiguousCompound { .. })
        ),
        "ambiguity comes second, after what it resolved to"
    );
}

#[test]
fn a_repeat_search_says_the_same_thing_without_asking_again() {
    structure_cache::clear();
    let repo = StubRepo::new(vec![Ok(csv("Q18216,aspirin,CC(=O)O"))]);
    let first = resolve_on("Aspirin", &repo);
    let second = resolve_on("Aspirin", &repo);
    assert_eq!(first, second);
    assert_eq!(
        repo.taken(),
        1,
        "the second search must not hit the network"
    );
}

#[test]
fn a_transport_failure_is_reported_rather_than_swallowed() {
    structure_cache::clear();
    let repo = StubRepo::new(vec![Err(RepositoryError::network("timeout"))]);
    let outcome =
        futures::executor::block_on(resolve("Aspirin", &repo, &mut SearchMetrics::default()));
    assert!(
        matches!(outcome, Err(DomainError::Transport { .. })),
        "expected Transport, got {outcome:?}"
    );
}

#[test]
fn a_service_failure_on_the_structure_route_is_not_read_as_not_in_wikidata() {
    // The failure mode worth naming: an unreachable service answering "no" to "is
    // this a compound you have?" would send every structure search down the
    // fallback silently, looking like a database that never heard of the
    // compound.
    structure_cache::clear();
    let repo = StubRepo::new(vec![Err(RepositoryError::network("timeout"))]);
    let outcome =
        futures::executor::block_on(resolve("C[C@H](O)CO", &repo, &mut SearchMetrics::default()));
    assert!(
        matches!(outcome, Err(DomainError::Transport { .. })),
        "expected Transport, got {outcome:?}"
    );
}

#[test]
fn a_failure_is_not_remembered_as_a_miss() {
    // Only a real miss goes in the cache. Caching a transport failure would
    // turn one dropped connection into a permanent "compound not found" for
    // the rest of the session.
    structure_cache::clear();
    let repo = StubRepo::new(vec![
        Err(RepositoryError::network("timeout")),
        Ok(csv("Q18216,aspirin,CC(=O)O")),
    ]);
    let failed =
        futures::executor::block_on(resolve("Aspirin", &repo, &mut SearchMetrics::default()));
    assert!(failed.is_err());
    let retried = resolve_on("Aspirin", &repo);
    assert_eq!(
        retried.resolved.compound.as_deref(),
        Some("Q18216"),
        "the retry has to reach the endpoint"
    );
}
