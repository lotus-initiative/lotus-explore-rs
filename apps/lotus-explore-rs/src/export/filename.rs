// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_search::SearchCriteria;

fn export_search_type_suffix(criteria: &SearchCriteria) -> Option<&'static str> {
    (!criteria.structure.trim().is_empty()).then(|| criteria.structure_search.as_str())
}

pub fn now_iso8601() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::new_0().to_iso_string().into()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};

        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs().cast_signed());
        let (y, m, d, hh, mm, ss) = epoch_to_ymdhms(secs);
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    }
}

#[cfg(not(target_arch = "wasm32"))]
// Every cast below narrows within a value already bounded by the caller: a
// time-of-day is under 86_400, a day count is four figures, and a signed
// offset becomes a `u64` count of days only after the era shift has made it
// non-negative. Widening the types to prove it would cost more than it says.
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
const fn epoch_to_ymdhms(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    // Dependency-free date conversion (Howard Hinnant, public domain).
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hh = (rem / 3600) as u32;
    let mm = ((rem % 3600) / 60) as u32;
    let ss = (rem % 60) as u32;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 {
        (mp + 3) as u32
    } else {
        (mp - 9) as u32
    };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d, hh, mm, ss)
}

pub fn today_yyyymmdd() -> String {
    now_iso8601()
        .chars()
        .filter(char::is_ascii_digit)
        .take(8)
        .collect()
}

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

pub fn generate_filename(criteria: &SearchCriteria, ext: &str) -> String {
    let date = today_yyyymmdd();
    let safe = safe_taxon_slug(&criteria.taxon);
    let mut stem = format!("{date}_lotus_{safe}");
    if let Some(st) = export_search_type_suffix(criteria) {
        stem.push('_');
        stem.push_str(st);
    }
    if criteria.has_effective_filters(crate::clock::current_year()) {
        stem.push_str("_filtered");
    }
    format!("{stem}.{ext}")
}

#[cfg(test)]
#[path = "filename/tests.rs"]
mod tests;
