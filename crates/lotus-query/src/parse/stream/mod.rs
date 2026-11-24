// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Reading a whole result set without holding the payload.
//!
//! # Why not the `csv` reader
//!
//! The [`csv`] crate reads a `Read` incrementally, but reads to end of *input* to know it
//! is done. Fine for a file, wrong for a body arriving in chunks: no `Read` can say "not
//! yet, ask me again", and zero bytes is end of input to `csv`, truncating the result at
//! the first chunk boundary.
//!
//! So record splitting is done here, one chunk at a time, an unfinished record carried
//! across the boundary. The rules are RFC 4180's -- quoted fields, `""` for a literal
//! quote, newlines inside quotes -- because reference titles here contain commas, quotes
//! and occasional newlines, and mishandling them silently shifts every column to the right.
//!
//! Parsing is as forgiving as
//! [`parse_compounds_csv_capped`](super::parse_compounds_csv_capped): a short row reads as
//! missing trailing fields rather than failing, since a search that returned usable rows
//! should not be discarded over one malformed cell.

mod fields;
mod reader;
mod splitter;

pub use fields::parse_compounds_columnar;
pub use reader::CsvColumnarReader;
pub use splitter::CsvSplitter;
