// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `PubChem`'s compact form of the trees: every level is an object keyed by
//! name, and compounds are keyed by `InChIKey`.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use super::index::non_empty;
use super::nodes::{
    ChemicalNode, ClassMember, CompoundNode, Pathway, Reference, TaxonNode, TreeNode,
};

/// One level: its QID when it has one, and its children by name. A key that
/// two children share keeps the later child.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Branch<C> {
    #[serde(rename = "QID", skip_serializing_if = "Option::is_none")]
    qid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<BTreeMap<String, C>>,
}

impl<C> Branch<C> {
    /// A branch over `nodes`, which has no `children` key when there are none.
    fn new<N: TreeNode>(qid: Option<String>, nodes: &[N], convert: impl Fn(&N) -> C) -> Self {
        let children = (!nodes.is_empty()).then(|| {
            nodes
                .iter()
                .map(|n| (n.name().to_string(), convert(n)))
                .collect()
        });
        Self { qid, children }
    }

    fn insert(&mut self, key: String, child: C) {
        self.children.get_or_insert_default().insert(key, child);
    }
}

/// A compound keyed by one of its `InChIKey`s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactCompound {
    #[serde(rename = "Name", skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "QID", skip_serializing_if = "Option::is_none")]
    qid: Option<String>,
}

/// The compound once per `InChIKey`. "Unknown" is the Python generator's
/// placeholder for a missing name, so it is not written as one.
fn by_inchikey(compound: &CompoundNode) -> impl Iterator<Item = (String, CompactCompound)> + '_ {
    compound.descriptors.inchikey.iter().map(|inchikey| {
        let leaf = CompactCompound {
            name: non_empty(&compound.name).filter(|name| name != "Unknown"),
            qid: non_empty(&compound.identifiers.qid),
        };
        (inchikey.clone(), leaf)
    })
}

/// A biological-tree entry: a taxon, or the organism record its NCBI id keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub(super) enum TaxonEntry {
    Taxon(Branch<Self>),
    Organism(Organism),
}

/// A taxon's compounds, under the taxon's NCBI id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Organism {
    organism_name: [String; 1],
    compounds: BTreeMap<String, OrganismCompound>,
}

/// A compound of an organism, merged across every compound with its `InChIKey`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(super) struct OrganismCompound {
    #[serde(rename = "Name", skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "QID", skip_serializing_if = "Option::is_none")]
    qid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    references: Option<Vec<CompactReference>>,
    #[serde(skip)]
    cited: HashSet<(Option<String>, Option<String>)>,
}

/// A reference without its QID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompactReference {
    #[serde(rename = "DOI", skip_serializing_if = "Option::is_none")]
    doi: Option<String>,
    #[serde(rename = "PMID", skip_serializing_if = "Option::is_none")]
    pmid: Option<String>,
}

impl OrganismCompound {
    /// Take the first name and QID seen, and every reference not cited yet.
    fn merge(&mut self, compound: &CompoundNode, references: &[Reference]) {
        self.name = self.name.take().or_else(|| non_empty(&compound.name));
        self.qid = self
            .qid
            .take()
            .or_else(|| non_empty(&compound.identifiers.qid));
        if references.is_empty() {
            return;
        }
        let list = self.references.get_or_insert_default();
        for r in references {
            let key = (r.doi.clone(), r.pmid.clone());
            if self.cited.insert(key) && (r.doi.is_some() || r.pmid.is_some()) {
                list.push(CompactReference {
                    doi: r.doi.clone(),
                    pmid: r.pmid.clone(),
                });
            }
        }
    }
}

/// The biological tree in compact form.
pub(super) fn biological(tree: &[TaxonNode]) -> Branch<TaxonEntry> {
    Branch::new(None, tree, taxon_entry)
}

fn taxon_entry(node: &TaxonNode) -> TaxonEntry {
    let mut branch = Branch::new(
        non_empty(&node.identifiers.qid),
        &node.children,
        taxon_entry,
    );
    if let (Some(ncbi), Some(organism)) = (&node.identifiers.ncbi_taxid, organism(node)) {
        branch.insert(ncbi.clone(), TaxonEntry::Organism(organism));
    }
    TaxonEntry::Taxon(branch)
}

/// The taxon's compounds by `InChIKey`, or `None` when none has one.
fn organism(node: &TaxonNode) -> Option<Organism> {
    let mut compounds: BTreeMap<String, OrganismCompound> = BTreeMap::new();
    for c in &node.compounds {
        for inchikey in &c.compound.descriptors.inchikey {
            compounds
                .entry(inchikey.clone())
                .or_default()
                .merge(&c.compound, &c.references);
        }
    }
    (!compounds.is_empty()).then(|| Organism {
        organism_name: [node.name.clone()],
        compounds,
    })
}

/// A chemical-tree entry: a class, or a compound under one of its `InChIKey`s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub(super) enum ChemicalEntry {
    Class(Branch<Self>),
    Compound(CompactCompound),
}

/// The chemical tree in compact form.
pub(super) fn chemical(tree: &[ChemicalNode]) -> Branch<ChemicalEntry> {
    Branch::new(None, tree, chemical_entry)
}

fn chemical_entry(node: &ChemicalNode) -> ChemicalEntry {
    let mut branch = Branch::new(
        non_empty(&node.compound.identifiers.qid),
        &node.children,
        chemical_entry,
    );
    for (inchikey, compound) in by_inchikey(&node.compound) {
        branch.insert(inchikey, ChemicalEntry::Compound(compound));
    }
    ChemicalEntry::Class(branch)
}

/// One superclass in compact form: class → `InChIKey` → compound.
type ClassEntries = BTreeMap<String, BTreeMap<String, CompactCompound>>;

/// The `NPClassifier` tree in compact form. Compounds with no class have no
/// key to sit under, so a superclass or pathway left without a class goes.
pub(super) fn npclassifier(tree: &[Pathway]) -> Branch<Branch<ClassEntries>> {
    let pathways = tree
        .iter()
        .filter_map(|pathway| {
            let superclasses: BTreeMap<String, ClassEntries> = pathway
                .children
                .iter()
                .filter_map(|superclass| {
                    let classes: ClassEntries = superclass
                        .children
                        .iter()
                        .filter_map(|member| match member {
                            ClassMember::Class(class) => Some((
                                class.name.clone(),
                                class.children.iter().flat_map(by_inchikey).collect(),
                            )),
                            ClassMember::Unclassified(_) => None,
                        })
                        .collect();
                    (!classes.is_empty()).then(|| (superclass.name.clone(), classes))
                })
                .collect();
            (!superclasses.is_empty()).then(|| {
                let branch = Branch {
                    qid: None,
                    children: Some(superclasses),
                };
                (pathway.name.clone(), branch)
            })
        })
        .collect();
    Branch {
        qid: None,
        children: Some(pathways),
    }
}
