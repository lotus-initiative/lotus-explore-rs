// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! A result set stored by column, with the strings interned.
//!
//! # Why this exists
//!
//! A [`CompoundEntry`] is the right shape for one row and the wrong shape for
//! three million of them. It carries thirteen fields, eight of which are not row
//! data at all: the compound's name, `InChIKey`, SMILES, mass and formula belong
//! to the *compound*, the title, DOI and year belong to the *reference*, and the
//! scientific name belongs to the *taxon*. A row repeats them, and the table
//! re-sorts and re-filters the same set many times over.
//!
//! What is stored per row here is three dictionary ids and one statement id --
//! 28 bytes -- against roughly 200 bytes for the same row as a [`CompoundEntry`].
//!
//! # The measurements those numbers come from
//!
//! Taken from `QLever` in October 2026. The whole graph is 2,990,730 edges over
//! 2,502,164 compounds, 2,818,725 statements, 37,771 taxa and 91,706 references.
//! One export of 50,000 rows in the columns the app selects is 15.7 MB of CSV
//! (314 B/row) and 3.0 MB gzipped (60 B/row).
//!
//! Two of those measurements changed this design, and both contradicted what it
//! was first written expecting:
//!
//! - **`statement` does not compress.** It is `{compoundQID}-{UUID}` -- a hyphen,
//!   not a `$` -- and the UUID is random: 49,246 distinct statements in the sample
//!   had 49,246 distinct suffixes, so interning the string buys nothing. What *is*
//!   redundant is the prefix, which equals the row's own compound QID in 49,983 of
//!   50,000 rows. Hence [`StatementId::Uuid`], 16 bytes, text rebuilt on demand.
//! - **The compound dictionary barely deduplicates.** 2,990,730 edges over
//!   2,502,164 compounds is 1.2 rows per compound, so the dictionary is nearly one
//!   entry per row and there is no repetition to exploit. The saving comes from
//!   the id columns and from **sparsity** -- an `InChIKey` is present on 20.7% of
//!   rows, a SMILES on 15.0%, a mass on 10.3%, a formula on 8.3% -- rather than
//!   from deduplication.
//!
//! At 1.5M rows the whole set is about 112 MB. At the full 3M-edge graph the same
//! arithmetic lands near 210 MB. That second number is over a 200 MB budget, and
//! it is the one query where the budget binds: the unfiltered whole-graph search.
//!
//! Two properties make the set *exact*, which is the point of the type:
//!
//! - **Absence is a first-class value.** [`NO_VALUE`] is an id no real
//!   dictionary entry can hold, so "this row has no taxon" stays
//!   distinguishable from "this row has taxon number zero". Collapsing the two
//!   would silently change `n_taxa`.
//! - **Rows are stored raw.** The deduplication a [`CompoundEntry`] arrives
//!   *after* has not happened, so `n_entries` is the endpoint's `COUNT(*)` and
//!   not a lower bound on it.
//!
//! # What it costs
//!
//! The set is exact but not free to build. The compound dictionary is close to
//! one entry per row for a wide search, because the graph holds about 1.2
//! occurrences per compound, so the saving comes from the id columns rather than
//! from deduplicating the dictionaries.

use super::{CompoundEntry, DatasetStats, WIKIDATA_ENTITY_BASE, WIKIDATA_STATEMENT_BASE};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Hands out each built set an identity that no other set shares.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// A fresh identity for a set that has just been built.
///
/// Ids start at one so that the empty state, which keeps generation zero, can
/// never collide with a real result set.
fn next_generation() -> u64 {
    NEXT_GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// The id that means "this row has no value here".
///
/// A real dictionary can never mint it: ids come from [`Dictionary::intern`] in
/// row order, and this one is handed out instead. A million-row result set does
/// not need a million compounds of dictionary to be safe to index.
pub const NO_VALUE: u32 = u32::MAX;

/// A set of interned strings, and the ids that name them.
///
/// Lookup is by `&str` rather than by `Arc<str>` because every call site has a
/// borrowed field from a CSV record and no reason to allocate one to find out
/// whether it has been seen before.
#[derive(Debug, Default, Clone)]
pub struct Dictionary {
    values: Vec<Arc<str>>,
    index: HashMap<Arc<str>, u32>,
}

impl Dictionary {
    /// An empty dictionary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The id for `value`, minting one if it is new.
    ///
    /// An empty `value` is *not* interned; it yields [`NO_VALUE`]. Two empty
    /// cells therefore share one absence rather than each taking a dictionary
    /// slot, which is what keeps a 90%-sparse column from costing a slot per row.
    pub fn intern(&mut self, value: &str) -> u32 {
        if value.is_empty() {
            return NO_VALUE;
        }
        if let Some(id) = self.index.get(value) {
            return *id;
        }
        let Ok(id) = u32::try_from(self.values.len()) else {
            return NO_VALUE;
        };
        let shared: Arc<str> = Arc::from(value);
        self.values.push(Arc::clone(&shared));
        self.index.insert(shared, id);
        id
    }

    /// The string an id names, or `None` for [`NO_VALUE`] and out-of-range ids.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&str> {
        if id == NO_VALUE {
            return None;
        }
        self.values.get(id as usize).map(AsRef::as_ref)
    }

    /// How many distinct values are interned.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether nothing has been interned.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The ids that actually appear in `ids`, as a bitmap over dictionary ids.
    ///
    /// This is how the exact counts are taken. The alternative -- a `HashSet` of
    /// borrowed strings per count, sized to the row count -- was itself a large
    /// transient allocation; this is a bitmap sized to the dictionary.
    #[must_use]
    pub fn used<'a>(&self, ids: impl Iterator<Item = &'a u32>) -> Bitmask {
        let mut mask = Bitmask::with_len(self.values.len());
        for id in ids {
            if *id != NO_VALUE {
                mask.insert(*id);
            }
        }
        mask
    }
}

/// A fixed-size set of ids, stored as words.
///
/// The filter path needs one of these per constrained column, and building one
/// costs a pass over a *dictionary* rather than a pass over the rows. That
/// inversion is what makes filtering a multi-million-row set as fast as
/// filtering five hundred rows.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Bitmask {
    words: Vec<u64>,
}

impl Bitmask {
    /// A mask over `len` ids, all clear.
    #[must_use]
    pub fn with_len(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(64)],
        }
    }

    /// Add `id`, ignoring [`NO_VALUE`] and out-of-range ids.
    pub fn insert(&mut self, id: u32) {
        if id == NO_VALUE {
            return;
        }
        if let Some(word) = self.words.get_mut((id / 64) as usize) {
            *word |= 1u64 << (id % 64);
        }
    }

    /// Whether `id` is in the mask. [`NO_VALUE`] never is.
    #[must_use]
    pub fn contains(&self, id: u32) -> bool {
        if id == NO_VALUE {
            return false;
        }
        self.words
            .get((id / 64) as usize)
            .is_some_and(|w| w & (1u64 << (id % 64)) != 0)
    }

    /// How many ids are in the mask.
    #[must_use]
    pub fn len(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Whether the mask holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }
}

/// Strings stored beside a dictionary entry, for columns that are mostly absent.
///
/// An `Option<Arc<str>>` column over two and a half million compounds costs 16
/// bytes per compound even when four fifths of them have no value. This costs
/// four, and allocates only for the values that exist.
#[derive(Debug, Default, Clone)]
pub struct SparseStrings {
    values: Vec<Arc<str>>,
    ids: Vec<u32>,
}

impl SparseStrings {
    /// An empty set of slots.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record `value` for dictionary slot `slot`, replacing what was there.
    ///
    /// Slots are filled in ascending order by [`ColumnarBuilder`], which is what
    /// makes this an append in the common case: a slot is created by the first
    /// row mentioning the compound, so there is never a gap to skip.
    pub fn set(&mut self, slot: usize, value: Option<&str>) {
        let id = value
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map_or(NO_VALUE, |v| {
                let Ok(id) = u32::try_from(self.values.len()) else {
                    return NO_VALUE;
                };
                self.values.push(Arc::from(v));
                id
            });
        if slot == self.ids.len() {
            self.ids.push(id);
        } else if let Some(existing) = self.ids.get_mut(slot) {
            // A later row wins. The query asks for a property the same way for
            // every row, so the values agree; where they do not, the last one
            // read is as defensible as the first.
            *existing = id;
        }
    }

    /// The id [`set`](Self::set) would give the next value, without storing it.
    #[must_use]
    pub fn next_id(&self) -> u32 {
        u32::try_from(self.values.len()).unwrap_or(NO_VALUE)
    }

    /// The value recorded for `slot`.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<&str> {
        let id = *self.ids.get(slot)?;
        if id == NO_VALUE {
            return None;
        }
        self.values.get(id as usize).map(AsRef::as_ref)
    }
}

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
enum ParsedStatement {
    Absent,
    Uuid([u8; 16]),
    /// Text that is not in the UUID form, kept as a borrowed slice where possible.
    Other,
}

impl StatementId {
    /// Read a statement cell, which may be a full URI or an already-bare id.
    ///
    /// `raw` must be the cell with the URI prefix already stripped, because the
    /// prefix is the row's compound QID and is not carried here.
    fn parse(raw: &str) -> ParsedStatement {
        let bare = raw.trim();
        if bare.is_empty() {
            return ParsedStatement::Absent;
        }
        if let Some((_, uuid)) = bare.rsplit_once('-')
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
            Self::Other(id) => fallbacks.get(id as usize).map(ToOwned::to_owned),
        }
    }
}

/// The URI prefix a statement cell arrives with, stripped without allocating.
fn strip_statement_prefix(cell: &str) -> &str {
    let trimmed = cell.trim();
    trimmed
        .strip_prefix(WIKIDATA_STATEMENT_BASE)
        .unwrap_or(trimmed)
}

/// Sixteen bytes from a hyphenated UUID, or `None` if it is not one.
fn parse_uuid(text: &str) -> Option<[u8; 16]> {
    /// Byte widths of the five groups of a hyphenated UUID.
    const WIDTHS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut out = [0u8; 16];
    let mut at = 0;
    for (index, group) in text.split('-').enumerate() {
        let width = WIDTHS.get(index).copied()?;
        if group.len() != width {
            return None;
        }
        for byte in group.bytes() {
            let nibble = char::from(byte).to_digit(16)?;
            let slot = out.get_mut(at)?;
            *slot = (*slot << 4) | u8::try_from(nibble).ok()?;
            at += 1;
        }
    }
    (at == 16).then_some(out)
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

/// One row as it comes off the wire, before anything is interned.
///
/// Borrowed, so the CSV parser can hand over a record's fields without
/// allocating a `String` per field per row -- the largest avoidable cost in the
/// row-at-a-time path.
#[derive(Debug, Clone, Copy, Default)]
pub struct RawRow<'a> {
    /// Wikidata QID of the compound. Required.
    pub compound_qid: &'a str,
    /// Compound label; empty when there is none.
    pub name: &'a str,
    /// `InChIKey`, if the compound has one.
    pub inchikey: Option<&'a str>,
    /// SMILES, if the compound has one.
    pub smiles: Option<&'a str>,
    /// Monoisotopic mass in daltons, if the compound has one.
    pub mass: Option<f64>,
    /// Molecular formula, if the compound has one.
    pub formula: Option<&'a str>,
    /// Wikidata QID of the taxon; empty when there is none.
    pub taxon_qid: &'a str,
    /// Scientific name of the taxon; empty when there is none.
    pub taxon_name: &'a str,
    /// Wikidata QID of the reference; empty when there is none.
    pub reference_qid: &'a str,
    /// Title of the reference, if it has one.
    pub ref_title: Option<&'a str>,
    /// DOI of the reference, if it has one.
    pub ref_doi: Option<&'a str>,
    /// Publication year, if the reference has one.
    pub pub_year: Option<i16>,
    /// Statement backing the occurrence, if there is one.
    pub statement: Option<&'a str>,
}

/// A result set in columnar form.
///
/// Built by [`ColumnarBuilder::build`]; there is no other way to make one, so a
/// set always has its statistics computed and its dictionaries consistent.
#[derive(Debug, Clone)]
pub struct ColumnarResultSet {
    compound_ids: Vec<u32>,
    taxon_ids: Vec<u32>,
    reference_ids: Vec<u32>,
    statements: Vec<StatementId>,
    compounds: Dictionary,
    compound_names: SparseStrings,
    compound_inchikeys: SparseStrings,
    compound_smiles: SparseStrings,
    compound_masses: Vec<f64>,
    compound_formulas: SparseStrings,
    taxa: Dictionary,
    taxon_names: SparseStrings,
    references: Dictionary,
    reference_titles: SparseStrings,
    reference_dois: SparseStrings,
    reference_years: Vec<Option<i16>>,
    statement_fallbacks: SparseStrings,
    stats: DatasetStats,
    /// Identifies this set, so it can be compared without being traversed.
    generation: u64,
}

/// Two sets are equal when they are the same set.
///
/// A derived comparison would walk every row and every dictionary, which for a
/// three-million-row set is the most expensive thing in the program -- and the
/// answer would be "yes" or "no" about something the caller can already see,
/// because it holds both. So this compares the identity [`build`](ColumnarBuilder::build)
/// stamped on each set instead: cheap, and exact in the sense that matters, which
/// is that a new result set is never equal to the one it replaced.
///
/// A clone keeps the identity, which is right: a clone is the same result.
impl PartialEq for ColumnarResultSet {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation
    }
}

/// The mass column's absence marker.
///
/// A mass is a measured quantity in daltons, so `NaN` cannot be one. That makes
/// it a free absence marker in a column that is over 90% absent, where
/// `Option<f64>` would cost eight bytes of padding per compound.
const NO_MASS: f64 = f64::NAN;

impl Default for ColumnarResultSet {
    /// The empty set.
    ///
    /// Generation **zero**, deliberately, and not one from the counter. Every
    /// empty set means the same thing -- "no search has run" -- so two of them
    /// must compare equal: the reducer builds one to reset the state, and a
    /// counter would make a freshly reset state look like a changed one and
    /// re-render the table for nothing.
    ///
    /// Zero is not an id [`ColumnarBuilder::build`] hands out, so an empty set is
    /// never equal to a real result set.
    fn default() -> Self {
        Self::empty_fields()
    }
}

impl ColumnarResultSet {
    /// Every field at its default, with no identity stamped on it.
    fn empty_fields() -> Self {
        Self {
            compound_ids: Vec::new(),
            taxon_ids: Vec::new(),
            reference_ids: Vec::new(),
            statements: Vec::new(),
            compounds: Dictionary::new(),
            compound_names: SparseStrings::new(),
            compound_inchikeys: SparseStrings::new(),
            compound_smiles: SparseStrings::new(),
            compound_masses: Vec::new(),
            compound_formulas: SparseStrings::new(),
            taxa: Dictionary::new(),
            taxon_names: SparseStrings::new(),
            references: Dictionary::new(),
            reference_titles: SparseStrings::new(),
            reference_dois: SparseStrings::new(),
            reference_years: Vec::new(),
            statement_fallbacks: SparseStrings::new(),
            stats: DatasetStats::default(),
            generation: 0,
        }
    }

    /// Build a set from rows that already exist as [`CompoundEntry`] values.
    ///
    /// For the CLI, for tests, and for anything holding a page rather than a
    /// stream. The statistics are the ones the stream path computes, which is
    /// what lets a test compare the two.
    #[must_use]
    pub fn from_entries(entries: &[CompoundEntry]) -> Self {
        let mut builder = ColumnarBuilder::new();
        for entry in entries {
            builder.push(RawRow {
                compound_qid: &entry.compound_qid,
                name: &entry.name,
                inchikey: entry.inchikey.as_deref(),
                smiles: entry.smiles.as_deref(),
                mass: entry.mass,
                formula: entry.formula.as_deref(),
                taxon_qid: &entry.taxon_qid,
                taxon_name: &entry.taxon_name,
                reference_qid: &entry.reference_qid,
                ref_title: entry.ref_title.as_deref(),
                ref_doi: entry.ref_doi.as_deref(),
                pub_year: entry.pub_year,
                statement: entry.statement.as_deref(),
            });
        }
        builder.build()
    }

    /// How many rows.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.compound_ids.len()
    }

    /// Whether the set has no rows.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.compound_ids.is_empty()
    }

    /// The counts, computed once when the set was built.
    ///
    /// Exact over the whole set, because a columnar set holds every row the
    /// endpoint returned and deduplicates nothing.
    #[must_use]
    pub fn stats(&self) -> DatasetStats {
        self.stats.clone()
    }

    /// How many distinct compounds are interned.
    ///
    /// A dictionary can hold a compound no row ended up keeping, so this is not
    /// [`stats`](Self::stats)' `n_compounds`.
    #[must_use]
    pub const fn compound_count(&self) -> usize {
        self.compounds.len()
    }

    /// Every distinct compound QID the set holds, in first-seen order.
    ///
    /// Already deduplicated, which is what makes this the cheap way to hash a
    /// result: the old row-at-a-time path collected a `Vec<&str>` of every row's
    /// compound QID and sorted it to get here, so for a three-million-row result
    /// it allocated three million entries to find two and a half million.
    pub fn compound_qids(&self) -> impl Iterator<Item = &str> {
        self.compounds.values.iter().map(AsRef::as_ref)
    }

    /// The compound QID of row `row`.
    #[must_use]
    pub fn compound_qid(&self, row: usize) -> Option<&str> {
        self.compounds.get(*self.compound_ids.get(row)?)
    }

    /// The taxon QID of row `row`, if it has one.
    #[must_use]
    pub fn taxon_qid(&self, row: usize) -> Option<&str> {
        self.taxa.get(*self.taxon_ids.get(row)?)
    }

    /// The reference QID of row `row`, if it has one.
    #[must_use]
    pub fn reference_qid(&self, row: usize) -> Option<&str> {
        self.references.get(*self.reference_ids.get(row)?)
    }

    /// The compound's label, falling back to its QID.
    ///
    /// The fallback is the one the table applies to a row with no label, kept
    /// here so a caller reading the set and a caller rebuilding a [`CompoundEntry`]
    /// agree on what the cell says.
    #[must_use]
    pub fn compound_label(&self, row: usize) -> Option<&str> {
        let id = *self.compound_ids.get(row)?;
        self.compound_names
            .get(id as usize)
            .filter(|n| !n.is_empty())
            .or_else(|| self.compounds.get(id))
    }

    /// The compound's `InChIKey`, if it has one.
    ///
    /// The row-level readers below exist so that sorting and filtering never have
    /// to build a [`CompoundEntry`]. Materialising a row to compare two of them
    /// would allocate thirteen `Arc<str>`s per comparison, and a sort of three
    /// million rows does three million of those.
    #[must_use]
    pub fn inchikey(&self, row: usize) -> Option<&str> {
        self.compound_inchikeys
            .get(Self::slot(self.compound_ids.get(row))?)
    }

    /// The compound's SMILES, if it has one.
    #[must_use]
    pub fn smiles(&self, row: usize) -> Option<&str> {
        self.compound_smiles
            .get(Self::slot(self.compound_ids.get(row))?)
    }

    /// The compound's molecular formula, if it has one.
    #[must_use]
    pub fn formula(&self, row: usize) -> Option<&str> {
        self.compound_formulas
            .get(Self::slot(self.compound_ids.get(row))?)
    }

    /// The reference's title, if it has one.
    #[must_use]
    pub fn reference_title(&self, row: usize) -> Option<&str> {
        self.reference_titles
            .get(Self::slot(self.reference_ids.get(row))?)
    }

    /// The reference's DOI, if it has one.
    #[must_use]
    pub fn reference_doi(&self, row: usize) -> Option<&str> {
        self.reference_dois
            .get(Self::slot(self.reference_ids.get(row))?)
    }

    /// The reference's publication year, if it has one.
    #[must_use]
    pub fn pub_year(&self, row: usize) -> Option<i16> {
        self.reference_year(slot_to_id(Self::slot(self.reference_ids.get(row))?))
    }

    /// The compound's mass in daltons, if it has one.
    #[must_use]
    pub fn mass(&self, row: usize) -> Option<f64> {
        self.compound_mass(slot_to_id(Self::slot(self.compound_ids.get(row))?))
    }

    /// The dictionary slot `id` names, or `None` when the row does not have one.
    ///
    /// A row with no taxon or no reference carries [`NO_VALUE`], which casts to
    /// `u32::MAX`; returning `None` for it keeps every caller from having to
    /// remember that, and from a slot index of four billion reaching a `Vec`.
    fn slot(id: Option<&u32>) -> Option<usize> {
        let id = *id?;
        (id != NO_VALUE).then_some(id as usize)
    }

    /// The scientific name of row `row`'s taxon.
    #[must_use]
    pub fn taxon_label(&self, row: usize) -> Option<&str> {
        self.taxon_names.get(Self::slot(self.taxon_ids.get(row))?)
    }

    /// The statement backing row `row`, as the table shows it.
    #[must_use]
    pub fn statement_text(&self, row: usize) -> Option<String> {
        let statement = *self.statements.get(row)?;
        let compound = self.compound_qid(row)?;
        statement.text(compound, &self.statement_fallbacks)
    }

    /// The compound's mass, or `None` when it has none.
    #[must_use]
    pub fn compound_mass(&self, compound_id: u32) -> Option<f64> {
        self.compound_masses
            .get(compound_id as usize)
            .copied()
            .filter(|m| !m.is_nan())
    }

    /// The reference's publication year, or `None` when it has none.
    #[must_use]
    pub fn reference_year(&self, reference_id: u32) -> Option<i16> {
        self.reference_years
            .get(reference_id as usize)
            .copied()
            .flatten()
    }

    /// Rebuild row `row` as a [`CompoundEntry`].
    ///
    /// For the paths that still want one row at a time: the export formats, and
    /// any caller that has to hand a single result to something expecting the
    /// old shape.
    #[must_use]
    pub fn entry(&self, row: usize) -> Option<CompoundEntry> {
        let compound_id = *self.compound_ids.get(row)?;
        let reference_id = *self.reference_ids.get(row)?;
        let compound_slot = compound_id as usize;
        let reference_slot = reference_id as usize;
        let shared = |text: &str| Arc::<str>::from(text);
        let optional = |text: Option<&str>| text.map(shared);
        Some(CompoundEntry {
            compound_qid: shared(self.compound_qid(row)?),
            name: shared(self.compound_label(row).unwrap_or_default()),
            // The three QID cells keep the `Arc<str>`-and-empty convention rather
            // than the `Option` one, because `n_taxa` and `n_references` are
            // defined in terms of what is empty.
            inchikey: optional(self.compound_inchikeys.get(compound_slot)),
            smiles: optional(self.compound_smiles.get(compound_slot)),
            mass: self.compound_mass(compound_id),
            formula: optional(self.compound_formulas.get(compound_slot)),
            taxon_qid: shared(self.taxon_qid(row).unwrap_or_default()),
            taxon_name: shared(self.taxon_label(row).unwrap_or_default()),
            reference_qid: shared(self.reference_qid(row).unwrap_or_default()),
            ref_title: optional(self.reference_titles.get(reference_slot)),
            ref_doi: optional(self.reference_dois.get(reference_slot)),
            pub_year: self.reference_year(reference_id),
            statement: self.statement_text(row).map(|s| Arc::from(s.as_str())),
        })
    }

    /// Compile a filter against this set.
    ///
    /// The returned [`FilterPlan`] holds one bitmap per constrained dictionary.
    /// A row is accepted when its four ids pass, which turns a keystroke into a
    /// pass over the dictionaries plus four bit tests per row.
    #[must_use]
    pub fn plan_filter(&self, spec: &FilterSpec) -> FilterPlan {
        let mut plan = FilterPlan::default();

        if !spec.compound.trim().is_empty() {
            let needle = folded(&spec.compound);
            plan.compound = Some(self.mask_compounds(|slot, _id| {
                self.compound_names
                    .get(slot)
                    .is_some_and(|n| contains_folded(n, &needle))
                    || self
                        .compound_inchikeys
                        .get(slot)
                        .is_some_and(|k| contains_folded(k, &needle))
                    || self
                        .compounds
                        .get(slot_to_id(slot))
                        .is_some_and(|q| contains_folded(q, &needle))
            }));
        }

        if !spec.formula.trim().is_empty() {
            let needle = folded(&spec.formula);
            plan.formula = Some(self.mask_compounds(|slot, _id| {
                self.compound_formulas
                    .get(slot)
                    .is_some_and(|f| contains_folded(f, &needle))
            }));
        }

        if let Some(range) = spec.mass {
            plan.mass =
                Some(self.mask_compounds(|_slot, id| range.accepts(self.compound_mass(id))));
        }

        if !spec.taxon.trim().is_empty() {
            let needle = folded(&spec.taxon);
            let mut mask = Bitmask::with_len(self.taxa.len());
            for slot in 0..self.taxa.len() {
                let id = slot_to_id(slot);
                if self
                    .taxa
                    .get(id)
                    .is_some_and(|q| contains_folded(q, &needle))
                    || self
                        .taxon_names
                        .get(slot)
                        .is_some_and(|n| contains_folded(n, &needle))
                {
                    mask.insert(id);
                }
            }
            plan.taxon = Some(mask);
        }

        if !spec.reference.trim().is_empty() {
            let needle = folded(&spec.reference);
            let mut mask = Bitmask::with_len(self.references.len());
            for slot in 0..self.references.len() {
                let id = slot_to_id(slot);
                if self
                    .references
                    .get(id)
                    .is_some_and(|q| contains_folded(q, &needle))
                    || self
                        .reference_titles
                        .get(slot)
                        .is_some_and(|t| contains_folded(t, &needle))
                    || self
                        .reference_dois
                        .get(slot)
                        .is_some_and(|d| contains_folded(d, &needle))
                {
                    mask.insert(id);
                }
            }
            plan.reference = Some(mask);
        }

        if let Some(range) = spec.year {
            let mut mask = Bitmask::with_len(self.references.len());
            for slot in 0..self.references.len() {
                let id = slot_to_id(slot);
                // An absent year never matches: an unknown year is not a recent
                // one, and the same rule the mass range follows.
                if range.accepts(self.reference_year(id).map(f64::from)) {
                    mask.insert(id);
                }
            }
            plan.year = Some(mask);
        }

        plan
    }

    /// A bitmap over the compound dictionary, from a per-compound predicate.
    fn mask_compounds(&self, keep: impl Fn(usize, u32) -> bool) -> Bitmask {
        let mut mask = Bitmask::with_len(self.compounds.len());
        for slot in 0..self.compounds.len() {
            let id = slot_to_id(slot);
            if keep(slot, id) {
                mask.insert(id);
            }
        }
        mask
    }
}

/// Intern an entity URI as a bare QID, without allocating.
///
/// [`normalize_qid`](super::normalize_qid) would do this too, but it returns an
/// owned `String`, and this is called three times per row. At three million rows
/// that is nine million allocations spent on a substring.
///
/// Anything that is not a QID -- a property, a blank node, an empty cell -- is an
/// absence. A blank node in particular must not become a dictionary entry: every
/// blank node is distinct, so it would cost a slot per row for a value no column
/// can display.
///
/// One `String` is allocated, and only for the bare-integer form.
fn intern_qid(dictionary: &mut Dictionary, raw: &str) -> u32 {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix(WIKIDATA_ENTITY_BASE)
        .unwrap_or(trimmed);
    // Drop the `"…"^^<…#integer>` wrapper some projections render, then any
    // quotes still on the ends.
    let lexical = stripped
        .split("^^")
        .next()
        .unwrap_or(stripped)
        .trim_matches('"');

    if lexical.strip_prefix('Q').is_some_and(is_digits) {
        return dictionary.intern(lexical);
    }
    // A bare integer is how `xsd:integer(STRAFTER(STR(?c), "Q"))` renders a QID,
    // and how a plain-number projection renders one. It has to become `Q…` here:
    // the cell is rendered into a `wikidata.org/entity/` URL, and a bare number
    // in that URL is a 404 for every row.
    if is_digits(lexical) {
        let mut qid = String::with_capacity(lexical.len() + 1);
        qid.push('Q');
        qid.push_str(lexical);
        return dictionary.intern(&qid);
    }
    NO_VALUE
}

/// Whether `text` is one or more ASCII digits and nothing else.
fn is_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// A dictionary slot as an id.
fn slot_to_id(slot: usize) -> u32 {
    u32::try_from(slot).unwrap_or(NO_VALUE)
}

/// Assembles a [`ColumnarResultSet`].
///
/// The dictionaries grow as rows arrive, which is what lets the CSV parser hand
/// over records without knowing the final size first. Statistics are taken once,
/// in [`build`](Self::build), because taking them per row would cost a pass over
/// the whole set for every row that arrived.
#[derive(Debug, Default)]
pub struct ColumnarBuilder {
    set: ColumnarResultSet,
}

impl ColumnarBuilder {
    /// A builder holding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many rows have been pushed.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.set.compound_ids.len()
    }

    /// Intern one row and record its ids.
    ///
    /// A row with no compound QID is dropped: there would be nothing to key it
    /// on. This is the only place a row can be lost, and it is why `n_entries`
    /// can still disagree with the endpoint if the endpoint counts a row this
    /// cannot represent.
    pub fn push(&mut self, row: RawRow<'_>) {
        let compound_id = intern_qid(&mut self.set.compounds, row.compound_qid);
        if compound_id == NO_VALUE {
            return;
        }
        let compound_slot = compound_id as usize;
        self.grow_compounds(compound_slot);
        self.set
            .compound_names
            .set(compound_slot, present(row.name));
        if let Some(key) = non_empty(row.inchikey) {
            self.set.compound_inchikeys.set(compound_slot, Some(key));
        }
        if let Some(smiles) = non_empty(row.smiles) {
            self.set.compound_smiles.set(compound_slot, Some(smiles));
        }
        if let Some(mass) = row.mass.filter(|m| m.is_finite())
            && let Some(cell) = self.set.compound_masses.get_mut(compound_slot)
        {
            *cell = mass;
        }
        if let Some(formula) = non_empty(row.formula) {
            self.set.compound_formulas.set(compound_slot, Some(formula));
        }

        let taxon_id = intern_qid(&mut self.set.taxa, row.taxon_qid);
        let reference_id = intern_qid(&mut self.set.references, row.reference_qid);
        self.grow_references(reference_id);

        if taxon_id != NO_VALUE
            && let Some(name) = present(row.taxon_name)
        {
            self.set.taxon_names.set(taxon_id as usize, Some(name));
        }
        if reference_id != NO_VALUE {
            let slot = reference_id as usize;
            if let Some(title) = non_empty(row.ref_title) {
                self.set.reference_titles.set(slot, Some(title));
            }
            // Normalised here rather than at parse time, so the `doi.org`
            // prefix is stripped and the case fixed once per *reference* rather
            // than once per row mentioning it.
            if let Some(doi) = non_empty(row.ref_doi).and_then(super::normalize_doi) {
                self.set.reference_dois.set(slot, Some(&doi));
            }
            if let Some(year) = row.pub_year
                && let Some(cell) = self.set.reference_years.get_mut(slot)
            {
                *cell = Some(year);
            }
        }

        let statement =
            row.statement
                .map(strip_statement_prefix)
                .map_or(StatementId::Absent, |cell| match StatementId::parse(cell) {
                    ParsedStatement::Absent => StatementId::Absent,
                    ParsedStatement::Uuid(bytes) => StatementId::Uuid(bytes),
                    ParsedStatement::Other => {
                        StatementId::Other(self.intern_statement_fallback(cell))
                    }
                });

        self.set.compound_ids.push(compound_id);
        self.set.taxon_ids.push(taxon_id);
        self.set.reference_ids.push(reference_id);
        self.set.statements.push(statement);
    }

    /// Make sure the compound columns cover `slot`.
    ///
    /// Ids are minted in first-seen order, so `slot` is at most one past the end
    /// and the loop runs at most once. It is a loop so that a caller which
    /// interleaves builders cannot leave the columns ragged.
    fn grow_compounds(&mut self, slot: usize) {
        while self.compound_slots() <= slot {
            let next = self.compound_slots();
            self.set.compound_names.set(next, None);
            self.set.compound_inchikeys.set(next, None);
            self.set.compound_smiles.set(next, None);
            self.set.compound_formulas.set(next, None);
            self.set.compound_masses.push(NO_MASS);
        }
    }

    /// How many compound slots have been filled.
    const fn compound_slots(&self) -> usize {
        self.set.compound_masses.len()
    }

    /// Make sure the reference columns cover `reference_id`.
    fn grow_references(&mut self, reference_id: u32) {
        if reference_id == NO_VALUE {
            return;
        }
        while self.set.reference_years.len() <= reference_id as usize {
            let next = self.set.reference_years.len();
            self.set.reference_titles.set(next, None);
            self.set.reference_dois.set(next, None);
            self.set.reference_years.push(None);
        }
    }

    /// Intern the text of a statement that is not in the `{QID}-{UUID}` form.
    fn intern_statement_fallback(&mut self, text: &str) -> u32 {
        let id = self.set.statement_fallbacks.next_id();
        if id == NO_VALUE {
            return NO_VALUE;
        }
        self.set.statement_fallbacks.set(id as usize, Some(text));
        id
    }

    /// Take the statistics and produce the set.
    #[must_use]
    pub fn build(self) -> ColumnarResultSet {
        let compounds = self.set.compounds.used(self.set.compound_ids.iter());
        let taxa = self.set.taxa.used(self.set.taxon_ids.iter());
        let references = self.set.references.used(self.set.reference_ids.iter());
        let unique = self.distinct_triples();
        let mut set = self.set;
        set.generation = next_generation();
        set.stats = DatasetStats {
            n_compounds: compounds.len(),
            n_taxa: taxa.len(),
            n_references: references.len(),
            n_entries: set.compound_ids.len(),
            n_entries_unique: unique,
        };
        set
    }

    /// The number of distinct compound-taxon-reference triples.
    ///
    /// This is the endpoint's `COUNT(DISTINCT CONCAT(…))`, the one number a local
    /// count used to be unable to reproduce: the old parser deduplicated
    /// *before* the rows existed, so it could only report a lower bound. Sorting
    /// a copy of the id columns is the price of being exact, and the copy is
    /// dropped before [`build`](Self::build) returns.
    fn distinct_triples(&self) -> usize {
        let mut triples = self
            .set
            .compound_ids
            .iter()
            .copied()
            .zip(self.set.taxon_ids.iter().copied())
            .zip(self.set.reference_ids.iter().copied())
            .collect::<Vec<_>>();
        triples.sort_unstable();
        triples.dedup();
        triples.len()
    }
}

/// A trimmed, non-empty optional string.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.and_then(present)
}

/// A trimmed, non-empty string, or `None`.
fn present(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// The characters of `text` under Unicode lowercasing.
///
/// Folded once per filter, not once per field per row. This is the allocation
/// the old filter made six to nine times per row per keystroke.
#[must_use]
pub fn folded(text: &str) -> Vec<char> {
    text.trim().to_lowercase().chars().collect()
}

/// Whether `haystack` contains `needle`, ignoring case, without allocating.
///
/// `needle` must already be [`folded`]. The haystack is folded as it is walked,
/// which is why this costs nothing per row: lowercasing both sides into fresh
/// `String`s was the dominant cost of filtering at all.
#[must_use]
pub fn contains_folded(haystack: &str, needle: &[char]) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut rest = haystack;
    while !rest.is_empty() {
        if folded_prefix_matches(rest, needle) {
            return true;
        }
        // `map_or` rather than `unwrap`: an empty `rest` cannot reach here, but
        // the lint profile does not take the reader's word for it.
        let width = rest.chars().next().map_or(1, char::len_utf8);
        rest = rest.get(width..).unwrap_or_default();
    }
    false
}

/// Whether the folded prefix of `haystack` is `needle`.
fn folded_prefix_matches(haystack: &str, needle: &[char]) -> bool {
    let mut matched = 0;
    for ch in haystack.chars() {
        if matched == needle.len() {
            return true;
        }
        for lowered in ch.to_lowercase() {
            if matched == needle.len() {
                break;
            }
            if needle.get(matched).copied() != Some(lowered) {
                return false;
            }
            matched += 1;
        }
    }
    matched == needle.len()
}

/// An inclusive range on a measured quantity, where either end may be open.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Range {
    /// The lower bound, if the range has one.
    pub min: Option<f64>,
    /// The upper bound, if the range has one.
    pub max: Option<f64>,
}

impl Range {
    /// Whether `value` falls in the range.
    ///
    /// A missing `value` never matches. "An unknown mass is not a small mass":
    /// the alternative makes `mass <= 200` return every compound Wikidata has
    /// not weighed, which is most of them.
    #[must_use]
    pub fn accepts(self, value: Option<f64>) -> bool {
        let Some(value) = value else { return false };
        self.min.is_none_or(|min| value >= min) && self.max.is_none_or(|max| value <= max)
    }
}

/// A filter, as the table's filter row states it.
///
/// The needles are raw user input; folding happens once per compile, in
/// [`ColumnarResultSet::plan_filter`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterSpec {
    /// Substring to find in a compound's name, QID or `InChIKey`.
    pub compound: String,
    /// Substring to find in a compound's formula.
    pub formula: String,
    /// Substring to find in a taxon's QID or scientific name.
    pub taxon: String,
    /// Substring to find in a reference's QID, title or DOI.
    pub reference: String,
    /// The mass window, if the row asks for one.
    pub mass: Option<Range>,
    /// The publication-year window, if the row asks for one.
    pub year: Option<Range>,
}

impl FilterSpec {
    /// Whether any column is constrained.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.compound.trim().is_empty()
            || !self.formula.trim().is_empty()
            || !self.taxon.trim().is_empty()
            || !self.reference.trim().is_empty()
            || self.mass.is_some()
            || self.year.is_some()
    }
}

/// A compiled filter: one bitmap per constrained dictionary.
///
/// Four bit tests per row, and no string comparison per row at all.
#[derive(Debug, Clone, Default)]
pub struct FilterPlan {
    compound: Option<Bitmask>,
    formula: Option<Bitmask>,
    mass: Option<Bitmask>,
    taxon: Option<Bitmask>,
    reference: Option<Bitmask>,
    year: Option<Bitmask>,
}

impl FilterPlan {
    /// Whether the plan constrains anything.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.compound.is_some()
            || self.formula.is_some()
            || self.mass.is_some()
            || self.taxon.is_some()
            || self.reference.is_some()
            || self.year.is_some()
    }

    /// Whether row `row` of `set` survives.
    #[must_use]
    pub fn accepts(&self, set: &ColumnarResultSet, row: usize) -> bool {
        let Some(&compound) = set.compound_ids.get(row) else {
            return false;
        };
        let taxon = set.taxon_ids.get(row).copied().unwrap_or(NO_VALUE);
        let reference = set.reference_ids.get(row).copied().unwrap_or(NO_VALUE);

        passes(self.compound.as_ref(), compound)
            && passes(self.formula.as_ref(), compound)
            && passes(self.mass.as_ref(), compound)
            && passes(self.taxon.as_ref(), taxon)
            && passes(self.reference.as_ref(), reference)
            && passes(self.year.as_ref(), reference)
    }

    /// The ids of every row that survives, in row order.
    ///
    /// The table's filtered view is this list, so it is built once per filter
    /// change and then read by the sort and the virtualiser.
    #[must_use]
    pub fn surviving_rows(&self, set: &ColumnarResultSet) -> Vec<u32> {
        (0..set.row_count())
            .filter(|row| self.accepts(set, *row))
            .map(slot_to_id)
            .collect()
    }

    /// How many rows survive.
    #[must_use]
    pub fn surviving_count(&self, set: &ColumnarResultSet) -> usize {
        (0..set.row_count())
            .filter(|row| self.accepts(set, *row))
            .count()
    }
}

/// An optional mask that passes everything when it is absent.
fn passes(mask: Option<&Bitmask>, id: u32) -> bool {
    mask.is_none_or(|m| m.contains(id))
}
