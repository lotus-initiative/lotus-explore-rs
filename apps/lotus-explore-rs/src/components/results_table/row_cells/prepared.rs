// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_model::CompoundEntry;
use std::sync::Arc;

#[derive(Clone, PartialEq, Debug)]
pub(in crate::components::results_table) struct PreparedRow {
    pub(super) display_name: Arc<str>,
    pub(super) display_name_short: Arc<str>,
    pub(super) depict_url: Option<Arc<str>>,
    pub(super) doi: Option<Arc<str>>,
    pub(super) statement_id: Option<Arc<str>>,
    pub(super) reference_title_short: Option<Arc<str>>,
    pub(super) short_inchikey: Option<Arc<str>>,
}

impl PreparedRow {
    /// Derive the row's display fields from one materialised row.
    ///
    /// Called only for the rows on screen. The previous shape pre-derived one per row
    /// for the whole result set, including URL-encoding a SMILES into a CDK depict link
    /// — a hundred-odd bytes per row, for the thirty a viewport holds.
    pub(in crate::components::results_table) fn from_entry(entry: &CompoundEntry) -> Self {
        let display_name = normalized_display_name(entry);
        Self {
            display_name_short: display_name.clone(),
            depict_url: depict_url_cached(entry),
            doi: trimmed_optional_arc(entry.ref_doi.as_deref()),
            statement_id: trimmed_statement_id_arc(entry.statement.as_deref()),
            reference_title_short: trimmed_optional_arc(entry.ref_title.as_deref()),
            short_inchikey: entry.inchikey.as_deref().map(short_inchikey_arc),
            display_name,
        }
    }
}

fn short_inchikey_arc(ik: &str) -> Arc<str> {
    Arc::<str>::from(ik.split('-').next().unwrap_or(ik))
}

fn normalized_display_name(entry: &CompoundEntry) -> Arc<str> {
    let trimmed = entry.name.trim();
    if trimmed.is_empty() {
        entry.compound_qid.clone()
    } else if trimmed.len() == entry.name.len() {
        entry.name.clone()
    } else {
        Arc::<str>::from(trimmed)
    }
}

fn depict_url_cached(entry: &CompoundEntry) -> Option<Arc<str>> {
    let smiles = entry.smiles.as_deref()?.trim();
    if smiles.is_empty() || smiles.contains('\n') {
        return None;
    }
    let encoded = urlencoding::encode(smiles);
    let mut url = String::with_capacity(
        "https://www.simolecule.com/cdkdepict/depict/cow/svg?smi=".len()
            + encoded.len()
            + "&annotate=cip".len(),
    );
    url.push_str("https://www.simolecule.com/cdkdepict/depict/cow/svg?smi=");
    url.push_str(encoded.as_ref());
    url.push_str("&annotate=cip");
    Some(Arc::<str>::from(url))
}

fn trimmed_optional_arc(value: Option<&str>) -> Option<Arc<str>> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(Arc::<str>::from(trimmed))
    }
}

fn trimmed_statement_id_arc(value: Option<&str>) -> Option<Arc<str>> {
    const STMT_PREFIX: &str = "http://www.wikidata.org/entity/statement/";
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(Arc::<str>::from(
        trimmed.strip_prefix(STMT_PREFIX).unwrap_or(trimmed),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_inchikey_returns_first_segment() {
        assert_eq!(short_inchikey_arc("AAAA-BBBB-CCCC").as_ref(), "AAAA");
        assert_eq!(short_inchikey_arc("NOSPLIT").as_ref(), "NOSPLIT");
    }

    #[test]
    fn reference_title_short_keeps_trimmed_full_text() {
        let entry = CompoundEntry {
            compound_qid: Arc::<str>::from("Q1"),
            name: Arc::<str>::from("Alpha"),
            inchikey: None,
            smiles: None,
            mass: None,
            formula: None,
            taxon_qid: Arc::<str>::from("T1"),
            taxon_name: Arc::<str>::from("Taxon"),
            reference_qid: Arc::<str>::from("R1"),
            reference_node: Arc::from(""),
            ref_title: Some(Arc::<str>::from(
                "  This reference title is intentionally much longer than sixty characters to verify it stays intact.  ",
            )),
            ref_doi: None,
            pub_year: None,
            statement: None,
        };

        let prepared = PreparedRow::from_entry(&entry);
        assert_eq!(
            prepared.reference_title_short.as_deref(),
            Some(
                "This reference title is intentionally much longer than sixty characters to verify it stays intact.",
            )
        );
    }

    #[test]
    fn display_name_short_keeps_full_trimmed_name() {
        let entry = CompoundEntry {
            compound_qid: Arc::<str>::from("Q1"),
            name: Arc::<str>::from(
                "  This compound name is intentionally much longer than sixty characters to verify it stays intact.  ",
            ),
            inchikey: None,
            smiles: None,
            mass: None,
            formula: None,
            taxon_qid: Arc::<str>::from("T1"),
            taxon_name: Arc::<str>::from("Taxon"),
            reference_qid: Arc::<str>::from("R1"),
            reference_node: Arc::from(""),
            ref_title: None,
            ref_doi: None,
            pub_year: None,
            statement: None,
        };

        let prepared = PreparedRow::from_entry(&entry);
        assert_eq!(
            prepared.display_name_short.as_ref(),
            "This compound name is intentionally much longer than sixty characters to verify it stays intact.",
        );
    }

    #[test]
    fn prepared_row_caches_trimmed_and_derived_fields() {
        let entry = CompoundEntry {
            compound_qid: Arc::<str>::from("Q1"),
            name: Arc::<str>::from("  Alpha  "),
            inchikey: Some(Arc::<str>::from("AAAA-BBBB-CCCC")),
            smiles: Some(Arc::<str>::from("CCO")),
            mass: None,
            formula: None,
            taxon_qid: Arc::<str>::from("T1"),
            taxon_name: Arc::<str>::from("Taxon"),
            reference_qid: Arc::<str>::from("R1"),
            reference_node: Arc::from(""),
            ref_title: Some(Arc::<str>::from("  A fairly short title  ")),
            ref_doi: Some(Arc::<str>::from(" 10.1000/test ")),
            pub_year: None,
            statement: Some(Arc::<str>::from(
                "http://www.wikidata.org/entity/statement/Q1-ABC",
            )),
        };

        let prepared = PreparedRow::from_entry(&entry);
        assert_eq!(prepared.display_name.as_ref(), "Alpha");
        assert_eq!(prepared.display_name_short.as_ref(), "Alpha");
        assert_eq!(prepared.short_inchikey.as_deref(), Some("AAAA"));
        assert_eq!(prepared.doi.as_deref(), Some("10.1000/test"));
        assert_eq!(prepared.statement_id.as_deref(), Some("Q1-ABC"));
        assert!(
            prepared
                .depict_url
                .as_deref()
                .is_some_and(|url| url.contains("annotate=cip"))
        );
    }
}
