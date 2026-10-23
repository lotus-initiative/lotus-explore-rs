// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The name an export is written under.
//!
//! The date in the name and the year that decides whether a filter "did
//! anything" both come from the clock, and the clock does not belong to a
//! domain crate. So they arrive as arguments: `year` from the host, and
//! `now` as the ISO-8601 stamp the host read. The `YYYYMMDD` prefix is then
//! derived from the stamp by this module, which is a string operation and
//! needs no clock of its own.

use lotus_search::SearchCriteria;

fn export_search_type_suffix(criteria: &SearchCriteria) -> Option<&'static str> {
    (!criteria.structure.trim().is_empty()).then(|| criteria.structure_search.as_str())
}

/// The `YYYYMMDD` prefix of an export's name, taken from an ISO-8601 stamp.
///
/// The stamp's digits, in order, with everything else dropped. Derived rather
/// than read, so a caller that already has the instant does not have to ask a
/// second time and get a second answer.
#[must_use]
pub fn today_yyyymmdd(iso_timestamp: &str) -> String {
    iso_timestamp
        .chars()
        .filter(char::is_ascii_digit)
        .take(8)
        .collect()
}

/// The taxon's name, reduced to something a filename may hold.
///
/// `*` becomes `all_taxa` rather than `star`, because the file is read by
/// people and "`all_taxa`" says what the file is; every other unusable
/// character becomes an underscore or is dropped.
#[must_use]
pub fn safe_taxon_slug(taxon: &str) -> String {
    let t = taxon.trim();
    if t.is_empty() {
        return "any_taxon".to_string();
    }
    if t == "*" {
        return "all_taxa".to_string();
    }
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            ' ' | '/' | '\\' | ':' => out.push('_'),
            '*' => out.push_str("star"),
            '?' | '"' | '<' | '>' | '|' => {}
            _ => out.push(c),
        }
    }
    out
}

/// The name an export of this search is written under.
///
/// `year` decides whether the search carries a year filter, and `now` dates the
/// file. Both are the caller's to supply: this module cannot read a clock.
#[must_use]
pub fn generate_filename(criteria: &SearchCriteria, ext: &str, year: u16, now: &str) -> String {
    let date = today_yyyymmdd(now);
    let safe = safe_taxon_slug(&criteria.taxon);
    let mut stem = format!("{date}_lotus_{safe}");
    if let Some(st) = export_search_type_suffix(criteria) {
        stem.push('_');
        stem.push_str(st);
    }
    if criteria.has_effective_filters(year) {
        stem.push_str("_filtered");
    }
    format!("{stem}.{ext}")
}

#[cfg(test)]
#[path = "filename/tests.rs"]
mod tests;
