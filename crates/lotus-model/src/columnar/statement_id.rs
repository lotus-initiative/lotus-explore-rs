// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Compact 16-byte UUID statement representation for Wikibase statement IDs.

use super::qid_dict::parse_numeric_qid;
use super::sparse_strings::SparseStrings;
use crate::WIKIDATA_STATEMENT_BASE;

/// The statement backing one occurrence.
///
/// On Wikidata a statement is `<compound QID>-<UUID>`, and the UUID is random:
/// across a fifty-thousand-row sample the 49,246 distinct statements had 49,246
/// distinct suffixes, so interning the whole string buys nothing. What *is*
/// redundant is the prefix -- it equals the row's compound QID in 99.97% of
/// rows -- so only the 16 bytes of the UUID are kept and the text is rebuilt for
/// the handful of rows the table actually renders.
///
/// A statement in any other shape is still stored, in a shared dictionary, so
/// the column is never lossy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatementId {
    /// The row cites no statement.
    #[default]
    Absent,
    /// The `{QID}-{UUID}` form, with the UUID alone.
    Uuid([u8; 16]),
    /// Anything else, as an id into the set's fallback strings.
    Other(u32),
}

/// What a statement cell parses to before it is interned.
pub(super) enum ParsedStatement {
    Absent,
    Uuid([u8; 16]),
    /// Text that is not in the UUID form, kept as a borrowed slice where possible.
    Other,
}

impl StatementId {
    /// Read a statement cell, which may be a full URI or an already-bare id.
    ///
    /// `raw` must be the cell with the URI prefix already stripped, because that
    /// prefix is the row's compound QID and is not carried here.
    ///
    /// `compound_numeric` is checked rather than assumed, because the 16-byte form
    /// rebuilds its text from the compound and a disagreeing prefix would come
    /// back pointing at the wrong entity. Measured at 99.97%, so the check almost
    /// never fires; those few keep their own text.
    pub(super) fn parse(raw: &str, compound_numeric: u32) -> ParsedStatement {
        let bare = raw.trim();
        if bare.is_empty() {
            return ParsedStatement::Absent;
        }
        // `split_once`, not `rsplit_once`: a UUID contains hyphens of its own, so
        // splitting at the last one leaves twelve characters and never parses.
        // That mistake made every statement take the fallback path and cost 63 MB
        // at a million rows.
        if let Some((prefix, uuid)) = bare.split_once('-')
            && prefix.strip_prefix('Q').and_then(parse_numeric_qid) == Some(compound_numeric)
            && let Some(bytes) = parse_uuid(uuid)
        {
            return ParsedStatement::Uuid(bytes);
        }
        ParsedStatement::Other
    }

    /// The id as it is shown, given the compound it belongs to.
    #[must_use]
    pub fn text(&self, compound_qid: &str, fallbacks: &SparseStrings) -> Option<String> {
        match *self {
            Self::Absent => None,
            Self::Uuid(bytes) => Some(format!("{compound_qid}-{}", format_uuid(&bytes))),
            Self::Other(id) => fallbacks.value(id as usize).map(ToOwned::to_owned),
        }
    }
}

/// The URI prefix a statement cell arrives with, stripped without allocating.
pub(super) fn strip_statement_prefix(cell: &str) -> &str {
    let trimmed = cell.trim();
    trimmed
        .strip_prefix(WIKIDATA_STATEMENT_BASE)
        .unwrap_or(trimmed)
}

/// Sixteen bytes from a hyphenated UUID, or `None` if it is not one.
///
/// Two hex characters make a byte, so the loop alternates between the high and low
/// half of the byte it fills. Advancing the byte index on every *nibble* instead
/// writes past the end of a sixteen-byte array after the first sixteen characters
/// and returns `None` for every UUID ever seen. That went unnoticed because the
/// caller keeps the whole string on `None` and the rebuilt text is the same either
/// way, so a round-trip test cannot tell the two paths apart -- hence
/// `a_uuid_statement_takes_the_sixteen_byte_path_and_not_the_text_one` asserts on
/// the storage.
fn parse_uuid(text: &str) -> Option<[u8; 16]> {
    /// Byte widths of the five groups of a hyphenated UUID.
    const WIDTHS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut out = [0u8; 16];
    let mut at = 0usize;
    let mut low_half = false;

    for (index, group) in text.split('-').enumerate() {
        if group.len() != WIDTHS.get(index).copied()? {
            return None;
        }
        for byte in group.bytes() {
            let nibble = u8::try_from(char::from(byte).to_digit(16)?).ok()?;
            let slot = out.get_mut(at)?;
            if low_half {
                *slot |= nibble;
                at += 1;
                low_half = false;
            } else {
                *slot = nibble << 4;
                low_half = true;
            }
        }
    }

    // A well-formed UUID has 32 hex characters, which is exactly 16 bytes with no
    // half-filled one left over.
    (at == out.len() && !low_half).then_some(out)
}

/// The hyphenated uppercase form of a UUID.
fn format_uuid(bytes: &[u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        out.push(char::from(
            HEX.get((byte >> 4) as usize).copied().unwrap_or(b'0'),
        ));
        out.push(char::from(
            HEX.get((byte & 0x0f) as usize).copied().unwrap_or(b'0'),
        ));
    }
    out
}
