// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! In-process structure-field → compound-resolution cache.
//!
//! The same contract as [`super::taxon_cache`], and the same reason: the cached
//! value is the whole resolution rather than just the QID, so a repeat search
//! reproduces the notice instead of losing it. That matters more here than it does
//! for taxa, because the input is a *string* and strings are retyped: a miss is
//! cached too, so a name that matched nothing is refused the same way twice
//! rather than being looked up again and then quietly answered.

use std::cell::RefCell;
use std::collections::HashMap;

use lotus_query::CompoundMatch;

/// A structure-field input that turned out to name a Wikidata compound.
///
/// `qid` is what an exact search is built from, and `canonical_smiles` is what
/// the two broader modes hand to the structure service — so both are load-bearing
/// and both have to survive the cache. `label` and `candidates` exist only so the
/// notice can say what was matched and what else it could have been.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedCompound {
    pub qid: String,
    pub label: String,
    /// `P233`, the canonical SMILES. Empty when the compound has none.
    pub canonical_smiles: String,
    /// `"Name (QID)"` strings, empty when the match was unambiguous.
    pub candidates: Vec<String>,
    /// Every distinct QID the structure service matched, first-seen order.
    ///
    /// The search asks about **all** of them rather than the one named by
    /// `qid`. A structure input names a molecule and the service answers with
    /// everything inside the cutoff -- the (R) form, the (S) form and the
    /// achiral one -- so keeping only the first asks an arbitrary question.
    /// Measured on `C[C@H](O)CO`: the first match was the achiral `Q161495`,
    /// which has no occurrence in Wikidata at all, while `Q27093218`, the (R)
    /// form, has six.
    ///
    /// Bounded by the `LIMIT` on the lookup query, so this is a handful of QIDs
    /// rather than a set of any size.
    pub all_qids: Vec<String>,
}

impl CachedCompound {
    /// The notices this resolution earns, in the order they apply.
    #[must_use]
    pub fn notices(&self) -> Vec<CompoundNotice> {
        let mut notices = vec![CompoundNotice::Resolved {
            chosen_label: self.label.clone(),
            chosen_qid: self.qid.clone(),
        }];
        if !self.candidates.is_empty() {
            notices.push(CompoundNotice::Ambiguous {
                chosen_name: self.label.clone(),
                chosen_qid: self.qid.clone(),
                candidates: self.candidates.clone(),
            });
        }
        notices
    }
}

/// What the structure lookup found, narrowed to what the notice needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompoundNotice {
    /// The input resolved to a compound.
    Resolved {
        chosen_label: String,
        chosen_qid: String,
    },
    /// More than one compound could have been meant.
    Ambiguous {
        chosen_name: String,
        chosen_qid: String,
        candidates: Vec<String>,
    },
}

/// What the cache knows about an input.
///
/// Three states, not two. "Looked up and found nothing" has to be
/// distinguishable from "never looked up", because only the second is worth
/// asking again — and that is the whole reason a miss is remembered rather than
/// simply absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cached {
    /// Looked up, and it named a compound.
    Resolved(Box<CachedCompound>),
    /// Looked up, and it named nothing. Remembered so it is not asked again.
    Unresolved,
    /// Never looked up.
    Unknown,
}

type Cache = HashMap<String, Cached>;

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(HashMap::new());
}

/// The cached resolution for `input`.
pub fn lookup(input: &str) -> Cached {
    let key = input.trim().to_lowercase();
    if key.is_empty() {
        return Cached::Unknown;
    }
    CACHE.with(|cache| cache.borrow().get(&key).cloned().unwrap_or(Cached::Unknown))
}

/// Record a resolution, or the absence of one.
pub fn store(input: &str, resolution: Option<CachedCompound>) {
    let key = input.trim().to_lowercase();
    if key.is_empty() {
        return;
    }
    let entry = resolution.map_or(Cached::Unresolved, |c| Cached::Resolved(Box::new(c)));
    CACHE.with(|cache| {
        cache.borrow_mut().insert(key, entry);
    });
}

/// Forget everything, for tests that assert on a cold cache.
#[cfg(test)]
pub fn clear() {
    CACHE.with(|cache| cache.borrow_mut().clear());
}

/// Narrow a candidate list to the one that was used, with the rest listed.
///
/// Distinct QIDs only. The endpoint can return the same compound twice for one
/// name — once per label it carries that matches — and counting those would tell
/// the reader that several compounds matched when they are looking at one.
///
/// An `InChIKey` is a single compound by construction, so a candidate list longer
/// than one there would mean the endpoint disagreed with itself; it is treated as
/// unambiguous rather than as noise.
#[must_use]
pub fn pick(matches: &[CompoundMatch]) -> Option<CachedCompound> {
    // Distinct QIDs, and for each one the row that is most useful: a compound
    // with several `P233` values arrives as several rows, and a substructure or
    // similarity search has nothing to send to the service if the one kept is the
    // one without a structure. Counting those rows as candidates would report an
    // ambiguity between a compound and itself.
    let mut distinct: Vec<&CompoundMatch> = Vec::with_capacity(matches.len());
    for m in matches {
        match distinct.iter_mut().find(|seen| seen.qid == m.qid) {
            Some(seen) if seen.canonical_smiles.is_empty() && !m.canonical_smiles.is_empty() => {
                *seen = m;
            }
            Some(_) => {}
            None => distinct.push(m),
        }
    }
    let best = distinct.first()?;
    Some(CachedCompound {
        qid: best.qid.clone(),
        label: best.label.clone(),
        canonical_smiles: best.canonical_smiles.clone(),
        candidates: if distinct.len() > 1 {
            distinct.iter().take(4).map(|m| describe(m)).collect()
        } else {
            Vec::new()
        },
        all_qids: distinct.iter().map(|m| m.qid.clone()).collect(),
    })
}

/// `"Label (QID)"`, falling back to the bare QID for an item with no English
/// label -- which is a real case for a compound, and an empty parenthetical
/// would read as a bug.
#[must_use]
pub fn describe(m: &CompoundMatch) -> String {
    if m.label.is_empty() {
        m.qid.clone()
    } else {
        format!("{} ({})", m.label, m.qid)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    fn match_on(qid: &str, label: &str) -> CompoundMatch {
        match_with(qid, label, "")
    }

    fn match_with(qid: &str, label: &str, smiles: &str) -> CompoundMatch {
        CompoundMatch {
            qid: qid.into(),
            label: label.into(),
            canonical_smiles: smiles.into(),
        }
    }

    #[test]
    fn an_unmatched_input_is_remembered_as_unmatched() {
        clear();
        store("cc", None);
        assert_eq!(
            lookup("CC"),
            Cached::Unresolved,
            "a second search must not pay for the same miss"
        );
    }

    #[test]
    fn an_input_never_looked_up_is_distinguishable_from_a_miss() {
        // The distinction the whole cache exists for: only the second is worth
        // asking about again.
        clear();
        store("cc", None);
        assert_eq!(lookup("CCC"), Cached::Unknown);
    }

    #[test]
    fn a_blank_key_is_never_cached() {
        clear();
        store(
            "",
            Some(CachedCompound {
                qid: "Q1".into(),
                label: "x".into(),
                canonical_smiles: "C".into(),
                candidates: Vec::new(),
                all_qids: Vec::new(),
            }),
        );
        assert_eq!(lookup(""), Cached::Unknown);
    }

    #[test]
    fn a_resolution_round_trips_case_insensitively() {
        clear();
        let cached = CachedCompound {
            qid: "Q18216".into(),
            label: "aspirin".into(),
            canonical_smiles: "CC(=O)OC1=CC=CC=C1C(=O)O".into(),
            candidates: Vec::new(),
            all_qids: Vec::new(),
        };
        store("Aspirin", Some(cached.clone()));
        assert_eq!(lookup("aspirin"), Cached::Resolved(Box::new(cached)));
    }

    #[test]
    fn a_resolution_with_a_structure_is_kept_for_the_structure_service() {
        // Substructure and similarity searches hand this to the service, so the
        // compound's own SMILES has to survive the cache.
        clear();
        let cached = pick(&[match_with("Q18216", "aspirin", "CC(=O)O")]).expect("one match");
        assert_eq!(cached.canonical_smiles, "CC(=O)O");
        store("aspirin", Some(cached));
        let Cached::Resolved(remembered) = lookup("Aspirin") else {
            panic!("a resolution must be remembered");
        };
        assert_eq!(remembered.canonical_smiles, "CC(=O)O");
    }

    #[test]
    fn among_several_structures_one_compound_carries_the_one_that_is_searched() {
        // A compound with several P233 values comes back as several rows. The
        // first is not necessarily the one with a structure.
        let cached = pick(&[
            match_with("Q1", "Aspirin", ""),
            match_with("Q1", "Aspirin", "CC(=O)O"),
        ])
        .expect("one compound");
        assert_eq!(cached.canonical_smiles, "CC(=O)O");
        assert!(cached.candidates.is_empty(), "still one compound");
    }

    #[test]
    fn a_compound_with_no_label_is_still_cached() {
        // The parse keeps the row even with nothing to show for it. Dropping the
        // match here would make the miss unobservable and so un-cacheable.
        clear();
        let cached = pick(&[match_on("Q1", "")]).expect("one match");
        store("nameless", Some(cached.clone()));
        assert_eq!(lookup("nameless"), Cached::Resolved(Box::new(cached)));
    }

    #[test]
    fn an_unambiguous_match_earns_only_the_resolved_notice() {
        let cached = pick(&[match_on("Q18216", "aspirin")]).expect("one match");
        assert_eq!(cached.notices().len(), 1);
        assert_eq!(cached.candidates.len(), 0);
    }

    #[test]
    fn several_matches_earn_both_notices_and_list_the_candidates() {
        let cached =
            pick(&[match_on("Q1", "Aspirin"), match_on("Q2", "Aspirin B")]).expect("two matches");
        assert_eq!(cached.qid, "Q1", "the first match is the one used");
        assert_eq!(cached.candidates, vec!["Aspirin (Q1)", "Aspirin B (Q2)"]);
        assert_eq!(cached.notices().len(), 2);
    }

    #[test]
    fn a_label_less_compound_is_described_by_its_qid() {
        let cached = pick(&[match_on("Q7", ""), match_on("Q8", "Named")]).expect("two matches");
        assert_eq!(cached.candidates, vec!["Q7", "Named (Q8)"]);
    }

    #[test]
    fn no_matches_is_no_resolution() {
        assert_eq!(pick(&[]), None);
    }
}
