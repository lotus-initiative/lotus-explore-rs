// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]
//! Structure resolution — maps whatever is in the structure field to the
//! compound it names and the structure the search service will be given.
//!
//! The counterpart to [`super::resolve_taxon`]: the field used to require
//! something only the endpoint could interpret, and a reader typing
//! `amarogentina` deserves the same answer as one typing a taxon name.
//!
//! ## Everything is resolved, then the mode decides
//!
//! Resolution runs first, for **every** input kind, because the exact mode needs a
//! QID and a structure is not one.
//!
//! | Typed | Resolved by | Round trips |
//! | --- | --- | --- |
//! | a QID, `Q23118` | [`compound_by_qid_query`] | 1 — a `VALUES`, nothing to match |
//! | an `InChIKey` | [`compound_inchikey_query`] (`P235`) | 1 |
//! | a name | [`compound_label_query`], then [`compound_alias_query`] | 1 or 2 |
//! | a SMILES or molfile | [`structure_compound_lookup_query`] | 1 — the service, at a cutoff of 1 |
//!
//! This module hands back a [`ResolvedStructure`] and stops; [`super::build_query`]
//! picks the route from the mode. A compound the reader named is searched by
//! identity — no service, no index load — and the two broader modes go to the
//! service with that compound's own canonical SMILES.
//!
//! ## The guard
//!
//! A SMILES must never be silently replaced by a same-spelled Wikidata item, so
//! [`lotus_model::could_be_a_compound_name`] decides what may be sent to the name
//! lookup at all. It is the reason `CC` and `CCC` stay structures, and where the
//! cost of that decision is written down: `ATP`, `GDP` and `NAD` are real compound
//! names it refuses.
//!
//! ## Why a name that misses is an error, and a structure is not
//!
//! A name, an `InChIKey` or a QID is a claim about *which* compound is meant, and
//! there is nothing to guess at when it matches nothing. Searching for a structure
//! literally spelled `aspirin` would return a confusingly empty result instead of
//! saying the name is unknown. A structure is not a claim at all, so a structure
//! Wikidata has no compound for is not a miss: it is a good structure simply not in
//! the database, and searching it is what the reader asked for. This is the one
//! asymmetry with the other three routes, and it keeps every structure search that
//! ever working still working.
//!
//! [`compound_by_qid_query`]: lotus_query::compound_by_qid_query
//! [`compound_inchikey_query`]: lotus_query::compound_inchikey_query
//! [`compound_label_query`]: lotus_query::compound_label_query
//! [`compound_alias_query`]: lotus_query::compound_alias_query
//! [`structure_compound_lookup_query`]: lotus_query::structure_compound_lookup_query

use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::service::build_query::ResolvedStructure;
use crate::features::explore::structure_cache::{self, CachedCompound};
use crate::features::explore::types::{
    DomainError, LookupNotice, ParseFault, QueryStage, ValidationFault,
};
use crate::perf;
use crate::repositories::LotusRepository;
use lotus_model::{looks_like_a_compound_qid, looks_like_inchikey, names_a_compound};

/// What the structure field resolved to, plus whatever is worth telling the
/// reader about how it got there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureResolution {
    pub resolved: ResolvedStructure,
    pub notices: Vec<LookupNotice>,
}

/// Resolve the structure field.
///
/// Everything is resolved to a Wikidata compound first, including a structure,
/// because the exact route needs a QID and a structure is not one.
///
/// Fails only on input *claiming* to name a compound and matching nothing: a QID,
/// an `InChIKey` or a plausible name. A structure with no Wikidata compound is not
/// a failure, so no search that worked before can stop working.
pub async fn resolve<R: LotusRepository>(
    input: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
) -> Result<StructureResolution, DomainError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(StructureResolution {
            resolved: ResolvedStructure::unresolved(""),
            notices: Vec::new(),
        });
    }

    // Both remembered outcomes short-circuit: a hit from the cache, a miss from it
    // too. Only `Unknown` means "ask the endpoint".
    let claims_a_compound = names_a_compound(trimmed);
    match structure_cache::lookup(trimmed) {
        structure_cache::Cached::Unknown => {}
        cached => return from_cached(input, cached, claims_a_compound),
    }

    let found = lookup(trimmed, claims_a_compound, repo, metrics).await?;
    // Remembered either way, so a repeat search does not repeat the lookup; the
    // miss is the one worth remembering, being the one a reader most likely
    // retypes.
    structure_cache::store(trimmed, found.clone());

    match found {
        Some(cached) => Ok(StructureResolution {
            resolved: to_resolved(input, &cached),
            notices: to_notices(&cached),
        }),
        // A name, an `InChIKey` or a QID matching nothing: a claim about a
        // compound that does not exist. Refused like an unknown taxon name.
        None if claims_a_compound => {
            Err(DomainError::Validation(ValidationFault::CompoundNotFound {
                input: input.to_owned(),
            }))
        }
        // A structure with no Wikidata compound. Not a miss: a good structure
        // simply not in the database. Nothing in a structure is a claim that
        // can turn out wrong.
        None => Ok(StructureResolution {
            resolved: ResolvedStructure::unresolved(input),
            notices: Vec::new(),
        }),
    }
}

/// The compound and the structure to search, given what was matched.
fn to_resolved(input: &str, cached: &CachedCompound) -> ResolvedStructure {
    ResolvedStructure {
        compound: Some(cached.qid.clone()),
        // Every match, not just the one named above. See `ResolvedStructure`.
        compounds: if cached.all_qids.is_empty() {
            vec![cached.qid.clone()]
        } else {
            cached.all_qids.clone()
        },
        // The compound's own structure. Otherwise the input stands, which for a
        // name is the text that named the compound -- the structure service
        // says so itself, which is the right place for it to surface.
        structure: if cached.canonical_smiles.is_empty() {
            input.to_owned()
        } else {
            cached.canonical_smiles.clone()
        },
    }
}

/// Rebuild a resolution from the cache.
///
/// A remembered miss raises the same error a first-time miss does; handing the
/// input back as a structure instead would make the second search of a typo
/// return something while the first refused it -- the "notice changes when you
/// search again" failure the taxon cache was written to avoid.
fn from_cached(
    input: &str,
    entry: structure_cache::Cached,
    claims_a_compound: bool,
) -> Result<StructureResolution, DomainError> {
    match entry {
        structure_cache::Cached::Resolved(cached) => Ok(StructureResolution {
            resolved: to_resolved(input, &cached),
            notices: to_notices(&cached),
        }),
        structure_cache::Cached::Unresolved | structure_cache::Cached::Unknown => {
            if claims_a_compound {
                Err(DomainError::Validation(ValidationFault::CompoundNotFound {
                    input: input.to_owned(),
                }))
            } else {
                Ok(StructureResolution {
                    resolved: ResolvedStructure::unresolved(input),
                    notices: Vec::new(),
                })
            }
        }
    }
}

fn to_notices(cached: &CachedCompound) -> Vec<LookupNotice> {
    cached
        .notices()
        .into_iter()
        .map(|notice| match notice {
            structure_cache::CompoundNotice::Resolved {
                chosen_label,
                chosen_qid,
            } => LookupNotice::CompoundResolved {
                chosen_label,
                chosen_qid,
            },
            structure_cache::CompoundNotice::Ambiguous {
                chosen_name,
                chosen_qid,
                candidates,
            } => LookupNotice::AmbiguousCompound {
                chosen_name,
                chosen_qid,
                candidates,
            },
        })
        .collect()
}

/// Run the lookup this input calls for.
///
/// One route per input kind. An `InChIKey` names one compound by construction and
/// a QID names one outright, so if neither is known a name lookup will not find
/// what the reader meant. A name is the only kind that can be wrong in a way
/// another kind would catch, so it is the only one that falls through — label
/// first, then alias, because a label is the item's preferred name and so the
/// one a reader meant.
///
/// A structure is asked of the structure service instead: "is this a compound
/// you have?" is the one question that identifies it, and at a cutoff of 1.0 the
/// narrowest one with an answer.
async fn lookup<R: LotusRepository>(
    input: &str,
    names_a_compound: bool,
    repo: &R,
    metrics: &mut SearchMetrics,
) -> Result<Option<CachedCompound>, DomainError> {
    let queries = if !names_a_compound {
        vec![lotus_query::structure_compound_lookup_query(input)]
    } else if looks_like_a_compound_qid(input) {
        vec![lotus_query::compound_by_qid_query(input)]
    } else if looks_like_inchikey(input) {
        vec![lotus_query::compound_inchikey_query(input)]
    } else {
        vec![
            lotus_query::compound_label_query(input),
            lotus_query::compound_alias_query(input),
        ]
    };

    for query in queries {
        let matches = run(&query, repo, metrics).await?;
        if !matches.is_empty() {
            return Ok(structure_cache::pick(&matches));
        }
    }
    Ok(None)
}

async fn run<R: LotusRepository>(
    query: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
) -> Result<Vec<lotus_query::CompoundMatch>, DomainError> {
    let timer = perf::start_timer("LOTUS:structure_resolution");
    let csv = repo.sparql_body(query).await.map_err(|error| {
        let _ = perf::end_timer("LOTUS:structure_resolution", timer);
        DomainError::Transport {
            stage: QueryStage::TaxonSearch,
            source: error,
        }
    })?;

    let elapsed = perf::end_timer("LOTUS:structure_resolution", timer);
    metrics.add_network(elapsed);

    lotus_query::parse_compound_lookup_csv(csv.as_ref()).map_err(|e| {
        DomainError::Parse(ParseFault::CompoundCsv {
            details: e.to_string(),
        })
    })
}

#[cfg(test)]
mod tests {
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
        let outcome = futures::executor::block_on(resolve(
            "Q999999999",
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
        let outcome = futures::executor::block_on(resolve(
            "C[C@H](O)CO",
            &repo,
            &mut SearchMetrics::default(),
        ));
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
}
