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
mod tests;
