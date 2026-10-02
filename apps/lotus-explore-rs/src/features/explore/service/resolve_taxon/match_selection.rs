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
    // Two passes over the candidates, because the two things being ranked are
    // independent questions:
    //
    // 1. an exact reading of the input beats a partial one, and
    // 2. a scientific name beats a common name at equal exactness.
    //
    // The second outranks nothing and is outranked by nothing: a common name
    // that the reader typed as a common name is still better answered by the
    // taxon that *calls itself* that than by the one somebody happened to record
    // the word as a vernacular name for. Getting this the other way round is the
    // failure this ordering exists to prevent — `Bacteria` is the scientific name
    // of Q10876 and a common name of several unrelated things.
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
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::features::explore::types::LookupNotice;

    fn candidate(name: &str, qid: &str) -> TaxonMatch {
        TaxonMatch {
            qid: qid.into(),
            name: name.into(),
            source: TaxonNameSource::Scientific,
        }
    }

    fn common(name: &str, qid: &str) -> TaxonMatch {
        TaxonMatch {
            qid: qid.into(),
            name: name.into(),
            source: TaxonNameSource::Common,
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

    // ── Scientific against common ────────────────────────────────────────────

    #[test]
    fn a_scientific_name_beats_a_common_name_at_equal_exactness() {
        // The whole point: `Bacteria` is the scientific name of Q10876 and a
        // common name of Q4034791. Ordering these the other way round sends
        // every bacteria search to the wrong kingdom.
        let matches = vec![
            common("Bacteria", "Q4034791"),
            candidate("Bacteria", "Q10876"),
        ];

        let selection = pick_best_match("Bacteria", &matches).expect("selection should succeed");

        assert_eq!(selection.best.qid, "Q10876");
        assert_eq!(selection.best.source, TaxonNameSource::Scientific);
        // Two exact readings, so still ambiguous — and the notice must not claim
        // the search fell back to a common name when it did not.
        assert_eq!(selection.candidates.len(), 2);
        assert_eq!(selection.to_cached().warnings().len(), 1);
    }

    #[test]
    fn a_common_name_is_used_when_no_scientific_name_matches() {
        let matches = vec![common("Gentian", "Q777")];

        let selection = pick_best_match("Gentian", &matches).expect("selection should succeed");

        assert_eq!(selection.best.qid, "Q777");
        assert_eq!(selection.best.source, TaxonNameSource::Common);
        // Unambiguous, but the source still earns its own notice.
        assert_eq!(selection.candidates.len(), 0);
        assert_eq!(selection.to_cached().warnings().len(), 1);
    }

    #[test]
    fn an_exact_scientific_name_beats_a_partial_scientific_one() {
        // Exactness outranks nothing about the source: this is still two
        // scientific names, and the one the reader typed is the answer.
        let matches = vec![
            candidate("Gentiana", "Q1"),
            candidate("Gentiana lutea", "Q2"),
        ];

        let selection = pick_best_match("gentiana", &matches).expect("selection should succeed");

        assert_eq!(selection.best.qid, "Q1");
    }

    #[test]
    fn exactness_outranks_the_source() {
        // Only the common name matches the input exactly; the scientific name is
        // a different organism entirely. Exactness decides first, so the common
        // name wins — and still says so, because it still is one.
        let matches = vec![
            common("Gentiana", "Q777"),
            candidate("Gentiana lutea", "Q2"),
        ];

        let selection = pick_best_match("Gentiana", &matches).expect("selection should succeed");

        assert_eq!(selection.best.qid, "Q777");
        assert_eq!(selection.best.source, TaxonNameSource::Common);
        let warnings = selection.to_cached().warnings();
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings
                .iter()
                .all(|w| matches!(w, LookupNotice::CommonName { .. })),
            "the only notice is the one about the source: {warnings:?}"
        );
    }

    #[test]
    fn a_common_name_that_matches_nothing_exactly_still_wins_over_nothing() {
        // Partial reading: the common name is the best available answer, and the
        // ambiguity notice lists what else could have been meant.
        let matches = vec![common("Gentian", "Q777"), candidate("Gentiana lutea", "Q2")];

        let selection = pick_best_match("Gentia", &matches).expect("selection should succeed");

        assert_eq!(selection.best.qid, "Q777");
        assert_eq!(selection.candidates.len(), 2);
    }
}
