// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Per-QID indexes and parent-child hierarchies for the `PubChem` trees.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::fetch::PubChemData;
use super::nodes::{CompoundNode, Descriptors, Qid, Reference};
pub(super) use crate::pipeline::{Row, cell, qid};

/// An empty cell means "no value": the documents leave the key out.
pub(super) fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

/// One compound's properties. The last row of a table wins, except that a
/// multilingual (`mul`) label is never overwritten by an English (`en`) one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CompoundProps {
    pub(super) smiles_can: Option<String>,
    pub(super) smiles_iso: Option<String>,
    pub(super) smarts: Option<String>,
    pub(super) cxsmiles: Option<String>,
    pub(super) label: Option<String>,
    pub(super) label_is_mul: bool,
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
        let columns: [Column<'_>; 5] = [
            (&data.compound_smiles_can, "compound_smiles_can", |p, v| {
                if let Some(s) = non_empty(v) {
                    p.smiles_can = Some(s);
                }
            }),
            (&data.compound_smiles_iso, "compound_smiles_iso", |p, v| {
                if let Some(s) = non_empty(v) {
                    p.smiles_iso = Some(s);
                }
            }),
            (&data.compound_smarts, "compound_smarts", |p, v| {
                if let Some(s) = non_empty(v) {
                    p.smarts = Some(s);
                }
            }),
            (&data.compound_cxsmiles, "compound_cxsmiles", |p, v| {
                if let Some(s) = non_empty(v) {
                    p.cxsmiles = Some(s);
                }
            }),
            (&data.compound_taxon, "compound_inchikey", |p, v| {
                p.inchikeys.extend(non_empty(v));
            }),
        ];
        for (rows, column, set) in columns {
            fill(&mut compounds, rows, "compound", column, set);
        }
        for row in &data.compound_label {
            let entry = compounds.entry(qid(row, "compound")).or_default();
            let is_mul = cell(row, "lang") == "mul";
            if (is_mul || !entry.label_is_mul)
                && let Some(label) = non_empty(cell(row, "compound_label"))
            {
                entry.label = Some(label);
                entry.label_is_mul = is_mul;
            }
        }

        let mut taxa: HashMap<String, TaxonProps> = HashMap::new();
        fill(
            &mut taxa,
            &data.taxon_name,
            "taxon",
            "taxon_name",
            |t, v| {
                if let Some(name) = non_empty(v) {
                    t.name = Some(name);
                }
            },
        );
        fill(
            &mut taxa,
            &data.taxon_ncbi,
            "taxon",
            "taxon_ncbi",
            |t, v| {
                if let Some(ncbi) = non_empty(v) {
                    t.ncbi = Some(ncbi);
                }
            },
        );

        let mut references: HashMap<String, HashMap<String, BTreeMap<String, ReferenceProps>>> =
            HashMap::new();
        let mut cite = |rows: &[Row], column: &str, set: fn(&mut ReferenceProps, String)| {
            for row in rows {
                if let Some(val) = non_empty(cell(row, column)) {
                    let reference = references
                        .entry(qid(row, "compound"))
                        .or_default()
                        .entry(qid(row, "taxon"))
                        .or_default()
                        .entry(qid(row, "reference"))
                        .or_default();
                    set(reference, val);
                }
            }
        };
        cite(&data.reference_doi, "doi", |r, doi| r.doi = Some(doi));
        cite(&data.reference_pmid, "pmid", |r, pmid| r.pmid = Some(pmid));

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
            descriptors: props.map_or_else(Descriptors::default, CompoundProps::descriptors),
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
    parents_of: HashMap<String, Vec<String>>,
    children_of: HashMap<String, Vec<String>>,
}

impl Hierarchy {
    pub(super) fn new(rows: &[Row], child_column: &str, parent_column: &str) -> Self {
        let mut hierarchy = Self::default();
        for row in rows {
            let child = qid(row, child_column);
            let parent = qid(row, parent_column);
            if child.is_empty() || parent.is_empty() {
                continue;
            }
            hierarchy
                .parents_of
                .entry(child.clone())
                .or_default()
                .push(parent.clone());
            hierarchy.children_of.entry(parent).or_default().push(child);
        }
        for list in hierarchy
            .parents_of
            .values_mut()
            .chain(hierarchy.children_of.values_mut())
        {
            list.sort_unstable();
            list.dedup();
        }
        hierarchy
    }

    /// All distinct QIDs appearing as parents or children, sorted by QID.
    pub(super) fn all_qids(&self) -> Vec<&String> {
        let qids: BTreeSet<&String> = self
            .children_of
            .keys()
            .chain(self.parents_of.keys())
            .collect();
        qids.into_iter().collect()
    }

    /// The parents in row order, a parent listed once per row.
    pub(super) fn parents(&self, qid: &str) -> &[String] {
        self.parents_of.get(qid).map_or(&[], Vec::as_slice)
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
            .filter(|qid| !self.parents_of.contains_key(*qid))
            .collect();
        roots.sort_unstable();
        roots
    }
}
