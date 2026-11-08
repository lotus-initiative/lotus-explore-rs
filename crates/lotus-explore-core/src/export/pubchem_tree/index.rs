// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The twelve Wikidata tables and the per-QID indexes the trees are built from.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use lotus_query::{
    COMPOUND_CXSMILES_QUERY, COMPOUND_INCHIKEY_TAXON_QUERY, COMPOUND_LABEL_QUERY,
    COMPOUND_PARENT_QUERY, COMPOUND_SMARTS_QUERY, COMPOUND_SMILES_CAN_ALL_QUERY,
    COMPOUND_SMILES_ISO_ALL_QUERY, REFERENCE_DOI_COMPOUND_TAXON_QUERY,
    REFERENCE_PMID_COMPOUND_TAXON_QUERY, TAXON_NAME_QUERY, TAXON_NCBI_QUERY, TAXON_PARENT_QUERY,
};
use lotus_search::{Endpoint, Http};

use super::nodes::{CompoundNode, Descriptors, Qid, Reference};
pub(super) use crate::export::pipeline::{ExportError, Row, cell, fetch_csv_rows, qid};

/// All 12 tables fetched from Wikidata, in Python's `LOTUSData` order.
#[derive(Debug, Clone, Default)]
pub(super) struct PubChemData {
    pub(super) compound_taxon: Vec<Row>,
    pub(super) taxon_ncbi: Vec<Row>,
    pub(super) taxon_name: Vec<Row>,
    pub(super) taxon_parent: Vec<Row>,
    pub(super) compound_smiles_can: Vec<Row>,
    pub(super) compound_smiles_iso: Vec<Row>,
    pub(super) compound_smarts: Vec<Row>,
    pub(super) compound_cxsmiles: Vec<Row>,
    pub(super) compound_label: Vec<Row>,
    pub(super) compound_parent: Vec<Row>,
    pub(super) reference_doi: Vec<Row>,
    pub(super) reference_pmid: Vec<Row>,
}

/// Fetch all 12 Wikidata tables through the injected transport.
pub(super) async fn fetch_all_pubchem_data<H: Http>(
    http: &H,
    endpoint: &Endpoint,
) -> Result<PubChemData, ExportError> {
    let fetch = |query: &'static str, name: &'static str| async move {
        fetch_csv_rows(http, endpoint, query, name).await
    };
    Ok(PubChemData {
        compound_taxon: fetch(COMPOUND_INCHIKEY_TAXON_QUERY, "compound_taxon").await?,
        taxon_ncbi: fetch(TAXON_NCBI_QUERY, "taxon_ncbi").await?,
        taxon_name: fetch(TAXON_NAME_QUERY, "taxon_name").await?,
        taxon_parent: fetch(TAXON_PARENT_QUERY, "taxon_parent").await?,
        compound_smiles_can: fetch(COMPOUND_SMILES_CAN_ALL_QUERY, "compound_smiles_can").await?,
        compound_smiles_iso: fetch(COMPOUND_SMILES_ISO_ALL_QUERY, "compound_smiles_iso").await?,
        compound_smarts: fetch(COMPOUND_SMARTS_QUERY, "compound_smarts").await?,
        compound_cxsmiles: fetch(COMPOUND_CXSMILES_QUERY, "compound_cxsmiles").await?,
        compound_label: fetch(COMPOUND_LABEL_QUERY, "compound_label").await?,
        compound_parent: fetch(COMPOUND_PARENT_QUERY, "compound_parent").await?,
        reference_doi: fetch(REFERENCE_DOI_COMPOUND_TAXON_QUERY, "reference_doi").await?,
        reference_pmid: fetch(REFERENCE_PMID_COMPOUND_TAXON_QUERY, "reference_pmid").await?,
    })
}

/// An empty cell means "no value": the documents leave the key out.
pub(super) fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

/// One compound's properties. The last row of a table wins, as in the Python.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CompoundProps {
    pub(super) smiles_can: Option<String>,
    pub(super) smiles_iso: Option<String>,
    pub(super) smarts: Option<String>,
    pub(super) cxsmiles: Option<String>,
    pub(super) label: Option<String>,
    /// Every `InChIKey` a compound-taxon row gives the compound. A compound
    /// "has taxa" exactly when this is not empty.
    pub(super) inchikeys: BTreeSet<String>,
}

impl CompoundProps {
    /// Whether the compound has an `InChIKey` and a taxon, which is what puts
    /// it in the biological tree and keeps it in the chemical one.
    pub(super) fn has_taxa(&self) -> bool {
        !self.inchikeys.is_empty()
    }

    /// The structures `NPClassifier` rows can be matched on.
    pub(super) fn smiles(&self) -> impl Iterator<Item = &String> {
        [&self.smiles_iso, &self.smiles_can].into_iter().flatten()
    }

    fn descriptors(&self) -> Descriptors {
        let smiles: BTreeSet<&String> = self.smiles().collect();
        Descriptors {
            inchikey: self.inchikeys.iter().cloned().collect(),
            smiles: smiles.into_iter().cloned().collect(),
            smarts: self.smarts.clone(),
            cxsmiles: self.cxsmiles.clone(),
        }
    }
}

/// One taxon's properties.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TaxonProps {
    pub(super) name: Option<String>,
    pub(super) ncbi: Option<String>,
}

/// One reference's identifiers; empty cells are `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ReferenceProps {
    doi: Option<String>,
    pmid: Option<String>,
}

/// Every table but the two hierarchies, indexed by QID.
#[derive(Debug, Clone, Default)]
pub(super) struct Index {
    pub(super) compounds: HashMap<String, CompoundProps>,
    pub(super) taxa: HashMap<String, TaxonProps>,
    /// `compound` → `taxon` → reference QID → identifiers. Ordered by QID so
    /// references citing the same DOI and PMID always list in the same order.
    references: HashMap<String, HashMap<String, BTreeMap<String, ReferenceProps>>>,
}

impl Index {
    pub(super) fn new(data: &PubChemData) -> Self {
        let mut compounds: HashMap<String, CompoundProps> = HashMap::new();
        let columns: [Column<'_>; 6] = [
            (&data.compound_smiles_can, "compound_smiles_can", |p, v| {
                p.smiles_can = Some(v.to_string());
            }),
            (&data.compound_smiles_iso, "compound_smiles_iso", |p, v| {
                p.smiles_iso = Some(v.to_string());
            }),
            (&data.compound_smarts, "compound_smarts", |p, v| {
                p.smarts = non_empty(v);
            }),
            (&data.compound_cxsmiles, "compound_cxsmiles", |p, v| {
                p.cxsmiles = non_empty(v);
            }),
            (&data.compound_label, "compound_label", |p, v| {
                p.label = Some(v.to_string());
            }),
            (&data.compound_taxon, "compound_inchikey", |p, v| {
                p.inchikeys.extend(non_empty(v));
            }),
        ];
        for (rows, column, set) in columns {
            fill(&mut compounds, rows, "compound", column, set);
        }

        let mut taxa: HashMap<String, TaxonProps> = HashMap::new();
        fill(
            &mut taxa,
            &data.taxon_name,
            "taxon",
            "taxon_name",
            |t, v| {
                t.name = Some(v.to_string());
            },
        );
        fill(
            &mut taxa,
            &data.taxon_ncbi,
            "taxon",
            "taxon_ncbi",
            |t, v| {
                t.ncbi = non_empty(v);
            },
        );

        let mut references: HashMap<String, HashMap<String, BTreeMap<String, ReferenceProps>>> =
            HashMap::new();
        let mut cite =
            |rows: &[Row], column: &str, set: fn(&mut ReferenceProps, Option<String>)| {
                for row in rows {
                    let reference = references
                        .entry(qid(row, "compound"))
                        .or_default()
                        .entry(qid(row, "taxon"))
                        .or_default()
                        .entry(qid(row, "reference"))
                        .or_default();
                    set(reference, non_empty(cell(row, column)));
                }
            };
        cite(&data.reference_doi, "doi", |r, doi| r.doi = doi);
        cite(&data.reference_pmid, "pmid", |r, pmid| r.pmid = pmid);

        Self {
            compounds,
            taxa,
            references,
        }
    }

    /// See [`CompoundProps::has_taxa`]; an unknown compound has none.
    pub(super) fn has_taxa(&self, compound: &str) -> bool {
        self.compounds
            .get(compound)
            .is_some_and(CompoundProps::has_taxa)
    }

    /// The compound under its label (its QID when it has none).
    pub(super) fn compound_node(&self, compound: &str) -> CompoundNode {
        let props = self.compounds.get(compound);
        CompoundNode {
            name: props
                .and_then(|p| p.label.clone())
                .unwrap_or_else(|| compound.to_string()),
            identifiers: Qid {
                qid: compound.to_string(),
            },
            descriptors: props.map(CompoundProps::descriptors).unwrap_or_default(),
        }
    }

    /// The references for one compound-taxon pair, by DOI then PMID.
    pub(super) fn references(&self, compound: &str, taxon: &str) -> Vec<Reference> {
        let mut list: Vec<Reference> = self
            .references
            .get(compound)
            .and_then(|by_taxon| by_taxon.get(taxon))
            .into_iter()
            .flatten()
            .map(|(qid, props)| Reference {
                qid: qid.clone(),
                doi: props.doi.clone(),
                pmid: props.pmid.clone(),
            })
            .collect();
        list.sort_by(|a, b| (&a.doi, &a.pmid).cmp(&(&b.doi, &b.pmid)));
        list
    }
}

/// A table, the column it gives a compound, and where that value goes.
type Column<'a> = (&'a [Row], &'static str, fn(&mut CompoundProps, &str));

/// Assign one column of every row to the properties of the row's key QID.
fn fill<P: Default>(
    index: &mut HashMap<String, P>,
    rows: &[Row],
    key: &str,
    column: &str,
    set: impl Fn(&mut P, &str),
) {
    for row in rows {
        set(index.entry(qid(row, key)).or_default(), cell(row, column));
    }
}

/// A `subclass of` / `parent taxon` hierarchy read from child-parent rows.
#[derive(Debug, Clone, Default)]
pub(super) struct Hierarchy {
    parent_of: HashMap<String, String>,
    children_of: HashMap<String, Vec<String>>,
}

impl Hierarchy {
    pub(super) fn new(rows: &[Row], child_column: &str, parent_column: &str) -> Self {
        let mut hierarchy = Self::default();
        for row in rows {
            let child = qid(row, child_column);
            let parent = qid(row, parent_column);
            hierarchy.parent_of.insert(child.clone(), parent.clone());
            hierarchy.children_of.entry(parent).or_default().push(child);
        }
        hierarchy
    }

    pub(super) fn parent(&self, qid: &str) -> Option<&String> {
        self.parent_of.get(qid)
    }

    /// The children in row order, a child listed once per row.
    pub(super) fn children(&self, qid: &str) -> &[String] {
        self.children_of.get(qid).map_or(&[], Vec::as_slice)
    }

    /// Every QID that has a child but no parent, by QID so that which root
    /// claims a shared descendant first does not depend on hash order.
    pub(super) fn roots(&self) -> Vec<&String> {
        let mut roots: Vec<&String> = self
            .children_of
            .keys()
            .filter(|qid| !self.parent_of.contains_key(*qid))
            .collect();
        roots.sort_unstable();
        roots
    }
}
