// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The four nomenclatural relationships, asserted as data.
//!
//! The only place the property-to-relationship mapping is checked, and it has to
//! be here rather than in `lotus-query`: a mutation run against this crate
//! executes this crate's tests only, so a `properties()` returning the wrong
//! predicate — or the same one twice, or an empty string — would survive every
//! query-side assertion and emit a query that traverses nothing.
//!
//! The panic lints keep library code from panicking on bad input; a test
//! failing on a bad value is reporting, not panicking.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::taxon_nomenclature as n;

/// The properties each relationship is built from, written out.
///
/// Written as literals rather than derived from the constants above: the point
/// is to catch a constant that was mistyped, and a test that reads the constant
/// back cannot.
const EXPECTED: [(n::Relation, &str, &str, &str); 4] = [
    (n::ACCEPTED_SYNONYM, "accepted", "wdt:P1420", "wdt:P12763"),
    (n::BASIONYM, "basionym", "wdt:P566", "wdt:P12766"),
    (n::PROTONYM, "protonym", "wdt:P1403", "wdt:P12765"),
    (n::REPLACEMENT, "replacement", "wdt:P694", "wdt:P12764"),
];

/// Every relationship names the two properties that record it, in that order.
#[test]
fn each_relationship_names_its_own_two_properties() {
    for (relation, id, from, to) in EXPECTED {
        assert_eq!(relation.id, id, "{id} has the wrong id");
        assert_eq!(relation.from_accepted, from, "{id} has the wrong forward");
        assert_eq!(relation.to_accepted, to, "{id} has the wrong inverse");
        assert_eq!(
            relation.properties(),
            [(from, from), (to, to)],
            "{id} hands back its two properties"
        );
    }
}

/// The two properties of a relationship are different properties.
///
/// A relation whose pair collapsed to one predicate would still return two
/// entries, and the query builder's dedupe would then emit a path with one
/// alternative in it — a query that looks fine and traverses one direction of
/// one relationship.
#[test]
fn a_relationship_is_a_pair_of_distinct_properties() {
    for relation in n::ALL {
        let [first, second] = relation.properties();
        assert_ne!(
            first.1, second.1,
            "{} pairs one property with itself",
            relation.id
        );
        assert!(!first.1.is_empty(), "{} has an empty property", relation.id);
    }
}

/// The four relationships are four, they are distinct, and each occupies its
/// own slot.
///
/// `slot` is what `TaxonNomenclature`'s dispatch indexes on, so two relations
/// sharing a slot would make one of them unreachable — and a duplicate `id`
/// would make the query parameters and the UI key collide.
#[test]
fn the_four_relationships_are_distinct_and_slot_each_one_out() {
    assert_eq!(n::ALL.len(), 4);

    let mut ids: Vec<&str> = Vec::new();
    let mut slots: Vec<usize> = Vec::new();
    let mut properties: Vec<&str> = Vec::new();
    for relation in n::ALL {
        assert!(!ids.contains(&relation.id), "{}: duplicate id", relation.id);
        assert!(
            !slots.contains(&relation.slot),
            "{}: duplicate slot",
            relation.id
        );
        for (_, prefixed) in relation.properties() {
            assert!(
                !properties.contains(&prefixed),
                "{prefixed} is claimed by two relationships"
            );
            properties.push(prefixed);
        }
        ids.push(relation.id);
        slots.push(relation.slot);
    }

    // Four distinct slots over a four-slot space, so the dispatch in
    // `TaxonNomenclature` can be a lookup rather than a search.
    let mut sorted = slots.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, [0, 1, 2, 3], "the slots must cover 0..4 exactly");
}

/// `ALL` lists the four in declaration order, which is the order the query
/// builder writes them into a property path.
///
/// Order is not cosmetic: it is part of the query's text, and therefore of
/// every cache key and shared link built from it.
#[test]
fn all_lists_the_relationships_in_declaration_order() {
    let declared = [
        n::ACCEPTED_SYNONYM,
        n::BASIONYM,
        n::PROTONYM,
        n::REPLACEMENT,
    ];
    for (index, relation) in n::ALL.iter().enumerate() {
        let expected = declared
            .get(index)
            .unwrap_or_else(|| panic!("position {index} is in range"))
            .id;
        assert_eq!(relation.id, expected, "position {index}");
    }
}
