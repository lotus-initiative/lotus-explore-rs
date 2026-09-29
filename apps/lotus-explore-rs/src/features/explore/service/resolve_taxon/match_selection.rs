// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::types::{DomainError, ParseFault, TaxonWarning};
use lotus_model::TaxonMatch;

fn eq_casefold(a: &str, b: &str) -> bool {
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(b);
    }
    a.chars()
        .flat_map(char::to_lowercase)
        .eq(b.chars().flat_map(char::to_lowercase))
}

pub(super) struct MatchSelection<'a> {
    pub best: &'a TaxonMatch,
    pub warning: Option<TaxonWarning>,
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

    let warning = if multiple_exact || (first_exact.is_none() && matches.len() > 1) {
        Some(TaxonWarning::Ambiguous {
            chosen_name: best.name.clone(),
            chosen_qid: best.qid.clone(),
            candidates: matches
                .iter()
                .take(4)
                .map(|candidate| format!("{} ({})", candidate.name, candidate.qid))
                .collect(),
        })
    } else {
        None
    };

    Ok(MatchSelection { best, warning })
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
        assert!(selection.warning.is_none());
    }

    #[test]
    fn multiple_candidates_without_exact_match_emit_ambiguity_warning() {
        let matches = vec![
            candidate("Rosa rubiginosa", "Q2"),
            candidate("Rosa canina", "Q3"),
        ];

        let selection = pick_best_match("rosa", &matches).expect("selection should succeed");

        assert_eq!(selection.best.qid, "Q2");
        assert!(matches!(
            selection.warning,
            Some(TaxonWarning::Ambiguous { .. })
        ));
    }

    #[test]
    fn duplicate_exact_matches_emit_ambiguity_warning() {
        let matches = vec![candidate("Rosa", "Q1"), candidate("rosa", "Q2")];

        let selection = pick_best_match("rosa", &matches).expect("selection should succeed");

        assert_eq!(selection.best.qid, "Q1");
        assert!(matches!(
            selection.warning,
            Some(TaxonWarning::Ambiguous { .. })
        ));
    }
}
