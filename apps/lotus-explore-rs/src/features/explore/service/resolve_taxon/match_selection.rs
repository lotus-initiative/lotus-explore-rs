// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::taxon_cache::CachedTaxon;
use crate::features::explore::types::{DomainError, ParseFault};
use lotus_model::{TaxonMatch, TaxonNameSource};

/// How many candidates the ambiguity notice lists before it truncates.
const MAX_LISTED_CANDIDATES: usize = 4;

fn eq_casefold(a: &str, b: &str) -> bool {
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(b);
    }
    a.chars()
        .flat_map(char::to_lowercase)
        .eq(b.chars().flat_map(char::to_lowercase))
}

/// The resolution a candidate list yields: a chosen match, plus the candidates
/// that make it worth telling the user about.
///
/// The notices themselves are not built here. They are derived from these
/// fields, so the first run and a cached repeat run cannot describe the same
/// match two different ways.
pub(super) struct MatchSelection<'a> {
    pub best: &'a TaxonMatch,
    /// `"Name (QID)"` strings, empty when the match was unambiguous.
    pub candidates: Vec<String>,
}

impl MatchSelection<'_> {
    /// The cached form of this resolution, ready to store and to replay.
    #[must_use]
    pub(super) fn to_cached(&self) -> CachedTaxon {
        CachedTaxon {
            qid: self.best.qid.clone(),
            label: self.best.name.clone(),
            source: self.best.source,
            candidates: self.candidates.clone(),
        }
    }
}

pub(super) fn pick_best_match<'a>(
    sanitized: &str,
    matches: &'a [TaxonMatch],
) -> Result<MatchSelection<'a>, DomainError> {
    // Two independent questions, so two passes: an exact reading of the input
    // beats a partial one, and at equal exactness a scientific name beats a
    // common name.
    //
    // The second outranks nothing and is outranked by nothing: a common name the
    // reader typed as a common name is better answered by the taxon that *calls
    // itself* that than by one somebody happened to record the word as a
    // vernacular name for. Inverted, `Bacteria` would send every bacteria search
    // to the wrong kingdom: it is the scientific name of Q10876 and a common name
    // of several unrelated things.
    let best = matches
        .iter()
        .filter(|candidate| eq_casefold(&candidate.name, sanitized))
        .min_by_key(|candidate| match candidate.source {
            TaxonNameSource::Scientific => 0,
            TaxonNameSource::Common => 1,
        })
        .or_else(|| matches.first())
        .ok_or_else(|| {
            DomainError::Parse(ParseFault::TaxonPick {
                details: "no candidates after parse".into(),
            })
        })?;

    // Whether anything else could plausibly have been meant. Two exact readings
    // of the input are ambiguous, and so is a partial reading of an input that
    // matched more than one taxon.
    let exact = matches
        .iter()
        .filter(|candidate| eq_casefold(&candidate.name, sanitized))
        .count();
    let ambiguous = exact > 1 || (exact == 0 && matches.len() > 1);

    let candidates = if ambiguous {
        matches
            .iter()
            .take(MAX_LISTED_CANDIDATES)
            .map(|candidate| format!("{} ({})", candidate.name, candidate.qid))
            .collect()
    } else {
        Vec::new()
    };

    Ok(MatchSelection { best, candidates })
}

#[cfg(test)]
#[path = "match_selection/tests.rs"]
mod tests;
