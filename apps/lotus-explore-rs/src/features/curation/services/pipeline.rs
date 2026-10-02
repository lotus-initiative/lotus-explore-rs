// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]

use crate::i18n::Locale;
use lotus_curation::{
    CurationError, CurationInputRow, CurationResultRow, QuickStatementsBundle,
    build_quickstatements_bundle,
};
use std::future::Future;

// Curation drives Qlever with several POSTs per row (compound fetch, ASK, ...).
// QLever's anonymous quota rejects bursts, so rows are curated one at a time.

pub async fn curate_rows<F, Fut>(
    locale: Locale,
    rows: Vec<CurationInputRow>,
    curate_single_row: F,
    row_uniqueness_key: fn(&CurationInputRow) -> String,
) -> Result<(Vec<CurationResultRow>, QuickStatementsBundle), CurationError>
where
    F: Fn(Locale, CurationInputRow) -> Fut,
    Fut: Future<Output = CurationResultRow>,
{
    let mut seen_keys = std::collections::HashSet::with_capacity(rows.len());
    let mut unique_rows = Vec::with_capacity(rows.len());
    for row in rows {
        if seen_keys.insert(row_uniqueness_key(&row)) {
            unique_rows.push(row);
        }
    }

    let mut results = Vec::with_capacity(unique_rows.len());
    for row in unique_rows {
        results.push(curate_single_row(locale, row).await);
    }
    let bundle = build_quickstatements_bundle(&results);
    Ok((results, bundle))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use futures::executor::block_on;
    use lotus_curation::{CurationInputRow, CurationStatus};

    fn key(row: &CurationInputRow) -> String {
        row.smiles.to_ascii_lowercase()
    }

    async fn fake_curate(_locale: Locale, input: CurationInputRow) -> CurationResultRow {
        CurationResultRow {
            input,
            canonical_smiles: None,
            inchikey: None,
            inchi: None,
            formula: None,
            exact_mass: None,
            mass_warning: None,
            wikidata_qid: None,
            status: CurationStatus::NewCompound,
            note: String::new(),
            dependency_blocks: Vec::new(),
            quickstatements: vec!["CREATE".to_string()],
        }
    }

    #[test]
    fn curate_rows_deduplicates_and_preserves_input_order() {
        let rows = vec![
            CurationInputRow {
                name: "a".into(),
                smiles: "CCO".into(),
                taxon: None,
                doi: None,
            },
            CurationInputRow {
                name: "dup".into(),
                smiles: "cco".into(),
                taxon: None,
                doi: None,
            },
            CurationInputRow {
                name: "b".into(),
                smiles: "N".into(),
                taxon: None,
                doi: None,
            },
        ];

        let (result_rows, _bundle) =
            block_on(curate_rows(Locale::En, rows, fake_curate, key)).expect("pipeline result");
        let names = result_rows
            .into_iter()
            .map(|r| r.input.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["a", "b"]);
    }
}
