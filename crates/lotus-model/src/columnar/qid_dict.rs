// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Numeric QID interning dictionary and allocation-free `Q…` formatting helpers.

use rustc_hash::FxHashMap;

use super::NO_VALUE;
use super::bitmask::Bitmask;
use super::filter::contains_folded;

/// The largest bare QID text this module renders into a stack buffer.
///
/// A `Q` and up to ten digits is eleven bytes; Wikidata item numbers are around
/// 250 million today and grow slowly. A QID that does not fit renders as nothing
/// rather than as a truncated identifier.
pub(super) const QID_BUFFER: usize = 16;

/// Interned Wikidata QIDs, keyed by their numeric part.
///
/// A QID is a `Q` and a number, so storing the text spends a hash, an allocation
/// and sixteen bytes of fat pointer on an integer the parser already has. At one
/// million rows: 39 MB and 640,000 allocations for four-byte values.
///
/// [`qid_text`] and [`write_qid`] render the same `Q…` the cell held, so links,
/// filters and hashes still see the identifier Wikidata knows the item by, and the
/// text is produced only for the thirty rows on screen.
#[derive(Debug, Default, Clone)]
pub struct QidDictionary {
    /// Numeric QID -> the id naming it.
    index: FxHashMap<u32, u32>,
    /// The numeric QIDs in first-seen order, so an id *is* a position.
    order: Vec<u32>,
}

impl QidDictionary {
    /// An empty dictionary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The id for `numeric`, minting one if it is new.
    ///
    /// [`NO_VALUE`] is never handed back: no Wikidata item is numbered `u32::MAX`,
    /// and reserving it keeps "no value" distinguishable from any real one.
    pub fn intern(&mut self, numeric: u32) -> u32 {
        if numeric == NO_VALUE {
            return NO_VALUE;
        }
        if let Some(id) = self.index.get(&numeric) {
            return *id;
        }
        let Ok(id) = u32::try_from(self.order.len()) else {
            return NO_VALUE;
        };
        if id == NO_VALUE {
            return NO_VALUE;
        }
        self.index.insert(numeric, id);
        self.order.push(numeric);
        id
    }

    /// The numeric QID an id names, or `None` for [`NO_VALUE`].
    ///
    /// `O(1)`: ids are assigned in first-seen order, so one is a position in
    /// `order`.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<u32> {
        if id == NO_VALUE {
            return None;
        }
        self.order.get(id as usize).copied()
    }

    /// The id for `numeric`, if it is already interned.
    #[must_use]
    pub fn lookup(&self, numeric: u32) -> Option<u32> {
        self.index.get(&numeric).copied()
    }

    /// How many distinct QIDs are interned.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.order.len()
    }

    /// Whether nothing has been interned.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Every interned numeric QID, in first-seen order.
    pub fn numeric_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.order.iter().copied()
    }

    /// The ids actually used in `ids`, as a bitmap, for the exact counts.
    #[must_use]
    pub fn used<'a>(&self, ids: impl Iterator<Item = &'a u32>) -> Bitmask {
        let mut mask = Bitmask::with_len(self.order.len());
        for id in ids {
            if *id != NO_VALUE {
                mask.insert(*id);
            }
        }
        mask
    }
}

/// A bare QID as `Q…`, rendered into `out`.
///
/// Returns the length written, or `0` when the number does not fit, which no
/// Wikidata item does.
#[must_use]
pub fn write_qid(numeric: u32, out: &mut [u8; QID_BUFFER]) -> usize {
    let mut digits = [0u8; 10];
    let mut count = 0usize;
    let mut rest = numeric;
    loop {
        let Some(slot) = digits.get_mut(count) else {
            return 0;
        };
        *slot = b'0' + u8::try_from(rest % 10).unwrap_or(0);
        rest /= 10;
        count += 1;
        if rest == 0 {
            break;
        }
    }
    let Some(first) = out.first_mut() else {
        return 0;
    };
    *first = b'Q';
    // `digits` is filled least-significant first, so it is emitted in reverse:
    // forward would render 42 as `Q24`, and a QID that does not round trip is a
    // dead link on every row that has one.
    for (offset, digit) in digits.iter().take(count).rev().enumerate() {
        let Some(slot) = out.get_mut(offset + 1) else {
            return 0;
        };
        *slot = *digit;
    }
    count + 1
}

/// A bare QID as `Q…`, allocated.
///
/// For the paths that hand one outside this module: a link, a displayed cell, a
/// hasher. Called for the rows on screen rather than for every row stored.
#[must_use]
pub fn qid_text(numeric: u32) -> String {
    let mut buffer = [0u8; QID_BUFFER];
    let length = write_qid(numeric, &mut buffer);
    String::from_utf8_lossy(buffer.get(..length).unwrap_or_default()).into_owned()
}

/// Whether a numeric QID's `Q…` text contains `needle`.
///
/// Renders into the caller's buffer, so filtering a two-million-compound dictionary
/// allocates nothing per compound per keystroke; `buffer` is reused for the whole
/// scan.
pub(super) fn qid_matches(numeric: u32, needle: &[char], buffer: &mut [u8; QID_BUFFER]) -> bool {
    let length = write_qid(numeric, buffer);
    std::str::from_utf8(buffer.get(..length).unwrap_or_default())
        .is_ok_and(|text| contains_folded(text, needle))
}

/// Intern an entity URI as a numeric QID, without allocating.
///
/// [`normalize_qid`](crate::normalize_qid) would do this too, but it returns an
/// owned `String` and this is called three times per row -- nine million
/// allocations at three million rows, spent on a substring and a digit scan.
///
/// Anything that is not a QID -- a property, a blank node, an empty cell -- is an
/// absence. A blank node in particular must not become a dictionary entry: every
/// blank node is distinct, so it would cost a slot per row for a value no column
/// can display.
pub(super) fn intern_qid(dictionary: &mut QidDictionary, raw: &str) -> Option<u32> {
    let numeric = parse_qid_number(raw)?;
    let id = dictionary.intern(numeric);
    dictionary.get(id).map(|_| id)
}

pub(super) fn lookup_slot(dictionary: &QidDictionary, raw: &str) -> Option<usize> {
    parse_qid_number(raw)
        .and_then(|n| dictionary.lookup(n))
        .map(|id| id as usize)
}

pub(super) fn parse_qid_number(raw: &str) -> Option<u32> {
    crate::identify::strip_qid_digits(raw).and_then(parse_numeric_qid)
}

/// The number a run of ASCII digits names, or `None` if it is not one.
///
/// [`NO_VALUE`] is refused because it is reserved for "no value".
pub(super) fn parse_numeric_qid(digits: &str) -> Option<u32> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value = digits.parse::<u32>().ok()?;
    (value != NO_VALUE).then_some(value)
}

/// A dictionary slot as an id.
pub(super) fn slot_to_id(slot: usize) -> u32 {
    u32::try_from(slot).unwrap_or(NO_VALUE)
}
