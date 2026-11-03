// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The `NPClassifier` tree: pathway → superclass → class → compound, read from
//! the classifier's cache and matched to Wikidata compounds by SMILES.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Bound;

use super::index::{Index, PubChemData, qid};
use super::nodes::{
    ClassMember, Classification, CompoundNode, Descriptors, Level, Pathway, Qid, Superclass,
    sort_by_name,
};
use crate::export::pipeline::Row;

/// The class name the Python generator files a classless compound under. A
/// class of that name is read the same way, and the name decides where those
/// compounds sit among the classes before the sort by name.
const UNCLASSIFIED: &str = "_unclassified";

/// One superclass's SMILES, by class and without one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SuperclassMembers {
    pub(super) classes: BTreeMap<String, BTreeSet<String>>,
    pub(super) unclassified: BTreeSet<String>,
}

/// pathway → superclass → members, as the cache classifies the SMILES.
pub(super) type Classified = BTreeMap<String, BTreeMap<String, SuperclassMembers>>;

/// Classify every error-free row with a SMILES. The i-th superclass of a row
/// takes its i-th class; a superclass with no class at that position keeps
/// the SMILES unclassified.
pub(super) fn classify(rows: &[Row]) -> Classified {
    let mut classified = Classified::new();
    for row in rows {
        let field = |column: &str| row.get(column).map_or("", String::as_str);
        let smiles = field("smiles");
        if !field("error").is_empty() || smiles.is_empty() {
            continue;
        }
        let superclasses = levels(field("superclass"));
        let classes = levels(field("class"));
        for pathway in levels(field("pathway")) {
            let by_superclass = classified.entry(pathway.to_string()).or_default();
            for (i, superclass) in superclasses.iter().enumerate() {
                let members = by_superclass.entry((*superclass).to_string()).or_default();
                let bucket = match classes.get(i) {
                    Some(class) if *class != UNCLASSIFIED => {
                        members.classes.entry((*class).to_string()).or_default()
                    }
                    _ => &mut members.unclassified,
                };
                bucket.insert(smiles.to_string());
            }
        }
    }
    classified
}

/// The non-empty values of one ` $ `-separated multi-value cell.
fn levels(raw: &str) -> Vec<&str> {
    raw.split(" $ ")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// The Wikidata compound a SMILES belongs to, with every `InChIKey` it has.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SmilesEntry {
    qid: String,
    inchikeys: BTreeSet<String>,
}

/// SMILES → compound, for the compounds that have taxa.
#[derive(Debug, Clone, Default)]
pub(super) struct SmilesIndex(HashMap<String, SmilesEntry>);

impl SmilesIndex {
    /// A SMILES shared by several compounds names the first in row order and
    /// carries the `InChIKey`s of them all.
    pub(super) fn new(data: &PubChemData, index: &Index) -> Self {
        let mut entries: HashMap<String, SmilesEntry> = HashMap::new();
        for row in &data.compound_taxon {
            let compound = qid(row, "compound");
            let Some(props) = index.compounds.get(&compound).filter(|p| p.has_taxa()) else {
                continue;
            };
            for smiles in props.smiles() {
                entries
                    .entry(smiles.clone())
                    .or_insert_with(|| SmilesEntry {
                        qid: compound.clone(),
                        inchikeys: BTreeSet::new(),
                    })
                    .inchikeys
                    .extend(props.inchikeys.iter().cloned());
            }
        }
        Self(entries)
    }

    /// The compound leaf for one classified SMILES, under its label.
    fn leaf(&self, smiles: &str, index: &Index) -> Option<CompoundNode> {
        let entry = self.0.get(smiles)?;
        Some(CompoundNode {
            name: index
                .compounds
                .get(&entry.qid)
                .and_then(|p| p.label.clone())
                .unwrap_or_else(|| smiles.to_string()),
            identifiers: Qid {
                qid: entry.qid.clone(),
            },
            descriptors: Descriptors {
                inchikey: entry.inchikeys.iter().cloned().collect(),
                smiles: vec![smiles.to_string()],
                ..Descriptors::default()
            },
        })
    }
}

/// Build the `NPClassifier` tree, keeping only levels with a known compound.
pub(super) fn npclassifier_tree(
    classified: &Classified,
    smiles: &SmilesIndex,
    index: &Index,
) -> Vec<Pathway> {
    classified
        .iter()
        .filter_map(|(pathway, superclasses)| {
            let children: Vec<Superclass> = superclasses
                .iter()
                .filter_map(|(name, members)| superclass(name, members, smiles, index))
                .collect();
            (!children.is_empty()).then(|| Classification {
                level: Level::Pathway,
                name: pathway.clone(),
                children,
            })
        })
        .collect()
}

/// One superclass node: its classes with a known compound, and its known
/// unclassified compounds as bare leaves.
fn superclass(
    name: &str,
    members: &SuperclassMembers,
    smiles: &SmilesIndex,
    index: &Index,
) -> Option<Superclass> {
    let leaves = |set: &BTreeSet<String>| -> Vec<CompoundNode> {
        set.iter().filter_map(|s| smiles.leaf(s, index)).collect()
    };
    let class = |(class, set): (&String, &BTreeSet<String>)| {
        let children = leaves(set);
        (!children.is_empty()).then(|| {
            ClassMember::Class(Classification {
                level: Level::Class,
                name: class.clone(),
                children,
            })
        })
    };
    let before = (Bound::Unbounded, Bound::Excluded(UNCLASSIFIED));
    let after = (Bound::Excluded(UNCLASSIFIED), Bound::Unbounded);
    let children: Vec<ClassMember> = members
        .classes
        .range::<str, _>(before)
        .filter_map(class)
        .chain(
            leaves(&members.unclassified)
                .into_iter()
                .map(ClassMember::Unclassified),
        )
        .chain(members.classes.range::<str, _>(after).filter_map(class))
        .collect();
    (!children.is_empty()).then(|| Classification {
        level: Level::Superclass,
        name: name.to_string(),
        children: sort_by_name(children),
    })
}
