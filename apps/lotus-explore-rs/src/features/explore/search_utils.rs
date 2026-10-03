// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#[cfg(test)]
use lotus_model::CompoundEntry;
use lotus_model::{ColumnarResultSet, SearchCriteria};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::fmt::Write as _;

pub fn sanitize_taxon_input(taxon: &str) -> String {
    let replaced: Cow<'_, str> = if taxon.contains('_') {
        Cow::Owned(taxon.replace('_', " "))
    } else {
        Cow::Borrowed(taxon)
    };

    let mut parts = replaced.split_whitespace();
    // If there are no tokens (blank or all-underscore input) return the replaced string.
    let Some(first_word) = parts.next() else {
        return replaced.into_owned();
    };
    // `split_whitespace` never produces empty tokens, so `first_word` is guaranteed
    // non-empty.  `chars.as_str()` gives the tail after the first char so we can
    // lowercase the rest without collecting each char individually.
    let mut chars = first_word.chars();
    let mut out = String::with_capacity(replaced.len());
    if let Some(c) = chars.next() {
        out.extend(c.to_uppercase());
        for c in chars {
            out.extend(c.to_lowercase());
        }
    }
    for part in parts {
        out.push(' ');
        out.push_str(part);
    }
    out
}

pub fn compute_hashes(
    qid: &str,
    criteria: &SearchCriteria,
    set: &ColumnarResultSet,
) -> (String, String) {
    let normalized_qid = if qid.trim().is_empty() { "*" } else { qid };
    let normalized_taxon = criteria.taxon.trim();
    let mut query_source =
        String::with_capacity(normalized_qid.len() + normalized_taxon.len() + 64);
    let _ = write!(query_source, "{normalized_qid}|{normalized_taxon}");

    // Build `|key=value&key=value&…` suffix without an intermediate Vec<String>.
    let params = crate::features::explore::url_codec::criteria_query_params(
        criteria,
        crate::clock::current_year(),
    );
    for (i, (k, v)) in params.into_iter().enumerate() {
        if i == 0 {
            query_source.push('|');
        } else {
            query_source.push('&');
        }
        let _ = write!(query_source, "{k}={v}");
    }
    let query_hash = to_hex_lower(&Sha256::digest(query_source.as_bytes()));

    // The compound dictionary is already the distinct set of QIDs, so there is
    // nothing to collect and nothing to sort: iterate it and stream each one into
    // the hasher.
    //
    // The bytes hashed are `Q` and the digits, the same as when the QID was stored
    // as text -- a result hash that changed shape would make every previously
    // shared link look like a different result. They are rendered through one
    // reusable buffer, because this runs over two and a half million QIDs at the
    // end of every search and an allocation each would be 2.5 million of them.
    let mut result_hasher = Sha256::new();
    let mut buffer = [0u8; 16];
    for (i, qid) in set.compound_qids().enumerate() {
        if i > 0 {
            result_hasher.update(b"|");
        }
        let length = lotus_model::write_qid(qid, &mut buffer);
        if let Some(text) = buffer.get(..length) {
            result_hasher.update(text);
        }
    }
    let result_hash = to_hex_lower(&result_hasher.finalize());

    (query_hash, result_hash)
}

// `HEX` has 16 entries and both nibble extracts (`b >> 4`, `b & 0x0f`)
// are always in `0..=15`, so the indexes are provably in bounds.
#[allow(
    clippy::indexing_slicing,
    reason = "`HEX` has 16 entries and both nibbles are masked to 4 bits, so neither index exceeds 15"
)]
pub fn to_hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn sanitizes_taxon_prefix_case_only() {
        assert_eq!(
            sanitize_taxon_input("voacanga africana"),
            "Voacanga africana"
        );
        assert_eq!(
            sanitize_taxon_input("VOACANGA africana"),
            "Voacanga africana"
        );
        assert_eq!(sanitize_taxon_input("__gentiana_lutea__"), "Gentiana lutea");
    }

    #[test]
    fn hex_encoder_is_lowercase() {
        assert_eq!(to_hex_lower(&[0xAB, 0xCD, 0xEF]), "abcdef");
    }

    #[test]
    fn hashes_depend_on_query_and_rows_only() {
        let crit = SearchCriteria {
            taxon: "*".into(),
            ..SearchCriteria::up_to_year(crate::clock::current_year())
        };
        let row = CompoundEntry {
            compound_qid: Arc::from("Q1"),
            name: Arc::from("A"),
            inchikey: None,
            smiles: None,
            mass: None,
            formula: None,
            taxon_qid: Arc::from("Q2"),
            taxon_name: Arc::from("Taxon"),
            reference_qid: Arc::from("Q3"),
            ref_title: None,
            ref_doi: None,
            pub_year: None,
            statement: None,
        };
        let (q1, r1) = compute_hashes(
            "QX",
            &crit,
            &ColumnarResultSet::from_entries(std::slice::from_ref(&row)),
        );
        let (q2, r2) = compute_hashes("QX", &crit, &ColumnarResultSet::from_entries(&[row]));
        assert_eq!(q1, q2);
        assert_eq!(r1, r2);
    }
}
