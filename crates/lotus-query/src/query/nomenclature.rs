// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Taxon nomenclature toggles and SPARQL ancestry closure patterns.

use lotus_model::SearchCriteria;
use lotus_model::taxon_nomenclature::{self, Relation};

use super::filters::{sanitize_qid, sanitize_qid_or_sentinel};

/// Which of the four nomenclatural relationships a taxon search follows.
///
/// Four independent booleans rather than one, because the four relationships
/// answer different questions — see [`lotus_nomenclature`] for why "old versus
/// new" and "accepted versus synonym" are two separate axes and not one.
///
/// [`lotus_nomenclature`]: lotus_model::taxon_nomenclature
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nomenclature {
    /// The four toggles, owned by the model.
    ///
    /// A wrapper rather than a parallel set of fields: there is one place in
    /// this workspace that knows which boolean is which relationship, and it is
    /// not here. Everything below delegates, so a fifth relationship is added
    /// to [`taxon_nomenclature::ALL`] and this wrapper picks it up.
    names: lotus_model::TaxonNomenclature,
}

impl Nomenclature {
    /// Every relationship followed. The default, and what a search gets when
    /// the user has not said otherwise.
    pub const ALL_ON: Self = Self {
        names: lotus_model::TaxonNomenclature::ALL_ON,
    };

    /// No relationship followed: the taxon name as typed, and nothing else.
    pub const ALL_OFF: Self = Self {
        names: lotus_model::TaxonNomenclature::ALL_OFF,
    };

    /// Only the accepted/synonym relationship, for a caller that wants
    /// taxonomic synonymy without the nomenclatural history.
    #[must_use]
    pub const fn accepted_synonyms_only() -> Self {
        Self::only(&taxon_nomenclature::ACCEPTED_SYNONYM)
    }

    /// Only the basionym relationship.
    #[must_use]
    pub const fn basionyms_only() -> Self {
        Self::only(&taxon_nomenclature::BASIONYM)
    }

    /// Only the original-combination (protonym) relationship.
    #[must_use]
    pub const fn protonyms_only() -> Self {
        Self::only(&taxon_nomenclature::PROTONYM)
    }

    /// Only the replacement-name relationship.
    #[must_use]
    pub const fn replacements_only() -> Self {
        Self::only(&taxon_nomenclature::REPLACEMENT)
    }

    const fn only(relation: &Relation) -> Self {
        let mut names = lotus_model::TaxonNomenclature::ALL_OFF;
        names.set_for_relation(relation, true);
        Self { names }
    }

    /// The same choice with one relationship switched off.
    ///
    /// The relationships are independent, so this is a normal operation rather
    /// than a special case: a user who wants accepted-name synonyms but not the
    /// nomenclatural history is asking a coherent question.
    #[must_use]
    pub const fn without(mut self, relation: &Relation) -> Self {
        self.names.set_for_relation(relation, false);
        self
    }

    /// The same choice with one relationship switched on.
    #[must_use]
    pub const fn with(mut self, relation: &Relation) -> Self {
        self.names.set_for_relation(relation, true);
        self
    }

    /// Accepted name ↔ its synonyms (`P1420` / `P12763`). Not chronological.
    /// New combination ↔ its basionym (`P566` / `P12766`).
    /// Current name ↔ its original combination, or protonym
    /// (`P1403` / `P12765`).
    /// Replacement name ↔ what it replaced (`P694` / `P12764`).
    /// Whether the given relationship is followed.
    #[must_use]
    pub const fn follows(&self, relation: &Relation) -> bool {
        self.names.for_relation(relation)
    }

    /// The enabled relationships, in the order
    /// [`taxon_nomenclature::ALL`] lists them.
    #[must_use]
    pub fn enabled_relations(&self) -> Vec<Relation> {
        taxon_nomenclature::ALL
            .into_iter()
            .filter(|relation| self.follows(relation))
            .collect()
    }

    /// Every property of every enabled relationship, deduplicated.
    ///
    /// Each relationship's two properties are distinct, so eight in the all-on
    /// case; the dedupe is here so that a relationship added with a property
    /// another one already uses cannot write it into the path twice.
    #[must_use]
    pub fn properties(&self) -> Vec<(&'static str, &'static str)> {
        let mut seen: Vec<(&'static str, &'static str)> = Vec::new();
        for relation in self.enabled_relations() {
            for pair in relation.properties() {
                if !seen.contains(&pair) {
                    seen.push(pair);
                }
            }
        }
        seen
    }
}

impl Default for Nomenclature {
    fn default() -> Self {
        Self::ALL_ON
    }
}

impl From<&SearchCriteria> for Nomenclature {
    fn from(criteria: &SearchCriteria) -> Self {
        Self::from(&criteria.taxon_names)
    }
}

impl From<&lotus_model::TaxonNomenclature> for Nomenclature {
    fn from(names: &lotus_model::TaxonNomenclature) -> Self {
        Self { names: *names }
    }
}

/// The nomenclatural closure of a taxon, as a property path over the
/// properties of the enabled relationships in [`lotus_model::taxon_nomenclature`].
///
/// Read in **both** directions and unioned with the seed itself in
/// [`taxon_ancestry_pattern_with_roots`].
///
/// The bidirectional read is load-bearing: a curator enters whichever end they are looking
/// at, and both are in active use -- `Rosmarinus officinalis` carries `P12764` toward
/// `Salvia rosmarinus`, while `Salvia rosmarinus` carries `P694` back. A one-directional
/// read finds the newer name from the older one and not the reverse, in all four relations.
///
/// Evaluated as a 1-hop `UNION` from the constant `wd:Q…` rather than a Kleene `*` path,
/// because `QLever` evaluates `VALUES ?seed { wd:Q… } ?seed (path)* ?root` by scanning every
/// triple of all eight predicates across the graph (~4.0s) before filtering to the seed,
/// whereas `{ VALUES ?root { wd:Q… } } UNION { wd:Q… (path) ?root . }` pushes `wd:Q…` into
/// all sixteen index lookups (~150 ms).
///
/// # Why two sibling subqueries and not one big path
///
/// Interleaving as `?t (wdt:P171*|SYN*) wd:Q` or chaining `?desc (wdt:P171*) ?root . ?desc (path)? ?t .`
/// in a single group fails at scale:
///
/// 1. Interleaving `(P171*|SYN*)` anchored at `wd:Q` re-walks the nomenclatural closure at
///    every node of the tree and misses compounds (`11,009` vs `11,088` on `Gentianales`).
/// 2. Chaining `?desc (path)? ?t` after `?desc (wdt:P171*) ?root` forces `QLever` to evaluate
///    the 16-predicate path across up to 1.4M descendants (`Plantae`), timing out at 30s.
///
/// Instead, [`taxon_ancestry_pattern_with_roots`] isolates two sibling subqueries joined on
/// `?desc`: one descends `?desc (wdt:P171*) ?root` from the expanded seed, and the other
/// expands `?t (path)? ?desc` starting strictly from the ~38.6k distinct taxa `?t` that ever
/// appear in `?_s ps:P703 ?t` across all of Wikidata (`56,806` pairs total, `O(1)` independent
/// of clade size and cached across queries by `QLever`).
fn nomenclature_path(nomenclature: Nomenclature) -> Option<String> {
    let props = nomenclature.properties();
    if props.is_empty() {
        return None;
    }
    let mut alternatives: Vec<String> = Vec::with_capacity(props.len() * 2);
    for &(property, _) in &props {
        alternatives.push(property.to_string());
    }
    // The inverses are appended afterwards, each with its own `^`.
    //
    // Writing `^a|b` instead of `^a|^b` would be a silent bug: `^` binds to the single path
    // that follows it, so that reads as "inverse of a, or b" and later properties are read
    // forwards only. The expansion still returns rows, so the query looks fine and returns a
    // different set -- the worst shape a query bug can take.
    for &(property, _) in &props {
        alternatives.push(format!("^{property}"));
    }
    Some(format!("({})", alternatives.join("|")))
}

/// The taxon filter for the innermost subquery: a descendant of any taxon in the
/// nomenclatural closure of `qid`.
///
/// With every relationship off this is the unchanged `P171*` path, so a search that does not
/// want them costs exactly what it used to.
pub(super) fn taxon_ancestry_pattern(qid: &str, nomenclature: Nomenclature) -> String {
    taxon_ancestry_pattern_with_roots(qid, nomenclature, &[])
}

/// The taxon filter, optionally from a closure resolved before the query was built.
///
/// `roots` is the seed's nomenclatural closure -- the taxa the seed can be called,
/// which [`nomenclature_path`] would otherwise compute in a subquery. Passing them trades
/// that subquery for a `VALUES` list, about half the cost on the widest search this app can
/// produce (`docs/SPARQL-VARIANTS.md`). The faster *interleaved* alternative is not used
/// because it answers a different question, losing ~79 of 11,087 distinct compounds.
///
/// **Empty means "not resolved", and that is load-bearing.** An empty `VALUES` matches
/// nothing, so a caller who resolved the closure and got nothing back would silently get an
/// empty result instead of the full subtree. Falling back to the subquery makes the failure
/// mode "slower" rather than "wrong".
pub(super) fn taxon_ancestry_pattern_with_roots(
    qid: &str,
    nomenclature: Nomenclature,
    roots: &[String],
) -> String {
    let sanitized_qid = sanitize_qid_or_sentinel(qid);
    let Some(path) = nomenclature_path(nomenclature) else {
        return format!("?t (wdt:P171*) wd:{sanitized_qid} .");
    };

    // Resolved, and resolved to something.
    let root_expansion = if roots.is_empty() {
        format!(
            "{{\n                SELECT DISTINCT ?root\n                WHERE {{\n                  {{ VALUES ?root {{ wd:{sanitized_qid} }} }}\n                  UNION\n                  {{ wd:{sanitized_qid} {path} ?root . }}\n                }}\n              }}"
        )
    } else {
        let mut values: Vec<String> = roots
            .iter()
            .filter_map(|root| sanitize_qid(root))
            .map(|root| format!("wd:{root}"))
            .collect();
        if values.is_empty() {
            values.push("wd:Q0".to_string());
        }
        format!("VALUES ?root {{ {} }}", values.join(" "))
    };

    format!(
        "{{\n            SELECT ?desc\n            WHERE {{\n              {root_expansion}\n              ?desc (wdt:P171*) ?root .\n            }}\n          }}\n          {{\n            SELECT ?desc ?t\n            WHERE {{\n              {{\n                SELECT DISTINCT ?t\n                WHERE {{\n                  ?_s ps:P703 ?t .\n                }}\n              }}\n              ?t {path}? ?desc .\n            }}\n          }}"
    )
}
