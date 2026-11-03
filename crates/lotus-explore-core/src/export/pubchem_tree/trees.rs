// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The two trees read from Wikidata hierarchies: taxa under Biota (P171) with
//! their compounds, and compounds under chemical compound (Q11173) via P279.

use std::collections::{HashMap, HashSet};

use super::index::{Hierarchy, Index, PubChemData, qid};
use super::nodes::{ChemicalNode, TaxonCompound, TaxonIdentifiers, TaxonNode, sort_by_name};

/// Build the biological tree: every taxon with a compound, every ancestor of
/// one, and the compounds with an `InChIKey` as leaves.
pub(super) fn biological_tree(data: &PubChemData, index: &Index) -> Vec<TaxonNode> {
    let mut taxon_compounds: HashMap<String, Vec<String>> = HashMap::new();
    for row in &data.compound_taxon {
        taxon_compounds
            .entry(qid(row, "taxon"))
            .or_default()
            .push(qid(row, "compound"));
    }
    let taxa = Hierarchy::new(&data.taxon_parent, "taxon", "taxon_parent");

    let mut relevant: HashSet<&str> = HashSet::new();
    for taxon in taxon_compounds.keys() {
        let mut current = Some(taxon);
        while let Some(taxon) = current.filter(|t| relevant.insert(t.as_str())) {
            current = taxa.parent(taxon);
        }
    }

    // Sorted by QID, as `lotus_pubchem_tree.py` does, so which root claims a
    // shared descendant first does not depend on hash order.
    let mut roots: Vec<&str> = relevant
        .iter()
        .copied()
        .filter(|t| {
            taxa.parent(t)
                .is_none_or(|p| !relevant.contains(p.as_str()))
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
    sort_by_name(roots.into_iter().filter_map(|r| builder.node(r)).collect())
}

/// What one biological-tree walk reads, and the taxa it has placed.
struct BiologicalTree<'a> {
    index: &'a Index,
    taxa: &'a Hierarchy,
    taxon_compounds: &'a HashMap<String, Vec<String>>,
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
    sort_by_name(
        classes
            .roots()
            .into_iter()
            .filter_map(|root| chemical_node(root, &classes, index, &mut visited))
            .collect(),
    )
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
