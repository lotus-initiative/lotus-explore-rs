// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Hash-join triplets against per-QID property tables and stream `frozen.csv`.

use std::collections::HashMap;
use std::io::Write;

use super::ExportError;
use super::index::{CompoundProps, ReferenceProps, TaxonProps, TripletRow};

/// The joined export row, in `frozen.csv` column order, borrowing from the
/// triplet row and the property indexes so joining allocates nothing.
#[derive(Clone, Copy)]
struct ExportRow<'a> {
    compound_qid: &'a str,
    compound_inchikey: &'a str,
    compound_smiles_can: &'a str,
    compound_smiles_iso: &'a str,
    compound_cid: &'a str,
    taxon_qid: &'a str,
    taxon_name: &'a str,
    taxon_ncbi: &'a str,
    taxon_ott: &'a str,
    taxon_gbif: &'a str,
    taxon_parent: &'a str,
    taxon_rank: &'a str,
    taxon_rank_label: &'a str,
    reference_qid: &'a str,
    reference_title: &'a str,
    reference_doi: &'a str,
    reference_pmid: &'a str,
    reference_pmcid: &'a str,
    reference_date: &'a str,
    reference_journal: &'a str,
    reference_journal_label: &'a str,
}

/// The columns of `frozen.csv`, in write order.
const FROZEN_COLUMNS: &[&str] = &[
    "compound_qid",
    "compound_inchikey",
    "compound_smiles_can",
    "compound_smiles_iso",
    "compound_cid",
    "taxon_qid",
    "taxon_name",
    "taxon_ncbi",
    "taxon_ott",
    "taxon_gbif",
    "taxon_parent",
    "taxon_rank",
    "taxon_rank_label",
    "reference_qid",
    "reference_title",
    "reference_doi",
    "reference_pmid",
    "reference_pmcid",
    "reference_date",
    "reference_journal",
    "reference_journal_label",
];

/// Join every triplet and stream the resulting rows to `out` as a CSV,
/// writing each row as soon as it is produced instead of buffering them all.
///
/// Returns the number of rows written.
pub(super) fn write_joined_rows<W: Write>(
    triplets: &[TripletRow],
    compounds: &HashMap<String, CompoundProps>,
    taxa: &HashMap<String, TaxonProps>,
    references: &HashMap<String, ReferenceProps>,
    out: &mut W,
) -> Result<usize, ExportError> {
    let mut writer = csv::Writer::from_writer(out);
    writer.write_record(FROZEN_COLUMNS)?;
    let mut count = 0;
    for triplet in triplets {
        let compound_qid = triplet.compound_qid.as_str();
        let taxon_qid = triplet.taxon_qid.as_str();
        let reference_qid = triplet.reference_qid.as_str();

        let compound = compounds.get(compound_qid);
        let taxon = taxa.get(taxon_qid);
        let reference = references.get(reference_qid);

        let row = ExportRow {
            compound_qid,
            compound_inchikey: triplet.compound_inchikey.as_str(),
            compound_smiles_can: compound.and_then(|c| c.smiles_can.as_deref()).unwrap_or(""),
            compound_smiles_iso: compound.and_then(|c| c.smiles_iso.as_deref()).unwrap_or(""),
            compound_cid: compound.and_then(|c| c.cid.as_deref()).unwrap_or(""),
            taxon_qid,
            taxon_name: taxon.and_then(|t| t.name.as_deref()).unwrap_or(""),
            taxon_ncbi: taxon.and_then(|t| t.ncbi.as_deref()).unwrap_or(""),
            taxon_ott: taxon.and_then(|t| t.ott.as_deref()).unwrap_or(""),
            taxon_gbif: taxon.and_then(|t| t.gbif.as_deref()).unwrap_or(""),
            taxon_parent: taxon.and_then(|t| t.parent.as_deref()).unwrap_or(""),
            taxon_rank: taxon.and_then(|t| t.rank.as_deref()).unwrap_or(""),
            taxon_rank_label: taxon.and_then(|t| t.rank_label.as_deref()).unwrap_or(""),
            reference_qid,
            reference_title: reference.and_then(|r| r.title.as_deref()).unwrap_or(""),
            reference_doi: reference.and_then(|r| r.doi.as_deref()).unwrap_or(""),
            reference_pmid: reference.and_then(|r| r.pmid.as_deref()).unwrap_or(""),
            reference_pmcid: reference.and_then(|r| r.pmcid.as_deref()).unwrap_or(""),
            reference_date: reference.and_then(|r| r.date.as_deref()).unwrap_or(""),
            reference_journal: reference.and_then(|r| r.journal.as_deref()).unwrap_or(""),
            reference_journal_label: reference
                .and_then(|r| r.journal_label.as_deref())
                .unwrap_or(""),
        };
        let record = [
            row.compound_qid,
            row.compound_inchikey,
            row.compound_smiles_can,
            row.compound_smiles_iso,
            row.compound_cid,
            row.taxon_qid,
            row.taxon_name,
            row.taxon_ncbi,
            row.taxon_ott,
            row.taxon_gbif,
            row.taxon_parent,
            row.taxon_rank,
            row.taxon_rank_label,
            row.reference_qid,
            row.reference_title,
            row.reference_doi,
            row.reference_pmid,
            row.reference_pmcid,
            row.reference_date,
            row.reference_journal,
            row.reference_journal_label,
        ];
        writer.write_record(record)?;
        count += 1;
    }
    writer.flush()?;
    Ok(count)
}
