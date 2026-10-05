// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod columnar;
mod criteria;
mod entry;
mod identify;
mod reference;
mod stats;
mod structure;
mod validate;

pub use columnar::{
    Bitmask, ColumnarBuilder, ColumnarResultSet, FilterPlan, FilterSpec, NO_VALUE, QidDictionary,
    Range, RawRow, SparseStrings, StatementId, contains_folded, folded, qid_text, write_qid,
};
pub use criteria::{SearchCriteria, TaxonNomenclature};
pub use entry::{CompoundEntry, Rows, TaxonMatch, TaxonNameSource};
pub use identify::{non_empty, normalize_doi, normalize_qid};
pub use reference::{looks_like_doi, looks_like_reference_qid, strip_doi_prefix};
pub use stats::{DEFAULT_STRUCTURE_THRESHOLD, DatasetStats, ElementState, SmilesSearchType};
pub use structure::{
    InputKind, REFERENCE_INPUT_EXAMPLES, STRUCTURE_INPUT_EXAMPLES, StructureKind,
    classify_structure, classify_structure_input, could_be_a_compound_name,
    looks_like_a_compound_qid, looks_like_inchikey, names_a_compound,
};
pub use validate::{ValidationError, validate_criteria};

/// Base URI for Wikidata entities (`Q123` → `<BASE>Q123`).
pub const WIKIDATA_ENTITY_BASE: &str = "http://www.wikidata.org/entity/";

/// Base URI for Wikidata reification statements (`S1` → `<BASE>statement/S1`).
pub const WIKIDATA_STATEMENT_BASE: &str = "http://www.wikidata.org/entity/statement/";

/// Namespace of a Wikidata *reference node* -- the thing a statement is
/// `prov:wasDerivedFrom`.
///
/// Not a QID and not a statement: `http://www.wikidata.org/reference/<64 hex>`.
/// Anything that tries to read a `Q<digits>` out of one gets a cast error, so this
/// is stripped explicitly instead, the same way [`WIKIDATA_STATEMENT_BASE`] is.
pub const WIKIDATA_REFERENCE_BASE: &str = "http://www.wikidata.org/reference/";

/// Widest atom count each of the six filterable elements is given by default.
///
/// A count above these cannot correspond to a real molecule, so a filter that
/// asks for one is rejected rather than used to widen the result set.
pub mod element_max {
    /// Carbon.
    pub const C: u16 = 512;
    /// Hydrogen.
    pub const H: u16 = 1_024;
    /// Nitrogen.
    pub const N: u16 = 256;
    /// Oxygen.
    pub const O: u16 = 256;
    /// Phosphorus.
    pub const P: u16 = 128;
    /// Sulfur.
    pub const S: u16 = 64;
}

/// The nomenclatural relationships that tie one taxon name to another, and so
/// decide which taxa a name-aware search must also collect.
///
/// Each is stored twice in Wikidata, once from each end, and the four are
/// **independent of each other**: they answer different questions.
///
/// | Relationship | Wikidata pair | Old / new? |
/// | --- | --- | --- |
/// | accepted name ↔ its synonyms | [`P1420`](https://www.wikidata.org/wiki/Property:P1420) *taxon synonym* / [`P12763`](https://www.wikidata.org/wiki/Property:P12763) *taxon synonym of* | no — see below |
/// | new combination ↔ its basionym | [`P566`](https://www.wikidata.org/wiki/Property:P566) *basionym* / [`P12766`](https://www.wikidata.org/wiki/Property:P12766) *basionym of* | yes |
/// | current name ↔ its original combination | [`P1403`](https://www.wikidata.org/wiki/Property:P1403) *original combination* / [`P12765`](https://www.wikidata.org/wiki/Property:P12765) *protonym of* | yes |
/// | replacement name ↔ what it replaced | [`P694`](https://www.wikidata.org/wiki/Property:P694) *replaced synonym (for nom. nov.)* / [`P12764`](https://www.wikidata.org/wiki/Property:P12764) *replaced synonym of* | yes |
///
/// **The last three are chronological**: subject is the *newer* name, object the
/// *older* one. A **basionym** is the name the specimen was first described under;
/// when the genus is reassigned the epithet moves into a **new combination** and
/// the basionym retires. An **original combination** (zoological mirror image: a
/// **protonym**) is the binomial as first published. A **replacement name**
/// (*nomen novum*) exists because the old name is unusable, typically a homonym.
///
/// **The first is not chronological.** `P1420` links an accepted name to another
/// name for the same taxon; which is older is not recorded and not implied. A
/// synonym is often the *older* name, but it can equally be one coined later and
/// found superfluous: `P1420` joins `Leontopodium nivale` and
/// `Leontopodium alpinum`, and *nivale* is the junior name, so "old to new" would
/// be backwards. Hence the first row is *accepted / synonym* and the rest *old /
/// new* — a single "synonyms" toggle over all four could not tell the user which
/// question they had just switched off.
///
/// **Old/new is not the axis accepted/not accepted.** A basionym is usually a
/// synonym now but may be the accepted name again under a different opinion, and a
/// new combination is sometimes itself a rejected synonym. *Accepted vs. synonym*
/// is a taxonomic judgement, *old vs. new* a nomenclatural fact; Wikidata keeps
/// them under four separate property pairs because neither determines the other.
///
/// [`P1531`](https://www.wikidata.org/wiki/Property:P1531) (*hybrid of*) is
/// deliberately **excluded**: it records which parent a hybrid was bred from,
/// provenance rather than identity. Every relationship above asserts two names
/// denote the same taxon, which is what makes a compound filed under one a
/// compound from the other. Searching `Nepeta x catariensis` and receiving every
/// compound reported from *Nepeta cataria* would answer a different question.
pub mod taxon_nomenclature {
    /// One nomenclatural relationship, as the two Wikidata properties that
    /// record it from opposite ends.
    ///
    /// The two properties are named for the end they lead *from*. Literal for the
    /// three chronological relationships (`P566` leads from a new combination to
    /// its basionym); for accepted/synonym there is no older end, so the names are
    /// convenience rather than a claim about chronology.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Relation {
        /// Stable identifier, used in query parameters and in the UI's
        /// per-relation state.
        pub id: &'static str,
        /// Which of the four toggles this relation is. A `usize` rather than a
        /// discriminant so a caller can index a `[bool; 4]` without a `match`
        /// that would have to be updated when a relation is added.
        pub slot: usize,
        /// The property leading from the accepted name to a synonym.
        pub from_accepted: &'static str,
        /// The property leading from a synonym back to the accepted name.
        pub to_accepted: &'static str,
    }

    /// Accepted name ↔ its synonyms. **Not** chronological.
    pub const ACCEPTED_SYNONYM: Relation = Relation {
        id: "accepted",
        slot: 0,
        from_accepted: "wdt:P1420",
        to_accepted: "wdt:P12763",
    };

    /// New combination ↔ its basionym. Chronological: `to_accepted` is the
    /// basionym, and the name the specimen was first described under.
    pub const BASIONYM: Relation = Relation {
        id: "basionym",
        slot: 1,
        from_accepted: "wdt:P566",
        to_accepted: "wdt:P12766",
    };

    /// Current name ↔ its original combination (its zoological mirror, a
    /// protonym). Chronological: `to_accepted` is the binomial as first
    /// published.
    pub const PROTONYM: Relation = Relation {
        id: "protonym",
        slot: 2,
        from_accepted: "wdt:P1403",
        to_accepted: "wdt:P12765",
    };

    /// Replacement name (*nomen novum*) ↔ the name it replaced. Chronological:
    /// `to_accepted` is the name that could no longer be used.
    pub const REPLACEMENT: Relation = Relation {
        id: "replacement",
        slot: 3,
        from_accepted: "wdt:P694",
        to_accepted: "wdt:P12764",
    };

    /// All four, in the order the query builder emits them.
    pub const ALL: [Relation; 4] = [ACCEPTED_SYNONYM, BASIONYM, PROTONYM, REPLACEMENT];

    impl Relation {
        /// Both properties, as the `(property, "wdt:P…")` pairs the tests read.
        #[must_use]
        pub const fn properties(self) -> [(&'static str, &'static str); 2] {
            [
                (self.from_accepted, self.from_accepted),
                (self.to_accepted, self.to_accepted),
            ]
        }
    }
}

/// Upper bound for the mass range, in daltons.
pub const MASS_MAX: f64 = 10_000.0;

/// Earliest plausible publication year. Earlier means a parsing fault, not
/// history: the LOTUS sources are modern.
pub const YEAR_MIN: u16 = 1800;

/// Longest taxon string accepted, in bytes.
pub const TAXON_MAX_LEN: usize = 500;

/// Longest structure string accepted, in bytes.
pub const STRUCTURE_MAX_LEN: usize = 10_000;

#[cfg(test)]
#[path = "taxon_nomenclature_tests.rs"]
mod taxon_nomenclature_tests;
