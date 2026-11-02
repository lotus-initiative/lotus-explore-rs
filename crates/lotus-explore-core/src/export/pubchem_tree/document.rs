// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The full form of a tree: the tree under a documentation block and a
//! metadata block.

use lotus_query::QLEVER_WIKIDATA;
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::nodes::{TreeNode, count_nodes};
use super::{APP_NAME, APP_VERSION, NPCLASSIFIER_CACHE_URL};

/// Which of the three trees a document carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TreeKind {
    Biological,
    Chemical,
    NpClassifier,
}

/// The fixed text one kind of document carries.
struct Texts {
    name: &'static str,
    overview: &'static str,
    description: &'static str,
    notes: [&'static str; 4],
    root: &'static str,
    classification: &'static str,
    endpoint: &'static str,
}

impl TreeKind {
    const fn texts(self) -> Texts {
        match self {
            Self::Biological => Texts {
                name: "LOTUS biological Tree",
                overview: "This JSON contains a hierarchical tree of biological taxa with their associated natural product compounds.",
                description: "Hierarchical taxonomy of biological organisms with associated natural product compounds",
                notes: [
                    "Descriptor values are strings when single, arrays when multiple values exist (very rare).",
                    "All nodes are sorted alphabetically by Name.",
                    "Only nodes with InChIKey associations (directly or via descendants) are included.",
                    "Data queried from Wikidata via QLever SPARQL endpoint.",
                ],
                root: "Biota (Q2382443)",
                classification: "Wikidata",
                endpoint: QLEVER_WIKIDATA,
            },
            Self::Chemical => Texts {
                name: "LOTUS chemical Tree (Wikidata)",
                overview: "This JSON contains a hierarchical tree of chemical compound classes with structural descriptors based on Wikidata P279 relationships.",
                description: "Wikidata-based hierarchical classification of chemical compounds",
                notes: [
                    "Note: Wikidata P279 (subclass of) relationships are sparse for natural products.",
                    "Consider using the NPClassifier-based tree (lotus_chemical_tree.json) for better coverage.",
                    "Descriptor values are strings when single, arrays when multiple values exist (very rare).",
                    "All nodes are sorted alphabetically by Name.",
                ],
                root: "chemical compound (Q11173)",
                classification: "Wikidata",
                endpoint: QLEVER_WIKIDATA,
            },
            Self::NpClassifier => Texts {
                name: "LOTUS chemical Tree (NPClassifier)",
                overview: "This JSON contains a hierarchical tree of chemical compounds classified using NPClassifier (pathway → superclass → class).",
                description: "NPClassifier-based hierarchical classification of natural products",
                notes: [
                    "NPClassifier provides a comprehensive classification for natural products.",
                    "Hierarchy: pathway → superclass → class → InChIKey",
                    "Multiple classifications are possible for a single compound.",
                    "See https://npclassifier.gnps2.org/ for more information.",
                ],
                root: "NPClassifier pathways",
                classification: "NPClassifier",
                endpoint: NPCLASSIFIER_CACHE_URL,
            },
        }
    }
}

/// One tree in its full form.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct FullDocument<'a, T> {
    #[serde(rename = "_documentation")]
    documentation: Value,
    metadata: Value,
    tree: &'a [T],
}

impl<'a, T: TreeNode> FullDocument<'a, T> {
    pub(super) fn new(kind: TreeKind, tree: &'a [T], generated: &str) -> Self {
        let texts = kind.texts();
        Self {
            documentation: documentation(kind, &texts),
            metadata: json!({
                "name": texts.name,
                "description": texts.description,
                "version": APP_VERSION,
                "generated": generated,
                "generator": APP_NAME,
                "source": {
                    "name": "LOTUS",
                    "url": "https://lotus.nprod.net/",
                    "wikidata_item": "Q104225190",
                    "endpoint": texts.endpoint,
                    "classification": texts.classification,
                },
                "license": {"data": "AGPL-3.0-only", "code": "AGPL-3.0-only"},
                "constraints": {"root": texts.root},
                "statistics": {"root_nodes": tree.len(), "total_nodes": count_nodes(tree)},
            }),
            tree,
        }
    }
}

/// The `_documentation` block: what the fields of this kind of tree mean.
fn documentation(kind: TreeKind, texts: &Texts) -> Value {
    let mut node_fields = vec![
        ("Name", "Human-readable name (taxon name or compound label)"),
        (
            "Identifiers",
            "External database identifiers (Wikidata QID, NCBI TaxID for taxa, NPClassifier levels)",
        ),
        (
            "Descriptors",
            "Chemical structure representations (InChIKey, SMILES, etc.)",
        ),
        (
            "Children",
            "Array of child nodes (same structure, recursive)",
        ),
    ];
    if kind == TreeKind::Biological {
        node_fields.extend([
            (
                "Compounds",
                "(Biological tree only) Array of compounds found in this taxon",
            ),
            (
                "References",
                "(Biological tree compounds only) Literature references for compound-taxon association",
            ),
        ]);
    }
    let node_fields: Map<String, Value> = node_fields
        .into_iter()
        .map(|(field, meaning)| (field.to_string(), Value::from(meaning)))
        .collect();
    json!({
        "overview": texts.overview,
        "structure": {
            "tree": "Array of root nodes. Each node is an object with Name, Identifiers, and optional Children.",
            "node_fields": node_fields,
            "descriptors_fields": {
                "InChIKey": "IUPAC International Chemical Identifier Key",
                "SMILES": "Simplified Molecular Input Line Entry System (isomeric preferred over canonical)",
                "SMARTS": "SMILES Arbitrary Target Specification (substructure patterns)",
                "CXSMILES": "ChemAxon Extended SMILES",
            },
            "reference_fields": {
                "QID": "QID of the reference article in Wikidata",
                "DOI": "Digital Object Identifier of the reference",
                "PMID": "PubMed ID of the reference",
            },
        },
        "notes": texts.notes,
    })
}
