// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The nodes of the three full trees, typed so that each tree can hold only
//! what its document carries, and serialized under the document's key names.

use std::collections::BTreeMap;

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

/// What every node of every tree answers: the name siblings are sorted by,
/// and how many nodes it stands for in the document's statistics.
pub(super) trait TreeNode {
    /// The `Name` the node is written and sorted under.
    fn name(&self) -> &str;
    /// This node plus every node under its `Children` (not its `Compounds`).
    fn node_count(&self) -> usize;
}

/// Siblings in `Name` order; the sort is stable, so equal names keep the
/// order they were built in.
pub(super) fn sort_by_name<T: TreeNode>(mut nodes: Vec<T>) -> Vec<T> {
    nodes.sort_by(|a, b| a.name().cmp(b.name()));
    nodes
}

/// Every node in a forest, children included.
pub(super) fn count_nodes<T: TreeNode>(nodes: &[T]) -> usize {
    nodes.iter().map(TreeNode::node_count).sum()
}

/// A compound's `Identifiers`: the Wikidata QID alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Qid {
    #[serde(rename = "QID")]
    pub(super) qid: String,
}

/// A compound's structure descriptors. Each list is written as a bare string
/// when it holds one value and as an array otherwise; empty ones are left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(super) struct Descriptors {
    #[serde(
        rename = "InChIKey",
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "one_or_many"
    )]
    pub(super) inchikey: Vec<String>,
    #[serde(
        rename = "SMILES",
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "one_or_many"
    )]
    pub(super) smiles: Vec<String>,
    #[serde(rename = "SMARTS", skip_serializing_if = "Option::is_none")]
    pub(super) smarts: Option<String>,
    #[serde(rename = "CXSMILES", skip_serializing_if = "Option::is_none")]
    pub(super) cxsmiles: Option<String>,
}

impl Descriptors {
    /// No descriptor at all, which leaves the `Descriptors` key out.
    pub(super) const fn is_empty(&self) -> bool {
        self.inchikey.is_empty()
            && self.smiles.is_empty()
            && self.smarts.is_none()
            && self.cxsmiles.is_none()
    }
}

/// `lotus_pubchem_tree.py` writes a single value as a string, so a reader of
/// the common case never meets an array.
fn one_or_many<S: Serializer>(values: &[String], serializer: S) -> Result<S::Ok, S::Error> {
    match values {
        [one] => serializer.serialize_str(one),
        many => many.serialize(serializer),
    }
}

/// A compound: a chemical-tree node without children, an `NPClassifier`
/// leaf, and the core of every compound a taxon lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CompoundNode {
    #[serde(rename = "Name")]
    pub(super) name: String,
    #[serde(rename = "Identifiers")]
    pub(super) identifiers: Qid,
    #[serde(rename = "Descriptors", skip_serializing_if = "Descriptors::is_empty")]
    pub(super) descriptors: Descriptors,
}

impl TreeNode for CompoundNode {
    fn name(&self) -> &str {
        &self.name
    }

    fn node_count(&self) -> usize {
        1
    }
}

/// One article supporting a compound-taxon pair.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(super) struct Reference {
    #[serde(rename = "QID")]
    pub(super) qid: String,
    #[serde(rename = "DOI", skip_serializing_if = "Option::is_none")]
    pub(super) doi: Option<String>,
    #[serde(rename = "PMID", skip_serializing_if = "Option::is_none")]
    pub(super) pmid: Option<String>,
}

/// A compound as one taxon lists it, with the references for that pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct TaxonCompound {
    #[serde(flatten)]
    pub(super) compound: CompoundNode,
    #[serde(rename = "References", skip_serializing_if = "Vec::is_empty")]
    pub(super) references: Vec<Reference>,
}

/// A taxon's `Identifiers`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct TaxonIdentifiers {
    #[serde(rename = "QID")]
    pub(super) qid: String,
    #[serde(rename = "NCBI_TaxID", skip_serializing_if = "Option::is_none")]
    pub(super) ncbi_taxid: Option<String>,
}

/// A node of the biological tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct TaxonNode {
    #[serde(rename = "Name")]
    pub(super) name: String,
    #[serde(rename = "Identifiers")]
    pub(super) identifiers: TaxonIdentifiers,
    #[serde(rename = "Compounds", skip_serializing_if = "Vec::is_empty")]
    pub(super) compounds: Vec<TaxonCompound>,
    #[serde(rename = "Children", skip_serializing_if = "Vec::is_empty")]
    pub(super) children: Vec<Self>,
}

impl TreeNode for TaxonNode {
    fn name(&self) -> &str {
        &self.name
    }

    fn node_count(&self) -> usize {
        1 + count_nodes(&self.children)
    }
}

/// A node of the Wikidata chemical tree: a class or a compound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct ChemicalNode {
    #[serde(flatten)]
    pub(super) compound: CompoundNode,
    #[serde(rename = "Children", skip_serializing_if = "Vec::is_empty")]
    pub(super) children: Vec<Self>,
}

impl TreeNode for ChemicalNode {
    fn name(&self) -> &str {
        &self.compound.name
    }

    fn node_count(&self) -> usize {
        1 + count_nodes(&self.children)
    }
}

/// The three `NPClassifier` levels, which differ only in the identifier key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Level {
    Pathway,
    Superclass,
    Class,
}

impl Level {
    const fn identifier_key(self) -> &'static str {
        match self {
            Self::Pathway => "NPClassifier_Pathway",
            Self::Superclass => "NPClassifier_Superclass",
            Self::Class => "NPClassifier_Class",
        }
    }
}

/// An `NPClassifier` level node. It has no Wikidata item, so its name stands
/// in for the QID, and it is only ever built with children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Classification<C> {
    pub(super) level: Level,
    pub(super) name: String,
    pub(super) children: Vec<C>,
}

/// An `NPClassifier` pathway, the root of the tree.
pub(super) type Pathway = Classification<Superclass>;
/// An `NPClassifier` superclass: classes and unclassified compounds.
pub(super) type Superclass = Classification<ClassMember>;
/// An `NPClassifier` class: compounds only.
pub(super) type Class = Classification<CompoundNode>;

impl<C: Serialize> Serialize for Classification<C> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let identifiers = BTreeMap::from([
            ("QID", &self.name),
            (self.level.identifier_key(), &self.name),
        ]);
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("Name", &self.name)?;
        map.serialize_entry("Identifiers", &identifiers)?;
        map.serialize_entry("Children", &self.children)?;
        map.end()
    }
}

impl<C: TreeNode> TreeNode for Classification<C> {
    fn name(&self) -> &str {
        &self.name
    }

    fn node_count(&self) -> usize {
        1 + count_nodes(&self.children)
    }
}

/// What a superclass holds: a class, or a compound no class was given for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub(super) enum ClassMember {
    Class(Class),
    Unclassified(CompoundNode),
}

impl TreeNode for ClassMember {
    fn name(&self) -> &str {
        match self {
            Self::Class(class) => class.name(),
            Self::Unclassified(compound) => compound.name(),
        }
    }

    fn node_count(&self) -> usize {
        match self {
            Self::Class(class) => class.node_count(),
            Self::Unclassified(compound) => compound.node_count(),
        }
    }
}
