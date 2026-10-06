// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The two invariants the columnar store's absence handling rests on.
//!
//! Both are a line of code each and both are load-bearing across the whole
//! type: a QID dictionary reserves `NO_VALUE` so "no value" is never
//! confusable with a real item, and a statement column stores the 16 bytes of a
//! UUID rather than the text -- which is where the memory saving came from, and
//! also where a wrong prefix would point the reader at the wrong entity.

// The panic lints keep library code free of panics on external input. A test
// failing on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_model::{Bitmask, ColumnarBuilder, NO_VALUE, QidDictionary, RawRow, write_qid};

#[test]
fn the_sentinel_is_never_interned_and_never_lands_in_a_bitmap() {
    // `NO_VALUE` is `u32::MAX`, which is also what a `try_from` overflow returns.
    // Both mean "no value", and both have to stay distinguishable from a real
    // value -- which only holds if the dictionary refuses to hand the sentinel
    // back as an id and the bitmap refuses to record it.
    let mut dictionary = QidDictionary::new();
    assert_eq!(
        dictionary.intern(NO_VALUE),
        NO_VALUE,
        "interning the sentinel must return the sentinel, not an id"
    );
    assert_eq!(
        dictionary.len(),
        0,
        "the sentinel must not occupy a slot in the dictionary"
    );

    // A real QID still works afterwards, so the refusal skipped the insert
    // rather than poisoning the dictionary.
    let real = dictionary.intern(42);
    assert_ne!(real, NO_VALUE, "a real QID must get a real id");
    assert_eq!(dictionary.len(), 1);
    assert_eq!(dictionary.intern(42), real, "interning is idempotent");

    // A bitmap built over ids that include the sentinel must not report the
    // sentinel as present. If it did, every "has no value" filter would match
    // every row that has no value, which is all of them.
    let mask = dictionary.used([real, NO_VALUE].iter());
    assert!(mask.contains(real), "a real id must be in its own bitmap");
    assert!(
        !mask.contains(NO_VALUE),
        "NO_VALUE must never be found, or an absent value would match every row"
    );
    assert!(
        !Bitmask::with_len(1).contains(NO_VALUE),
        "an empty bitmap must not contain the sentinel either"
    );
}

#[test]
fn the_sentinel_never_escapes_the_dictionary_as_a_numeric_qid() {
    // `Bitmask::insert` has to refuse the sentinel for itself: it has a bit index
    // in it, so a missing guard there would be a wrong answer rather than a wrong
    // count -- every "has no value" filter would match every row with no value.
    let mut mask = Bitmask::with_len(4);
    mask.insert(NO_VALUE);
    mask.insert(1);
    assert!(mask.contains(1), "a real id still lands");
    assert!(
        !mask.contains(NO_VALUE),
        "the guard is per-call, not a property of the bitmap"
    );
    assert_eq!(mask.len(), 1, "the sentinel must not have counted as a bit");

    // The other containment point is `QidDictionary::get`, which is what every
    // accessor on the set goes through. It returns `None` for the sentinel, so a
    // renderer holding an `Option<u32>` cannot be handed the sentinel and
    // cannot print `Q4294967295`.
    let mut dictionary = QidDictionary::new();
    let real = dictionary.intern(42);
    assert_eq!(dictionary.get(real), Some(42), "a real id reads back");
    assert_eq!(
        dictionary.get(NO_VALUE),
        None,
        "the sentinel must read back as absent, not as u32::MAX"
    );

    // `write_qid` itself is a formatter, not a validator: it renders whatever
    // number it is handed, and `Q4294967295` is the honest rendering of
    // `u32::MAX`. The guarantee is that no caller can get that far, not that
    // this function refuses -- so the test above pins the one that refuses.
    let mut out = [0u8; 16];
    assert_eq!(write_qid(42, &mut out), 3, "and a real id still renders");
    assert_eq!(&out[..3], b"Q42", "as `Q42`");
}

/// One row citing `statement`, with everything else empty.
fn statement_row(statement: Option<&str>) -> lotus_model::ColumnarResultSet {
    let mut builder = ColumnarBuilder::new();
    builder.push(RawRow {
        compound_qid: "Q1",
        statement,
        ..RawRow::default()
    });
    builder.build()
}

#[test]
fn a_statement_uuid_is_stored_as_sixteen_bytes_and_rebuilt_to_its_own_text() {
    // The reason the column exists: the text is 52 characters and the stored
    // form is 16. The text has to come back identical, because the endpoint's
    // statement URI is what identifies the claim the row came from.
    let uuid = "0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE";
    let set = statement_row(Some(&format!("Q1-{uuid}")));

    assert_eq!(set.row_count(), 1, "one row in, one row out");
    assert_eq!(
        set.statement_text(0).as_deref(),
        Some(format!("Q1-{uuid}").as_str()),
        "the rebuilt text must be the text that went in"
    );
}

#[test]
fn a_statement_cell_in_any_other_shape_is_kept_verbatim() {
    // Never lossy. A statement that is not the UUID form -- an endpoint that
    // answered with a bare id, a curator's placeholder -- still reaches the
    // reader exactly as it arrived. This is the `Other` branch, and it is what
    // keeps the column honest when the assumption above does not hold.
    for cell in [
        "not-a-uuid",
        "Q1",
        "Q1-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE-extra",
        "0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE",
    ] {
        let set = statement_row(Some(cell));
        assert_eq!(
            set.statement_text(0).as_deref(),
            Some(cell),
            "{cell:?} must survive verbatim rather than be dropped or mangled"
        );
    }
}

#[test]
fn a_row_with_no_statement_is_absent_rather_than_empty_text() {
    // `Absent` and "the empty string" are different answers. The second means
    // the endpoint cited a statement whose id is the empty string, which is not
    // a thing; rendering both the same way puts a bogus URI in the output.
    let set = statement_row(None);
    assert_eq!(
        set.statement_text(0),
        None,
        "a row citing no statement must render nothing, not an empty statement"
    );
    // An empty cell is the same answer as an absent one, and must not become a
    // fallback string: the fallback dictionary exists for text that was there.
    let set = statement_row(Some(""));
    assert_eq!(
        set.statement_text(0),
        None,
        "an empty cell is an absent statement, not an empty string"
    );
}

#[test]
fn a_reference_node_renders_its_hash_and_nothing_keeps_it_absent() {
    // `reference_node_text` had no test in this crate, which is the only place a
    // mutant of it can be killed: cargo-mutants runs one package's own suite, so
    // an assertion in a downstream crate cannot see it. Two mutants survived --
    // the function replaced with `None` and with `Some("")` -- because every
    // fixture in the tree left the column empty.
    //
    // The hash is what identifies the reference. A row that names one renders it;
    // a row that names none renders nothing rather than an empty string, so the
    // export's cell is blank instead of a value that reads as an identifier.
    let hash = "9F1C0E4A-7B2D-4C6E-8A91-3D5F7B2E9C10";
    let mut builder = ColumnarBuilder::new();
    builder.push(RawRow {
        compound_qid: "Q1",
        reference_qid: "Q100",
        reference_node: hash,
        ..RawRow::default()
    });
    builder.push(RawRow {
        compound_qid: "Q1",
        reference_qid: "Q100",
        reference_node: "",
        ..RawRow::default()
    });
    let set = builder.build();

    assert_eq!(
        set.reference_node_text(0).as_deref(),
        Some(hash),
        "a row naming a reference node must render it"
    );
    assert_eq!(
        set.reference_node_text(1),
        None,
        "a row naming none must render nothing, not an empty identifier"
    );

    // And through `entry`, which is the path the export takes: the two rows
    // differ only in that column, so a builder that lost it would be caught.
    let with = set.entry(0).expect("row 0 is present");
    let without = set.entry(1).expect("row 1 is present");
    assert_eq!(with.reference_node.as_ref(), hash);
    assert!(
        without.reference_node.is_empty(),
        "and the rebuilt entry must agree with the accessor"
    );
}
