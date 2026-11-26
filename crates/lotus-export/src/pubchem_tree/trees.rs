// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The two trees read from Wikidata hierarchies: taxa under Biota (`P171`) with
//! their compounds, and compounds under chemical compound (`Q11173`) via `P279`.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::fetch::PubChemData;
use super::index::{Hierarchy, Index, qid};
use super::nodes::{ChemicalNode, TaxonCompound, TaxonIdentifiers, TaxonNode, sort_by_name};

/// Build the biological tree: every taxon with a compound, every ancestor of
/// one, and the compounds with an `InChIKey` as leaves.
pub(super) fn biological_tree(data: &PubChemData, index: &Index) -> Vec<TaxonNode> {
    let mut taxon_compounds: HashMap<String, BTreeSet<String>> = HashMap::new();
    for row in &data.compound_taxon {
        let compound = qid(row, "compound");
        let taxon = qid(row, "taxon");
        if !compound.is_empty() && !taxon.is_empty() {
            taxon_compounds.entry(taxon).or_default().insert(compound);
        }
    }
    let taxa = Hierarchy::new(&data.taxon_parent, "taxon", "taxon_parent");

    let mut relevant: HashSet<&str> = HashSet::new();
    let mut stack: Vec<&str> = taxon_compounds
        .keys()
        .map(String::as_str)
        .filter(|t| relevant.insert(*t))
        .collect();
    while let Some(taxon) = stack.pop() {
        for parent in taxa.parents(taxon) {
            if relevant.insert(parent.as_str()) {
                stack.push(parent.as_str());
            }
        }
    }

    // Sorted by QID, as `lotus_pubchem_tree.py` does, so which root claims a
    // shared descendant first does not depend on hash order.
    let mut roots: Vec<&str> = relevant
        .iter()
        .copied()
        .filter(|t| {
            !taxa
                .parents(t)
                .iter()
                .any(|p| relevant.contains(p.as_str()))
        })
        .collect();
    roots.sort_unstable();

    let mut builder = BiologicalTree {
        index,
        taxa: &taxa,
        taxon_compounds: &taxon_compounds,
        relevant: &relevant,
        visited: HashSet::new(),
    };
    let mut nodes: Vec<TaxonNode> = roots.into_iter().filter_map(|r| builder.node(r)).collect();
    // Deterministic cycle breaking: pick lowest unvisited QID in any remaining
    // cyclic component so cyclic P171 edges do not drop entire subtrees.
    while let Some(next) = relevant
        .iter()
        .copied()
        .filter(|t| !builder.visited.contains(t))
        .min()
    {
        if let Some(node) = builder.node(next) {
            nodes.push(node);
        }
    }
    sort_by_name(nodes)
}

/// What one biological-tree walk reads, and the taxa it has placed.
struct BiologicalTree<'a> {
    index: &'a Index,
    taxa: &'a Hierarchy,
    taxon_compounds: &'a HashMap<String, BTreeSet<String>>,
    relevant: &'a HashSet<&'a str>,
    visited: HashSet<&'a str>,
}

impl<'a> BiologicalTree<'a> {
    /// The taxon's node, or `None` when it was placed already or has neither
    /// a compound nor a descendant with one.
    fn node(&mut self, taxon: &'a str) -> Option<TaxonNode> {
        if !self.visited.insert(taxon) {
            return None;
        }
        let index = self.index;
        let compounds: Vec<TaxonCompound> = self
            .taxon_compounds
            .get(taxon)
            .into_iter()
            .flatten()
            .filter(|c| index.has_taxa(c))
            .map(|c| TaxonCompound {
                compound: index.compound_node(c),
                references: index.references(c, taxon),
            })
            .collect();
        let (taxa, relevant) = (self.taxa, self.relevant);
        let children: Vec<TaxonNode> = taxa
            .children(taxon)
            .iter()
            .filter(|c| relevant.contains(c.as_str()))
            .filter_map(|c| self.node(c))
            .collect();
        if compounds.is_empty() && children.is_empty() {
            return None;
        }
        let props = index.taxa.get(taxon);
        Some(TaxonNode {
            name: props
                .and_then(|t| t.name.clone())
                .unwrap_or_else(|| taxon.to_string()),
            identifiers: TaxonIdentifiers {
                qid: taxon.to_string(),
                ncbi_taxid: props.and_then(|t| t.ncbi.clone()),
            },
            compounds,
            children: sort_by_name(children),
        })
    }
}

/// Build the chemical tree: every class with a compound that has taxa below it.
pub(super) fn chemical_tree(data: &PubChemData, index: &Index) -> Vec<ChemicalNode> {
    let classes = Hierarchy::new(&data.compound_parent, "compound", "compound_parent");
    let mut visited = HashSet::new();
    let mut nodes: Vec<ChemicalNode> = classes
        .roots()
        .into_iter()
        .filter_map(|root| chemical_node(root, &classes, index, &mut visited))
        .collect();
    // Break any cyclic P279 component deterministically by sorted QID.
    for qid in classes.all_qids() {
        if !visited.contains(qid.as_str())
            && let Some(node) = chemical_node(qid, &classes, index, &mut visited)
        {
            nodes.push(node);
        }
    }
    sort_by_name(nodes)
}

/// The compound's node, or `None` when it was placed already or neither it
/// nor anything below it has taxa.
fn chemical_node<'a>(
    compound: &'a str,
    classes: &'a Hierarchy,
    index: &Index,
    visited: &mut HashSet<&'a str>,
) -> Option<ChemicalNode> {
    if !visited.insert(compound) {
        return None;
    }
    let children: Vec<ChemicalNode> = classes
        .children(compound)
        .iter()
        .filter_map(|child| chemical_node(child, classes, index, visited))
        .collect();
    if children.is_empty() && !index.has_taxa(compound) {
        return None;
    }
    Some(ChemicalNode {
        compound: index.compound_node(compound),
        children: sort_by_name(children),
    })
}
