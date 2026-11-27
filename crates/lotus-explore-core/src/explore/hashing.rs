// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Query and result-set SHA-256 hashing helpers.

#[cfg(test)]
use lotus_model::CompoundEntry;
use lotus_model::{ColumnarResultSet, SearchCriteria};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// Compute the query hash and the result hash for a search.
///
/// The query hash covers the resolved QID, the taxon, and every serialized
/// criterion; the result hash covers the ordered compound QIDs in `set`.
#[must_use]
pub(in crate::explore) fn compute_hashes(
    qid: &str,
    criteria: &SearchCriteria,
    set: &ColumnarResultSet,
    year: u16,
) -> (String, String) {
    let normalized_qid = if qid.trim().is_empty() { "*" } else { qid };
    let normalized_taxon = criteria.taxon.trim();
    let mut query_source =
        String::with_capacity(normalized_qid.len() + normalized_taxon.len() + 64);
    let _ = write!(query_source, "{normalized_qid}|{normalized_taxon}");

    // Build `|key=value&key=value&…` suffix without an intermediate Vec<String>.
    let params = crate::explore::url_codec::criteria_query_params(criteria, year);
    for (i, (k, v)) in params.into_iter().enumerate() {
        if i == 0 {
            query_source.push('|');
        } else {
            query_source.push('&');
        }
        let _ = write!(query_source, "{k}={v}");
    }
    let query_hash = lotus_model::encode_hex_lower(&Sha256::digest(query_source.as_bytes()));

    // The compound dictionary is already the distinct set of QIDs: iterate it and
    // stream each one into the hasher, nothing collected or sorted.
    //
    // The bytes hashed are `Q` and the digits, the same as when the QID was stored
    // as text — a changed hash shape would make every shared link look like a
    // different result. One reusable buffer, because this runs over two and a half
    // million QIDs per search and an allocation each would be 2.5 million.
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
    let result_hash = lotus_model::encode_hex_lower(&result_hasher.finalize());

    (query_hash, result_hash)
}

#[cfg(test)]
#[path = "hashing/tests.rs"]
mod tests;
