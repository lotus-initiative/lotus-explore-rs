// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::taxon_cache::CachedTaxon;
use crate::features::explore::types::{DomainError, ParseFault};
use lotus_model::TaxonMatch;

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
/// The notice itself is not built here. It is derived from these two fields, so
/// the first run and a cached repeat run cannot describe the same match two
/// different ways.
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
            candidates: self.candidates.clone(),
        }
    }
}

pub(super) fn pick_best_match<'a>(
    sanitized: &str,
    matches: &'a [TaxonMatch],
) -> Result<MatchSelection<'a>, DomainError> {
    // Scan once: find the first exact match and whether a second exists.
    // Early-exit after the second exact match so we avoid scanning the entire
    // candidate list just to count duplicates.
    let mut first_exact: Option<&TaxonMatch> = None;
    let mut multiple_exact = false;
    for candidate in matches {
        if eq_casefold(&candidate.name, sanitized) {
            if first_exact.is_none() {
                first_exact = Some(candidate);
            } else {
                multiple_exact = true;
                break;
            }
        }
    }

    let best = first_exact.or_else(|| matches.first()).ok_or_else(|| {
        DomainError::Parse(ParseFault::TaxonPick {
            details: "no candidates after parse".into(),
        })
    })?;

    let candidates = if multiple_exact || (first_exact.is_none() && matches.len() > 1) {
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
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn candidate(name: &str, qid: &str) -> TaxonMatch {
        TaxonMatch {
            qid: qid.into(),
            name: name.into(),
        }
    }

    #[test]
    fn exact_match_is_preferred_over_first_non_exact_candidate() {
        let matches = vec![candidate("Rosa rubiginosa", "Q2"), candidate("Rosa", "Q1")];

        let selection = pick_best_match("rosa", &matches).expect("selection should succeed");

        assert_eq!(selection.best.qid, "Q1");
        assert_eq!(selection.candidates.len(), 0);
        assert_eq!(selection.to_cached().warnings().len(), 0);
    }

    #[test]
    fn multiple_candidates_without_exact_match_emit_ambiguity_warning() {
        let matches = vec![
            candidate("Rosa rubiginosa", "Q2"),
            candidate("Rosa canina", "Q3"),
        ];

        let selection = pick_best_match("rosa", &matches).expect("selection should succeed");

        assert_eq!(selection.best.qid, "Q2");
        let cached = selection.to_cached();
        assert_eq!(
            cached.candidates,
            vec!["Rosa rubiginosa (Q2)", "Rosa canina (Q3)"]
        );
        assert_eq!(cached.warnings().len(), 1);
    }

    #[test]
    fn duplicate_exact_matches_emit_ambiguity_warning() {
        let matches = vec![candidate("Rosa", "Q1"), candidate("rosa", "Q2")];

        let selection = pick_best_match("rosa", &matches).expect("selection should succeed");

        assert_eq!(selection.best.qid, "Q1");
        assert_eq!(selection.to_cached().warnings().len(), 1);
    }

    #[test]
    fn the_candidate_list_is_truncated_but_never_empty() {
        let matches = (0..6)
            .map(|i| candidate(&format!("Taxon {i}"), &format!("Q{i}")))
            .collect::<Vec<_>>();

        let selection = pick_best_match("taxon", &matches).expect("selection should succeed");

        let cached = selection.to_cached();
        assert_eq!(cached.candidates.len(), MAX_LISTED_CANDIDATES);
        assert_eq!(cached.warnings().len(), 1);
    }
}
