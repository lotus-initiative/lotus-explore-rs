// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! In-process structure-field → compound-resolution cache.
//!
//! The same contract as [`super::taxon_cache`]: the cached value is the whole
//! resolution, not just the QID, so a repeat search reproduces the notice.
//! Stricter here because the input is a *string* and strings are retyped — a miss
//! is cached too, so a name that matched nothing is refused identically twice
//! rather than looked up again and quietly answered.

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
    /// The search asks about **all** of them, not only the one named by `qid`:
    /// the service answers with everything inside the cutoff — the (R) form, the
    /// (S) form, the achiral one — so keeping the first asks an arbitrary
    /// question. Measured on `C[C@H](O)CO`: the first match was the achiral
    /// `Q161495`, with no occurrence in Wikidata at all, while the (R) form
    /// `Q27093218` has six.
    ///
    /// Bounded by the `LIMIT` on the lookup query, so a handful of QIDs.
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
/// Three states, not two: only "never looked up" is worth asking again, which is
/// why a miss is remembered rather than simply absent.
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
/// Distinct QIDs only: the endpoint can return one compound twice for a name,
/// once per matching label, and counting those would claim several compounds
/// matched. An `InChIKey` is a single compound by construction, so a longer list
/// there means the endpoint disagreed with itself and is treated as unambiguous.
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
#[path = "structure_cache/tests.rs"]
mod tests;
