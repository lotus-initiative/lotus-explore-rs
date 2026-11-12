// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![allow(clippy::expect_used, clippy::indexing_slicing)]
//! Unit tests for `pubchem_tree`.

use std::collections::BTreeSet;

use lotus_query::QLEVER_WIKIDATA;
use lotus_search::Service;
use lotus_search::testing::Scripted;
use serde_json::{Value, json};

use super::index::PubChemData;
use super::nodes::{
    ChemicalNode, Class, ClassMember, Classification, CompoundNode, Descriptors, Level, Qid,
    Reference, TaxonCompound, TaxonIdentifiers, TaxonNode, count_nodes,
};
use super::*;
use crate::export::pipeline::Row;

const NOW: &str = "2026-03-04T05:06:07Z";

fn row_map(pairs: &[(&str, &str)]) -> Row {
    pairs
        .iter()
        .map(|(k, v)| (std::sync::Arc::from(*k), (*v).to_string()))
        .collect()
}

/// A value as the documents write it.
fn written(value: &impl serde::Serialize) -> Value {
    serde_json::to_value(value).expect("the nodes serialize")
}

// The twelve Wikidata tables, in fetch order. Every row is there to make one
// branch matter: Q3 has no InChIKey, Q4 has two and no structure, Q50..Q52 are
// classes without taxa, Q103 is a taxon without compounds, Q104 one whose only
// compound has no InChIKey, Q102 an empty NCBI id, R1/R3 an empty PMID/DOI.
const COMPOUND_TAXON: &str = "compound,compound_inchikey,taxon\n\
Q1,IK1,Q101\nQ2,IK2,Q101\nQ4,IK4A,Q101\nQ1,IK1,Q102\nQ4,IK4B,Q102\nQ3,,Q104\n";
const TAXON_NCBI: &str = "taxon,taxon_ncbi\nQ900,33090\nQ101,4204\nQ102,\n";
const TAXON_NAME: &str = "taxon,taxon_name\n\
Q900,Plantae\nQ101,Gentiana\nQ102,Arnica\nQ103,Lonely\nQ104,Barren\n";
const TAXON_PARENT: &str = "taxon,taxon_parent\nQ101,Q900\nQ102,Q900\nQ103,Q900\nQ104,Q900\n";
const SMILES_CAN: &str = "compound,compound_smiles_can\nQ1,CN1C\nQ2,CC\n";
const SMILES_ISO: &str = "compound,compound_smiles_iso\nQ1,CN1C@\n";
const SMARTS: &str = "compound,compound_smarts\nQ1,[#6]\nQ2,\n";
const CXSMILES: &str = "compound,compound_cxsmiles\nQ1,\nQ2,CC-cx\n";
const LABEL: &str = "compound,compound_label\nQ1,caffeine\nQ3,ghost\nQ50,Purines\nQ51,Xanthines\n";
const COMPOUND_PARENT: &str = "compound,compound_parent\nQ1,Q50\nQ51,Q50\nQ2,Q51\nQ52,Q50\n";
const REFERENCE_DOI: &str = "compound,taxon,reference,doi\n\
Q1,Q101,R1,10.1/b\nQ1,Q101,R2,10.1/a\nQ2,Q101,R3,\n";
const REFERENCE_PMID: &str = "compound,taxon,reference,pmid\n\
Q1,Q101,R2,7\nQ1,Q101,R1,\nQ2,Q101,R3,5\n";

// A classified row, an unclassified one with a trailing empty pathway, one whose
// SMILES no compound has (its whole pathway goes), and one carrying an error.
const NPCLASSIFIER: &str = "smiles,pathway,superclass,class,error\n\
CN1C,Alkaloids,Purine alkaloids,Xanthines,\n\
CC,Alkaloids $ ,Purine alkaloids,,\n\
XYZ,Terpenoids,Monoterpenoids,Menthanes,\n\
CN1C,Polyketides,Macrolides,,boom\n";

/// A transport answering the twelve tables, then the `NPClassifier` cache.
fn scripted() -> Scripted {
    Scripted::new(vec![
        (200, COMPOUND_TAXON),
        (200, TAXON_NCBI),
        (200, TAXON_NAME),
        (200, TAXON_PARENT),
        (200, SMILES_CAN),
        (200, SMILES_ISO),
        (200, SMARTS),
        (200, CXSMILES),
        (200, LABEL),
        (200, COMPOUND_PARENT),
        (200, REFERENCE_DOI),
        (200, REFERENCE_PMID),
        (200, NPCLASSIFIER),
    ])
}

fn q1_descriptors() -> Value {
    json!({"InChIKey": "IK1", "SMILES": ["CN1C", "CN1C@"], "SMARTS": "[#6]"})
}

fn q2_descriptors() -> Value {
    json!({"InChIKey": "IK2", "SMILES": "CC", "CXSMILES": "CC-cx"})
}

fn q4_descriptors() -> Value {
    json!({"InChIKey": ["IK4A", "IK4B"]})
}

/// The `metadata` block every full document carries.
fn metadata(
    name: &str,
    description: &str,
    source: (&str, &str),
    root: &str,
    total: usize,
) -> Value {
    json!({
        "name": name,
        "description": description,
        "version": "0.1.0",
        "generated": NOW,
        "generator": "LOTUS PubChem Tree Generator",
        "source": {
            "name": "LOTUS",
            "url": "https://lotus.nprod.net/",
            "wikidata_item": "Q104225190",
            "endpoint": source.0,
            "classification": source.1,
        },
        "license": {"data": "AGPL-3.0-only", "code": "AGPL-3.0-only"},
        "constraints": {"root": root},
        "statistics": {"root_nodes": 1, "total_nodes": total},
    })
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("the generator writes JSON")
}

/// The names under `_documentation.structure.node_fields`.
fn node_fields(doc: &Value) -> Vec<String> {
    doc["_documentation"]["structure"]["node_fields"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

#[tokio::test]
async fn index_names_and_describes_each_compound() {
    let data = fetch_all_pubchem_data(&scripted(), &Endpoint::new(Service::Qlever), &mut |_| {})
        .await
        .expect("the tables parse");
    let index = Index::new(&data);
    let compounds = ["Q1", "Q2", "Q3", "Q4", "Q50", "Q99"];
    let nodes: Vec<Value> = compounds
        .iter()
        .map(|q| written(&index.compound_node(q)))
        .collect();
    assert_eq!(
        nodes,
        [
            caffeine(),
            json!({"Name": "Q2", "Identifiers": {"QID": "Q2"}, "Descriptors": q2_descriptors()}),
            json!({"Name": "ghost", "Identifiers": {"QID": "Q3"}}),
            json!({"Name": "Q4", "Identifiers": {"QID": "Q4"}, "Descriptors": q4_descriptors()}),
            json!({"Name": "Purines", "Identifiers": {"QID": "Q50"}}),
            json!({"Name": "Q99", "Identifiers": {"QID": "Q99"}}),
        ]
    );
    let with_taxa: Vec<&str> = compounds
        .into_iter()
        .filter(|q| index.has_taxa(q))
        .collect();
    assert_eq!(with_taxa, ["Q1", "Q2", "Q4"]);
}

/// The six documents, parsed: biological, chemical and `NPClassifier`, full
/// then compact.
async fn documents() -> [Value; 6] {
    let http = scripted();
    let mut steps = Vec::new();
    let documents = run_pubchem_tree(&http, Endpoint::new(Service::Qlever), NOW, |step| {
        steps.push(step);
    })
    .await
    .expect("the pipeline runs");
    assert_eq!(http.call_count(), 13);
    assert_eq!(steps.len() as u64, PUBCHEM_TREE_QUERY_COUNT);
    assert_eq!(
        steps,
        [
            "compound_taxon",
            "taxon_ncbi",
            "taxon_name",
            "taxon_parent",
            "compound_smiles_can",
            "compound_smiles_iso",
            "compound_smarts",
            "compound_cxsmiles",
            "compound_label",
            "compound_parent",
            "reference_doi",
            "reference_pmid",
            "npclassifier_cache",
        ]
    );
    let queries = http.queries();
    assert!(
        queries[10].contains("SELECT DISTINCT ?compound ?taxon ?reference ?doi"),
        "biological tree references require compound and taxon columns: {}",
        queries[10]
    );
    assert!(
        queries[11].contains("SELECT DISTINCT ?compound ?taxon ?reference ?pmid"),
        "biological tree references require compound and taxon columns: {}",
        queries[11]
    );
    assert_eq!(
        http.endpoints().last().map(String::as_str),
        Some(NPCLASSIFIER_CACHE_URL)
    );
    <[String; 6]>::from(documents).map(|t| parse(&t))
}

fn caffeine() -> Value {
    json!({"Name": "caffeine", "Identifiers": {"QID": "Q1"}, "Descriptors": q1_descriptors()})
}

#[tokio::test]
async fn biological_tree_keeps_taxa_with_inchikey_compounds() {
    let [bio, ..] = documents().await;
    let caffeine =
        json!({"Name": "caffeine", "Identifiers": {"QID": "Q1"}, "Descriptors": q1_descriptors()});
    let mut caffeine_cited = caffeine.clone();
    caffeine_cited["References"] = json!([
        {"QID": "R2", "DOI": "10.1/a", "PMID": "7"},
        {"QID": "R1", "DOI": "10.1/b"},
    ]);
    let q4 = json!({"Name": "Q4", "Identifiers": {"QID": "Q4"}, "Descriptors": q4_descriptors()});

    assert_eq!(
        bio["tree"],
        json!([{
            "Name": "Plantae",
            "Identifiers": {"QID": "Q900", "NCBI_TaxID": "33090"},
            "Children": [
                {
                    "Name": "Arnica",
                    "Identifiers": {"QID": "Q102"},
                    "Compounds": [caffeine, q4],
                },
                {
                    "Name": "Gentiana",
                    "Identifiers": {"QID": "Q101", "NCBI_TaxID": "4204"},
                    "Compounds": [
                        caffeine_cited,
                        {
                            "Name": "Q2",
                            "Identifiers": {"QID": "Q2"},
                            "Descriptors": q2_descriptors(),
                            "References": [{"QID": "R3", "PMID": "5"}],
                        },
                        q4,
                    ],
                },
            ],
        }])
    );
    assert_eq!(
        bio["metadata"],
        metadata(
            "LOTUS biological Tree",
            "Hierarchical taxonomy of biological organisms with associated natural product compounds",
            (QLEVER_WIKIDATA, "Wikidata"),
            "Biota (Q2382443)",
            3,
        )
    );
    assert_eq!(
        bio["_documentation"]["overview"],
        "This JSON contains a hierarchical tree of biological taxa with their associated natural product compounds."
    );
    assert_eq!(
        node_fields(&bio),
        [
            "Children",
            "Compounds",
            "Descriptors",
            "Identifiers",
            "Name",
            "References"
        ]
    );
}

#[tokio::test]
async fn chemical_tree_keeps_classes_above_compounds_with_taxa() {
    let [_, chem, ..] = documents().await;
    let caffeine = caffeine();
    assert_eq!(
        chem["tree"],
        json!([{
            "Name": "Purines",
            "Identifiers": {"QID": "Q50"},
            "Children": [
                {
                    "Name": "Xanthines",
                    "Identifiers": {"QID": "Q51"},
                    "Children": [
                        {"Name": "Q2", "Identifiers": {"QID": "Q2"}, "Descriptors": q2_descriptors()},
                    ],
                },
                caffeine,
            ],
        }])
    );
    assert_eq!(
        chem["metadata"],
        metadata(
            "LOTUS chemical Tree (Wikidata)",
            "Wikidata-based hierarchical classification of chemical compounds",
            (QLEVER_WIKIDATA, "Wikidata"),
            "chemical compound (Q11173)",
            4,
        )
    );
    assert_eq!(
        node_fields(&chem),
        ["Children", "Descriptors", "Identifiers", "Name"]
    );
}

#[tokio::test]
async fn npclassifier_tree_keeps_known_structures() {
    let [_, _, npc, ..] = documents().await;
    assert_eq!(
        npc["tree"],
        json!([{
            "Name": "Alkaloids",
            "Identifiers": {"QID": "Alkaloids", "NPClassifier_Pathway": "Alkaloids"},
            "Children": [{
                "Name": "Purine alkaloids",
                "Identifiers": {
                    "QID": "Purine alkaloids",
                    "NPClassifier_Superclass": "Purine alkaloids",
                },
                "Children": [
                    {
                        "Name": "CC",
                        "Identifiers": {"QID": "Q2"},
                        "Descriptors": {"InChIKey": "IK2", "SMILES": "CC"},
                    },
                    {
                        "Name": "Xanthines",
                        "Identifiers": {"QID": "Xanthines", "NPClassifier_Class": "Xanthines"},
                        "Children": [{
                            "Name": "caffeine",
                            "Identifiers": {"QID": "Q1"},
                            "Descriptors": {"InChIKey": "IK1", "SMILES": "CN1C"},
                        }],
                    },
                ],
            }],
        }])
    );
    assert_eq!(
        npc["metadata"],
        metadata(
            "LOTUS chemical Tree (NPClassifier)",
            "NPClassifier-based hierarchical classification of natural products",
            (NPCLASSIFIER_CACHE_URL, "NPClassifier"),
            "NPClassifier pathways",
            5,
        )
    );
}

#[tokio::test]
async fn compact_documents_key_by_name() {
    let [_, _, _, bio_pubchem, chem_pubchem, npc_pubchem] = documents().await;
    assert_eq!(
        bio_pubchem,
        json!({"children": {"Plantae": {"QID": "Q900", "children": {
            "Arnica": {"QID": "Q102"},
            "Gentiana": {"QID": "Q101", "children": {"4204": {
                "organism_name": ["Gentiana"],
                "compounds": {
                    "IK1": {
                        "Name": "caffeine",
                        "QID": "Q1",
                        "references": [{"DOI": "10.1/a", "PMID": "7"}, {"DOI": "10.1/b"}],
                    },
                    "IK2": {"Name": "Q2", "QID": "Q2", "references": [{"PMID": "5"}]},
                    "IK4A": {"Name": "Q4", "QID": "Q4"},
                    "IK4B": {"Name": "Q4", "QID": "Q4"},
                },
            }}},
        }}}})
    );
    assert_eq!(
        chem_pubchem,
        json!({"children": {"Purines": {"QID": "Q50", "children": {
            "Xanthines": {"QID": "Q51", "children": {
                "Q2": {"QID": "Q2", "children": {"IK2": {"Name": "Q2", "QID": "Q2"}}},
            }},
            "caffeine": {"QID": "Q1", "children": {"IK1": {"Name": "caffeine", "QID": "Q1"}}},
        }}}})
    );
    assert_eq!(
        npc_pubchem,
        json!({"children": {"Alkaloids": {"children": {"Purine alkaloids": {
            "Xanthines": {"IK1": {"Name": "caffeine", "QID": "Q1"}},
        }}}}})
    );
}

/// A compound node with only its `InChIKey`s for descriptors.
fn compound(name: &str, qid: &str, inchikeys: &[&str]) -> CompoundNode {
    CompoundNode {
        name: name.to_string(),
        identifiers: Qid {
            qid: qid.to_string(),
        },
        descriptors: Descriptors {
            inchikey: inchikeys.iter().map(ToString::to_string).collect(),
            ..Descriptors::default()
        },
    }
}

fn reference(qid: &str, doi: Option<&str>, pmid: Option<&str>) -> Reference {
    Reference {
        qid: qid.to_string(),
        doi: doi.map(str::to_string),
        pmid: pmid.map(str::to_string),
    }
}

fn cited(compound: CompoundNode, references: Vec<Reference>) -> TaxonCompound {
    TaxonCompound {
        compound,
        references,
    }
}

fn taxon(
    (name, qid, ncbi): (&str, &str, Option<&str>),
    compounds: Vec<TaxonCompound>,
    children: Vec<TaxonNode>,
) -> TaxonNode {
    TaxonNode {
        name: name.to_string(),
        identifiers: TaxonIdentifiers {
            qid: qid.to_string(),
            ncbi_taxid: ncbi.map(str::to_string),
        },
        compounds,
        children,
    }
}

#[test]
fn biological_compact_form_merges_compounds_by_inchikey() {
    let tree = [taxon(
        ("Root", "Q900", Some("1")),
        vec![
            cited(
                compound("Alpha", "Q1", &["IK1"]),
                vec![
                    reference("R1", Some("10.1/a"), None),
                    reference("R2", None, Some("99")),
                    reference("R3", None, None),
                ],
            ),
            cited(compound("", "", &["IK2", "IK3"]), Vec::new()),
            cited(
                compound("Beta", "Q3", &["IK1"]),
                vec![
                    reference("R4", Some("10.1/a"), None),
                    reference("R5", Some("10.1/b"), Some("7")),
                ],
            ),
            cited(compound("No key", "Q4", &[]), Vec::new()),
        ],
        vec![taxon(
            ("Leaf", "", None),
            vec![cited(compound("Five", "Q5", &["IK5"]), Vec::new())],
            Vec::new(),
        )],
    )];
    assert_eq!(
        written(&compact::biological(&tree)),
        json!({"children": {"Root": {"QID": "Q900", "children": {
            "Leaf": {},
            "1": {
                "organism_name": ["Root"],
                "compounds": {
                    "IK1": {
                        "Name": "Alpha",
                        "QID": "Q1",
                        "references": [{"DOI": "10.1/a"}, {"PMID": "99"}, {"DOI": "10.1/b", "PMID": "7"}],
                    },
                    "IK2": {},
                    "IK3": {},
                },
            },
        }}}})
    );
    assert_eq!(written(&compact::biological(&[])), json!({}));
}

fn chemical(compound: CompoundNode, children: Vec<ChemicalNode>) -> ChemicalNode {
    ChemicalNode { compound, children }
}

#[test]
fn chemical_compact_form_names_leaves_only_when_known() {
    let tree = [
        chemical(
            compound("Class", "Q10", &[]),
            vec![
                chemical(compound("Caffeine", "Q1", &["IK1"]), Vec::new()),
                chemical(compound("Unknown", "", &["IK2", "IK3"]), Vec::new()),
            ],
        ),
        chemical(compound("", "Q2", &["IK4"]), Vec::new()),
    ];
    assert_eq!(
        written(&compact::chemical(&tree)),
        json!({"children": {
            "Class": {"QID": "Q10", "children": {
                "Caffeine": {"QID": "Q1", "children": {"IK1": {"Name": "Caffeine", "QID": "Q1"}}},
                "Unknown": {"children": {"IK2": {}, "IK3": {}}},
            }},
            "": {"QID": "Q2", "children": {"IK4": {"QID": "Q2"}}},
        }})
    );
    assert_eq!(written(&compact::chemical(&[])), json!({}));
}

fn classification<C>(level: Level, name: &str, children: Vec<C>) -> Classification<C> {
    Classification {
        level,
        name: name.to_string(),
        children,
    }
}

fn class(name: &str, leaves: Vec<CompoundNode>) -> ClassMember {
    ClassMember::Class(classification(Level::Class, name, leaves))
}

#[test]
fn npclassifier_compact_form_keeps_only_classified_compounds() {
    let unclassified = || ClassMember::Unclassified(compound("Loose", "Q9", &["IK9"]));
    let tree = [
        classification(
            Level::Pathway,
            "Alkaloids",
            vec![
                classification(
                    Level::Superclass,
                    "Indoles",
                    vec![
                        class(
                            "Carbazoles",
                            vec![
                                compound("Carb", "Q5", &["IK5"]),
                                compound("Unknown", "", &["IK6", "IK7"]),
                                compound("", "Q8", &["IK8"]),
                            ],
                        ),
                        unclassified(),
                        class("Tryptamines", vec![compound("Trypt", "Q1", &["IK1"])]),
                    ],
                ),
                classification(Level::Superclass, "Hollow", vec![unclassified()]),
            ],
        ),
        classification(
            Level::Pathway,
            "Fatty acids",
            vec![classification(
                Level::Superclass,
                "Bare",
                vec![unclassified()],
            )],
        ),
    ];
    assert_eq!(
        written(&compact::npclassifier(&tree)),
        json!({"children": {"Alkaloids": {"children": {"Indoles": {
            "Carbazoles": {"IK5": {"Name": "Carb", "QID": "Q5"}, "IK6": {}, "IK7": {}, "IK8": {"QID": "Q8"}},
            "Tryptamines": {"IK1": {"Name": "Trypt", "QID": "Q1"}},
        }}}}})
    );
    assert_eq!(
        written(&compact::npclassifier(&[])),
        json!({"children": {}})
    );
}

#[test]
fn npclassifier_levels_identify_by_name() {
    let tree: [Class; 1] = [classification(
        Level::Class,
        "Xanthines",
        vec![compound("caffeine", "Q1", &["IK1"])],
    )];
    assert_eq!(
        written(&tree),
        json!([{
            "Name": "Xanthines",
            "Identifiers": {"QID": "Xanthines", "NPClassifier_Class": "Xanthines"},
            "Children": [{"Name": "caffeine", "Identifiers": {"QID": "Q1"}, "Descriptors": {"InChIKey": "IK1"}}],
        }])
    );
}

#[test]
fn index_keeps_inchikeys_of_compounds_with_taxa() {
    let data = PubChemData {
        compound_taxon: vec![
            row_map(&[
                ("compound", "http://www.wikidata.org/entity/Q1"),
                ("compound_inchikey", "IK1"),
                ("taxon", "http://www.wikidata.org/entity/Q2"),
            ]),
            row_map(&[
                ("compound", "http://www.wikidata.org/entity/Q3"),
                ("compound_inchikey", ""),
                ("taxon", "http://www.wikidata.org/entity/Q4"),
            ]),
        ],
        ..Default::default()
    };
    let index = Index::new(&data);
    assert!(index.has_taxa("Q1"), "Q1 has an InChIKey");
    assert!(!index.has_taxa("Q3"), "Q3 has none");
    assert_eq!(
        index.compounds.get("Q1").map(|p| &p.inchikeys),
        Some(&BTreeSet::from(["IK1".to_string()]))
    );
}

#[test]
fn classify_pairs_superclasses_with_classes_by_position() {
    let rows = [
        row_map(&[
            ("smiles", "C"),
            ("pathway", "P1 $ P2"),
            ("superclass", "S1 $ S2 $ S3"),
            ("class", "C1 $ _unclassified"),
            ("error", ""),
        ]),
        row_map(&[
            ("smiles", "N"),
            ("pathway", "P1"),
            ("superclass", "S1"),
            ("class", "C1"),
        ]),
        row_map(&[
            ("smiles", ""),
            ("pathway", "P1"),
            ("superclass", "S1"),
            ("class", "C9"),
        ]),
        row_map(&[
            ("smiles", "O"),
            ("pathway", "P1"),
            ("superclass", "S1"),
            ("error", "x"),
        ]),
    ];
    let set =
        |items: &[&str]| -> BTreeSet<String> { items.iter().map(ToString::to_string).collect() };
    let members =
        |classes: &[(&str, &[&str])], unclassified: &[&str]| npclassifier::SuperclassMembers {
            classes: classes
                .iter()
                .map(|(c, s)| ((*c).to_string(), set(s)))
                .collect(),
            unclassified: set(unclassified),
        };
    let p1 = || {
        [
            ("S1".to_string(), members(&[("C1", &["C", "N"])], &[])),
            ("S2".to_string(), members(&[], &["C"])),
            ("S3".to_string(), members(&[], &["C"])),
        ]
    };
    let mut p2 = p1();
    if let Some((_, s1)) = p2.first_mut() {
        *s1 = members(&[("C1", &["C"])], &[]);
    }
    assert_eq!(
        npclassifier::classify(&rows),
        npclassifier::Classified::from([
            ("P1".to_string(), p1().into()),
            ("P2".to_string(), p2.into()),
        ])
    );
}

#[test]
fn superclass_places_unclassified_compounds_where_their_class_name_sorts() {
    let structure =
        |q: &str, smiles: &str| row_map(&[("compound", q), ("compound_smiles_can", smiles)]);
    let data = PubChemData {
        compound_taxon: ["Q1", "Q2", "Q3", "Q4"]
            .map(|q| {
                row_map(&[
                    ("compound", q),
                    ("compound_inchikey", &format!("IK-{q}")),
                    ("taxon", "T"),
                ])
            })
            .into(),
        compound_smiles_can: vec![
            structure("Q1", "S1"),
            structure("Q2", "S2"),
            structure("Q3", "S3"),
            structure("Q4", "S4"),
        ],
        compound_label: vec![
            row_map(&[("compound", "Q1"), ("compound_label", "Zeta")]),
            row_map(&[("compound", "Q2"), ("compound_label", "zeta")]),
        ],
        ..Default::default()
    };
    let index = Index::new(&data);
    let row = |smiles: &str, class: &str| {
        row_map(&[
            ("smiles", smiles),
            ("pathway", "P"),
            ("superclass", "S"),
            ("class", class),
        ])
    };
    let rows = [
        row("S1", ""),
        row("S2", ""),
        row("S3", "Zeta"),
        row("S4", "zeta"),
        row("S5", "Gone"),
    ];
    let tree = npclassifier_tree(&classify(&rows), &SmilesIndex::new(&data, &index), &index);
    let leaf = |name: &str, q: &str, smiles: &str| json!({"Name": name, "Identifiers": {"QID": q}, "Descriptors": {"InChIKey": format!("IK-{q}"), "SMILES": smiles}});
    let class = |name: &str, child: Value| json!({"Name": name, "Identifiers": {"QID": name, "NPClassifier_Class": name}, "Children": [child]});
    // "Zeta" sorts before "_unclassified" and "zeta" after it, so the class
    // leads its namesake leaf in the first pair and follows it in the second.
    assert_eq!(
        written(&tree),
        json!([{
            "Name": "P",
            "Identifiers": {"QID": "P", "NPClassifier_Pathway": "P"},
            "Children": [{
                "Name": "S",
                "Identifiers": {"QID": "S", "NPClassifier_Superclass": "S"},
                "Children": [
                    class("Zeta", leaf("S3", "Q3", "S3")),
                    leaf("Zeta", "Q1", "S1"),
                    leaf("zeta", "Q2", "S2"),
                    class("zeta", leaf("S4", "Q4", "S4")),
                ],
            }],
        }])
    );
    assert_eq!(count_nodes(&tree), 8);
}

#[test]
fn descriptors_write_one_value_as_a_string() {
    assert_eq!(written(&Descriptors::default()), json!({}));
    assert_eq!(
        written(&Descriptors {
            inchikey: vec!["a".to_string()],
            smiles: vec!["s1".to_string(), "s2".to_string()],
            smarts: Some("[#6]".to_string()),
            cxsmiles: None,
        }),
        json!({"InChIKey": "a", "SMILES": ["s1", "s2"], "SMARTS": "[#6]"})
    );
}

#[test]
fn count_nodes_counts_children_not_compounds() {
    let leaf = |q: &str| chemical(compound(q, q, &[]), Vec::new());
    let chemical_tree = [
        chemical(compound("A", "Q1", &[]), vec![leaf("B"), leaf("C")]),
        leaf("D"),
    ];
    assert_eq!(count_nodes(&chemical_tree), 4);
    let compounds = vec![cited(compound("x", "Q9", &["IK"]), Vec::new())];
    let biological_tree = [taxon(("T", "Q1", None), compounds, Vec::new())];
    assert_eq!(count_nodes(&biological_tree), 1);
}

#[test]
fn index_prefers_mul_compound_labels_over_english_regardless_of_row_order() {
    let label_row = |q: &str, label: &str, lang: &str| {
        row_map(&[("compound", q), ("compound_label", label), ("lang", lang)])
    };
    let data = PubChemData {
        compound_label: vec![
            label_row("Q1", "Caffeine (mul)", "mul"),
            label_row("Q1", "Caffeine (en)", "en"),
            label_row("Q2", "Theobromine (en)", "en"),
            label_row("Q2", "Theobromine (mul)", "mul"),
            label_row("Q3", "First en", "en"),
            label_row("Q3", "Second en", "en"),
        ],
        ..Default::default()
    };
    let index = Index::new(&data);
    assert_eq!(index.compound_node("Q1").name, "Caffeine (mul)");
    assert_eq!(index.compound_node("Q2").name, "Theobromine (mul)");
    assert_eq!(index.compound_node("Q3").name, "Second en");
}

#[test]
fn biological_tree_walks_all_parents_when_a_taxon_has_multiple_parent_rows() {
    let parent_row =
        |child: &str, parent: &str| row_map(&[("taxon", child), ("taxon_parent", parent)]);
    let data = PubChemData {
        compound_taxon: vec![row_map(&[
            ("compound", "Q1"),
            ("compound_inchikey", "IK1"),
            ("taxon", "Q100"),
        ])],
        // Q100 has two parents: Q200 (which leads up to Biota Q10) and Q300
        // (which is in a two-node cycle Q300 <-> Q301 and comes last in row order).
        taxon_parent: vec![
            parent_row("Q100", "Q200"),
            parent_row("Q200", "Q10"),
            parent_row("Q100", "Q300"),
            parent_row("Q300", "Q301"),
            parent_row("Q301", "Q300"),
        ],
        taxon_name: vec![
            row_map(&[("taxon", "Q10"), ("taxon_name", "Biota")]),
            row_map(&[("taxon", "Q200"), ("taxon_name", "Plantae")]),
            row_map(&[("taxon", "Q100"), ("taxon_name", "Salvia")]),
        ],
        ..Default::default()
    };
    let index = Index::new(&data);
    let tree = trees::biological_tree(&data, &index);
    assert_eq!(
        written(&tree),
        json!([{
            "Name": "Biota",
            "Identifiers": {"QID": "Q10"},
            "Children": [{
                "Name": "Plantae",
                "Identifiers": {"QID": "Q200"},
                "Children": [{
                    "Name": "Salvia",
                    "Identifiers": {"QID": "Q100"},
                    "Compounds": [{
                        "Name": "Q1",
                        "Identifiers": {"QID": "Q1"},
                        "Descriptors": {"InChIKey": "IK1"},
                    }],
                }],
            }],
        }])
    );
}

#[tokio::test]
async fn run_pubchem_tree_records_configured_endpoint_in_wikidata_metadata() {
    let endpoint = Endpoint::new(Service::Wdqs);
    let expected_url = endpoint.url().to_string();
    let (bio_full, chem_full, npc_full, _, _, _) =
        run_pubchem_tree(&scripted(), endpoint, NOW, |_| {})
            .await
            .expect("pubchem tree succeeds");

    assert_eq!(
        parse(&bio_full)["metadata"]["source"]["endpoint"],
        expected_url
    );
    assert_eq!(
        parse(&chem_full)["metadata"]["source"]["endpoint"],
        expected_url
    );
    assert_eq!(
        parse(&npc_full)["metadata"]["source"]["endpoint"],
        NPCLASSIFIER_CACHE_URL
    );
}
