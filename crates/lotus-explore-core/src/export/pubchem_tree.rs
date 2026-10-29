// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The LOTUS PubChem tree generator: fetch Wikidata tables, build biological
//! / chemical / NPClassifier trees, serialize JSON.
//!
//! Mirrors `lotus_pubchem_tree.py`: it fetches the same 12 SPARQL tables,
//! indexes them by QID, builds three hierarchical trees, and writes JSON in
//! either a rich "full" form or PubChem's compact name-as-key form.
//!
//! No clock: the timestamp comes from the caller. No UI: the CLI writes the
//! files. Wasm-clean: all I/O goes through the injected [`Http`] transport.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use lotus_query::{
    QLEVER_WIKIDATA, extract_qid, query_compound_cxsmiles, query_compound_inchikey_taxon,
    query_compound_label, query_compound_parent, query_compound_smarts,
    query_compound_smiles_can_all, query_compound_smiles_iso_all, query_reference_doi,
    query_reference_pmid, query_taxon_name, query_taxon_ncbi, query_taxon_parent,
};
use lotus_search::{Endpoint, Http, HttpResponse};

use super::pipeline::{ExportError, Row, fetch_csv_rows};

/// The `NPClassifier` cache CSV URL.
const NPCLASSIFIER_CACHE_URL: &str = "https://media.githubusercontent.com/media/adafede/marimo/main/apps/public/npclassifier/npclassifier_cache.csv";

/// The app version carried in every metadata document.
const APP_VERSION: &str = "0.1.0";
/// The generator name carried in every metadata document.
const APP_NAME: &str = "LOTUS PubChem Tree Generator";

/// All 12 tables fetched from Wikidata, in Python's `LOTUSData` order.
#[derive(Debug, Clone, Default)]
struct PubChemData {
    compound_taxon: Vec<Row>,
    taxon_ncbi: Vec<Row>,
    taxon_name: Vec<Row>,
    taxon_parent: Vec<Row>,
    compound_smiles_can: Vec<Row>,
    compound_smiles_iso: Vec<Row>,
    compound_smarts: Vec<Row>,
    compound_cxsmiles: Vec<Row>,
    compound_label: Vec<Row>,
    compound_parent: Vec<Row>,
    reference_doi: Vec<Row>,
    reference_pmid: Vec<Row>,
}

/// One compound's structure properties, keyed by compound QID.
#[derive(Debug, Clone, Default)]
struct CompoundProps {
    smiles_can: Option<String>,
    smiles_iso: Option<String>,
    smarts: Option<String>,
    cxsmiles: Option<String>,
    label: Option<String>,
    parent: Option<String>,
    inchikey: Option<String>,
}

/// One taxon's properties, keyed by taxon QID.
#[derive(Debug, Clone, Default)]
struct TaxonProps {
    name: Option<String>,
    ncbi: Option<String>,
    parent: Option<String>,
}

/// All of one reference's properties, keyed by reference QID.
#[derive(Debug, Clone, Default)]
struct ReferenceProps {
    doi: Option<String>,
    pmid: Option<String>,
}

/// Fetch all 12 Wikidata tables through the injected transport.
async fn fetch_all_pubchem_data<H: Http>(
    http: &H,
    endpoint: &Endpoint,
) -> Result<PubChemData, ExportError> {
    let compound_taxon = fetch_csv_rows(
        http,
        endpoint,
        &query_compound_inchikey_taxon(),
        "compound_taxon",
    )
    .await?;
    let taxon_ncbi = fetch_csv_rows(http, endpoint, &query_taxon_ncbi(), "taxon_ncbi").await?;
    let taxon_name = fetch_csv_rows(http, endpoint, &query_taxon_name(), "taxon_name").await?;
    let taxon_parent =
        fetch_csv_rows(http, endpoint, &query_taxon_parent(), "taxon_parent").await?;
    let compound_smiles_can = fetch_csv_rows(
        http,
        endpoint,
        &query_compound_smiles_can_all(),
        "compound_smiles_can",
    )
    .await?;
    let compound_smiles_iso = fetch_csv_rows(
        http,
        endpoint,
        &query_compound_smiles_iso_all(),
        "compound_smiles_iso",
    )
    .await?;
    let compound_smarts =
        fetch_csv_rows(http, endpoint, &query_compound_smarts(), "compound_smarts").await?;
    let compound_cxsmiles = fetch_csv_rows(
        http,
        endpoint,
        &query_compound_cxsmiles(),
        "compound_cxsmiles",
    )
    .await?;
    let compound_label =
        fetch_csv_rows(http, endpoint, &query_compound_label(), "compound_label").await?;
    let compound_parent =
        fetch_csv_rows(http, endpoint, &query_compound_parent(), "compound_parent").await?;
    let reference_doi =
        fetch_csv_rows(http, endpoint, &query_reference_doi(), "reference_doi").await?;
    let reference_pmid =
        fetch_csv_rows(http, endpoint, &query_reference_pmid(), "reference_pmid").await?;

    Ok(PubChemData {
        compound_taxon,
        taxon_ncbi,
        taxon_name,
        taxon_parent,
        compound_smiles_can,
        compound_smiles_iso,
        compound_smarts,
        compound_cxsmiles,
        compound_label,
        compound_parent,
        reference_doi,
        reference_pmid,
    })
}

/// Build `compound QID → CompoundProps` from the structure and label tables.
fn build_compound_props(data: &PubChemData) -> HashMap<String, CompoundProps> {
    let mut props: HashMap<String, CompoundProps> = HashMap::new();

    for row in &data.compound_smiles_can {
        let q = extract_qid(&cell(row, "compound")).to_string();
        props.entry(q.clone()).or_default().smiles_can = Some(cell(row, "compound_smiles_can"));
    }
    for row in &data.compound_smiles_iso {
        let q = extract_qid(&cell(row, "compound")).to_string();
        props.entry(q.clone()).or_default().smiles_iso = Some(cell(row, "compound_smiles_iso"));
    }
    for row in &data.compound_smarts {
        let q = extract_qid(&cell(row, "compound")).to_string();
        props.entry(q.clone()).or_default().smarts = Some(cell(row, "compound_smarts"));
    }
    for row in &data.compound_cxsmiles {
        let q = extract_qid(&cell(row, "compound")).to_string();
        props.entry(q.clone()).or_default().cxsmiles = Some(cell(row, "compound_cxsmiles"));
    }
    for row in &data.compound_label {
        let q = extract_qid(&cell(row, "compound")).to_string();
        props.entry(q.clone()).or_default().label = Some(cell(row, "compound_label"));
    }
    for row in &data.compound_parent {
        let q = extract_qid(&cell(row, "compound")).to_string();
        let p = extract_qid(&cell(row, "compound_parent")).to_string();
        props.entry(q.clone()).or_default().parent = Some(p);
    }
    for row in &data.compound_taxon {
        let q = extract_qid(&cell(row, "compound")).to_string();
        let ik = cell(row, "compound_inchikey");
        if !ik.is_empty() {
            props.entry(q.clone()).or_default().inchikey = Some(ik);
        }
    }

    props
}

/// Build `taxon QID → TaxonProps` from the taxon tables.
fn build_taxon_props(data: &PubChemData) -> HashMap<String, TaxonProps> {
    let mut props: HashMap<String, TaxonProps> = HashMap::new();
    for row in &data.taxon_name {
        let q = extract_qid(&cell(row, "taxon")).to_string();
        props.entry(q.clone()).or_default().name = Some(cell(row, "taxon_name"));
    }
    for row in &data.taxon_ncbi {
        let q = extract_qid(&cell(row, "taxon")).to_string();
        props.entry(q.clone()).or_default().ncbi = Some(cell(row, "taxon_ncbi"));
    }
    for row in &data.taxon_parent {
        let q = extract_qid(&cell(row, "taxon")).to_string();
        let p = extract_qid(&cell(row, "taxon_parent")).to_string();
        props.entry(q.clone()).or_default().parent = Some(p);
    }
    props
}

/// The value of one column, empty when absent.
fn cell(row: &Row, column: &str) -> String {
    row.get(column).cloned().unwrap_or_default()
}

/// Identify compounds that have `InChIKey`s AND are associated with taxa.
fn build_compounds_with_taxa(
    data: &PubChemData,
) -> (HashSet<String>, HashMap<String, Vec<String>>) {
    let mut inchikey_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut compounds_with_taxa: HashSet<String> = HashSet::new();

    for row in &data.compound_taxon {
        let q = extract_qid(&cell(row, "compound")).to_string();
        let ik = cell(row, "compound_inchikey");
        if ik.is_empty() {
            continue;
        }
        inchikey_map.entry(q.clone()).or_default().push(ik);
        compounds_with_taxa.insert(q);
    }

    (compounds_with_taxa, inchikey_map)
}

/// Build the unified descriptor map: compound QID → descriptor fields.
///
/// Each field is a single string when there is exactly one value, otherwise a
/// JSON array of strings. Empty fields are omitted.
fn build_descriptor_map(
    props: &HashMap<String, CompoundProps>,
    inchikey_map: &HashMap<String, Vec<String>>,
) -> HashMap<String, Value> {
    let mut map: HashMap<String, Value> = HashMap::new();

    for (qid, p) in props {
        let mut fields = Map::new();

        let mut inchikeys: Vec<String> = inchikey_map.get(qid).cloned().unwrap_or_default();
        inchikeys.sort_unstable();
        inchikeys.dedup();
        if !inchikeys.is_empty() {
            fields.insert("InChIKey".to_string(), simplify(&inchikeys));
        }
        let mut smiles: Vec<String> = [p.smiles_iso.clone(), p.smiles_can.clone()]
            .into_iter()
            .flatten()
            .collect();
        smiles.sort_unstable();
        smiles.dedup();
        if !smiles.is_empty() {
            fields.insert("SMILES".to_string(), simplify(&smiles));
        }
        if let Some(s) = p.smarts.as_deref().filter(|s| !s.is_empty()) {
            fields.insert("SMARTS".to_string(), Value::String(s.to_string()));
        }
        if let Some(c) = p.cxsmiles.as_deref().filter(|c| !c.is_empty()) {
            fields.insert("CXSMILES".to_string(), Value::String(c.to_string()));
        }

        if !fields.is_empty() {
            map.insert(qid.clone(), Value::Object(fields));
        }
    }

    map
}

/// `single value when unique, list when multiple`.
fn simplify(values: &[String]) -> Value {
    match values.len() {
        0 => Value::Null,
        1 => Value::String(values.first().cloned().unwrap_or_default()),
        _ => Value::Array(values.iter().map(|v| Value::String(v.clone())).collect()),
    }
}

/// Index references by `(compound, taxon)` → reference QID → props.
fn build_reference_lookup(
    data: &PubChemData,
) -> HashMap<(String, String), HashMap<String, ReferenceProps>> {
    let mut lookup: HashMap<(String, String), HashMap<String, ReferenceProps>> = HashMap::new();

    for row in &data.reference_doi {
        let compound = extract_qid(&cell(row, "compound")).to_string();
        let taxon = extract_qid(&cell(row, "taxon")).to_string();
        let ref_qid = extract_qid(&cell(row, "reference")).to_string();
        let e = lookup.entry((compound, taxon)).or_default();
        e.entry(ref_qid).or_default().doi = Some(cell(row, "doi"));
    }
    for row in &data.reference_pmid {
        let compound = extract_qid(&cell(row, "compound")).to_string();
        let taxon = extract_qid(&cell(row, "taxon")).to_string();
        let ref_qid = extract_qid(&cell(row, "reference")).to_string();
        let e = lookup.entry((compound, taxon)).or_default();
        e.entry(ref_qid).or_default().pmid = Some(cell(row, "pmid"));
    }

    lookup
}

/// References for one `(compound, taxon)` pair, sorted by DOI then PMID.
fn references_for(
    lookup: &HashMap<(String, String), HashMap<String, ReferenceProps>>,
    compound: &str,
    taxon: &str,
) -> Option<Value> {
    let refs = lookup.get(&(compound.to_string(), taxon.to_string()))?;
    if refs.is_empty() {
        return None;
    }
    let mut list: Vec<Value> = refs
        .iter()
        .map(|(ref_qid, props)| {
            let mut obj = Map::new();
            obj.insert("QID".to_string(), Value::String(ref_qid.clone()));
            if let Some(doi) = props.doi.as_deref().filter(|doi| !doi.is_empty()) {
                obj.insert("DOI".to_string(), Value::String(doi.to_string()));
            }
            if let Some(pmid) = props.pmid.as_deref().filter(|pmid| !pmid.is_empty()) {
                obj.insert("PMID".to_string(), Value::String(pmid.to_string()));
            }
            Value::Object(obj)
        })
        .collect();
    list.sort_by(|a, b| {
        let a_doi = a.get("DOI").and_then(|v| v.as_str()).unwrap_or_default();
        let b_doi = b.get("DOI").and_then(|v| v.as_str()).unwrap_or_default();
        let a_pmid = a.get("PMID").and_then(|v| v.as_str()).unwrap_or_default();
        let b_pmid = b.get("PMID").and_then(|v| v.as_str()).unwrap_or_default();
        a_doi.cmp(b_doi).then(a_pmid.cmp(b_pmid))
    });
    Some(Value::Array(list))
}

/// One tree node: `{"Name": ..., "Identifiers": {"QID": ...}}`.
fn node(name: &str, qid: &str) -> Value {
    let mut obj = Map::new();
    obj.insert("Name".to_string(), Value::String(name.to_string()));
    let mut ident = Map::new();
    ident.insert("QID".to_string(), Value::String(qid.to_string()));
    obj.insert("Identifiers".to_string(), Value::Object(ident));
    Value::Object(obj)
}

/// Add an optional `NCBI` `TaxID` to a taxon node's identifiers.
fn with_ncbi(node: &mut Value, ncbi: Option<&str>) {
    let Some(n) = ncbi else { return };
    if n.is_empty() {
        return;
    }
    let Some(ident) = node.get_mut("Identifiers").and_then(|v| v.as_object_mut()) else {
        return;
    };
    ident.insert("NCBI_TaxID".to_string(), Value::String(n.to_string()));
}

/// Add descriptors to a compound node.
fn with_descriptors(node: &mut Value, descriptors: Option<&Value>) {
    let Some(d) = descriptors else { return };
    let Some(m) = d.as_object() else { return };
    if m.is_empty() {
        return;
    }
    let Some(obj) = node.as_object_mut() else {
        return;
    };
    obj.insert("Descriptors".to_string(), d.clone());
}

/// Add children to a node.
fn with_children(node: &mut Value, children: Vec<Value>) {
    if children.is_empty() {
        return;
    }
    let Some(obj) = node.as_object_mut() else {
        return;
    };
    obj.insert("Children".to_string(), Value::Array(children));
}

/// Add compounds to a taxon node.
fn with_compounds(node: &mut Value, compounds: Vec<Value>) {
    if compounds.is_empty() {
        return;
    }
    let Some(obj) = node.as_object_mut() else {
        return;
    };
    obj.insert("Compounds".to_string(), Value::Array(compounds));
}

/// Sort an array of node objects by their `Name` field.
fn sort_nodes(mut nodes: Vec<Value>) -> Vec<Value> {
    nodes.sort_by(|a, b| {
        let a_name = a.get("Name").and_then(|v| v.as_str()).unwrap_or_default();
        let b_name = b.get("Name").and_then(|v| v.as_str()).unwrap_or_default();
        a_name.cmp(b_name)
    });
    nodes
}

/// Build one compound node dict (name, identifiers, optional descriptors).
fn compound_node<'a>(
    qid: &'a str,
    props: &'a CompoundProps,
    descriptors: Option<&'a Value>,
) -> Value {
    let name = props.label.as_deref().unwrap_or(qid);
    let mut n = node(name, qid);
    with_descriptors(&mut n, descriptors);
    n
}

/// Add references to a compound node for a specific `(compound, taxon)` pair.
fn with_compound_references(
    node: &mut Value,
    lookup: &HashMap<(String, String), HashMap<String, ReferenceProps>>,
    compound: &str,
    taxon: &str,
) {
    let Some(refs) = references_for(lookup, compound, taxon) else {
        return;
    };
    let Some(obj) = node.as_object_mut() else {
        return;
    };
    obj.insert("References".to_string(), refs);
}

/// Parameters shared across `build_biological_tree_node` calls.
struct BiologicalNodeCtx<'a> {
    visited: &'a mut HashSet<String>,
    relevant: &'a HashSet<String>,
    parent_to_children: &'a HashMap<String, Vec<String>>,
    taxon_to_compounds: &'a HashMap<String, Vec<String>>,
    props: &'a HashMap<String, CompoundProps>,
    taxon_props: &'a HashMap<String, TaxonProps>,
    descriptors: &'a HashMap<String, Value>,
    lookup: &'a HashMap<(String, String), HashMap<String, ReferenceProps>>,
    compounds_with_taxa: &'a HashSet<String>,
}

/// Build one biological-tree node for a taxon QID.
fn build_biological_tree_node(taxon_qid: &str, ctx: &mut BiologicalNodeCtx<'_>) -> Option<Value> {
    if !ctx.visited.insert(taxon_qid.to_string()) {
        return None;
    }

    let compound_qids = ctx
        .taxon_to_compounds
        .get(taxon_qid)
        .cloned()
        .unwrap_or_default();
    let mut compounds: Vec<Value> = Vec::new();
    for qid in &compound_qids {
        if !ctx.compounds_with_taxa.contains(qid) {
            continue;
        }
        let default = CompoundProps::default();
        let p = ctx.props.get(qid).unwrap_or(&default);
        let d = ctx.descriptors.get(qid);
        let mut c = compound_node(qid, p, d);
        with_compound_references(&mut c, ctx.lookup, qid, taxon_qid);
        compounds.push(c);
    }

    let children_qids = ctx
        .parent_to_children
        .get(taxon_qid)
        .cloned()
        .unwrap_or_default();
    let mut child_nodes: Vec<Value> = Vec::new();
    for child in &children_qids {
        if !ctx.relevant.contains(child) {
            continue;
        }
        if let Some(node) = build_biological_tree_node(child, ctx) {
            child_nodes.push(node);
        }
    }

    if compounds.is_empty() && child_nodes.is_empty() {
        return None;
    }

    let name = ctx
        .taxon_props
        .get(taxon_qid)
        .and_then(|t| t.name.as_deref())
        .unwrap_or(taxon_qid);
    let ncbi = ctx
        .taxon_props
        .get(taxon_qid)
        .and_then(|t| t.ncbi.as_deref());
    let mut n = node(name, taxon_qid);
    with_ncbi(&mut n, ncbi);
    with_compounds(&mut n, compounds);
    with_children(&mut n, sort_nodes(child_nodes));

    Some(n)
}

/// Parameters shared across `build_compound_tree_node` calls.
struct CompoundNodeCtx<'a> {
    visited: &'a mut HashSet<String>,
    parent_to_children: &'a HashMap<String, Vec<String>>,
    props: &'a HashMap<String, CompoundProps>,
    descriptors: &'a HashMap<String, Value>,
    compounds_with_taxa: &'a HashSet<String>,
}

/// Build one chemical-tree node for a compound QID.
fn build_compound_tree_node(compound_qid: &str, ctx: &mut CompoundNodeCtx<'_>) -> Option<Value> {
    if !ctx.visited.insert(compound_qid.to_string()) {
        return None;
    }

    let has_taxa = ctx.compounds_with_taxa.contains(compound_qid);

    let children_qids = ctx
        .parent_to_children
        .get(compound_qid)
        .cloned()
        .unwrap_or_default();
    let mut child_nodes: Vec<Value> = Vec::new();
    for child in &children_qids {
        if let Some(node) = build_compound_tree_node(child, ctx) {
            child_nodes.push(node);
        }
    }

    if !has_taxa && child_nodes.is_empty() {
        return None;
    }

    let default = CompoundProps::default();
    let p = ctx.props.get(compound_qid).unwrap_or(&default);
    let d = ctx.descriptors.get(compound_qid);
    let mut n = compound_node(compound_qid, p, d);
    with_children(&mut n, sort_nodes(child_nodes));

    Some(n)
}

/// Build the biological tree: taxa under Biota (P171), compounds as leaves.
fn build_biological_tree(
    data: &PubChemData,
    props: &HashMap<String, CompoundProps>,
    taxon_props: &HashMap<String, TaxonProps>,
    compounds_with_taxa: &HashSet<String>,
    descriptors: &HashMap<String, Value>,
    lookup: &HashMap<(String, String), HashMap<String, ReferenceProps>>,
) -> Vec<Value> {
    let mut taxon_to_compounds: HashMap<String, Vec<String>> = HashMap::new();
    for row in &data.compound_taxon {
        let compound = extract_qid(&cell(row, "compound")).to_string();
        let taxon = extract_qid(&cell(row, "taxon")).to_string();
        taxon_to_compounds.entry(taxon).or_default().push(compound);
    }

    let mut child_to_parent: HashMap<String, String> = HashMap::new();
    let mut parent_to_children: HashMap<String, Vec<String>> = HashMap::new();
    for row in &data.taxon_parent {
        let child = extract_qid(&cell(row, "taxon")).to_string();
        let parent = extract_qid(&cell(row, "taxon_parent")).to_string();
        child_to_parent.insert(child.clone(), parent.clone());
        parent_to_children.entry(parent).or_default().push(child);
    }

    let mut relevant: HashSet<String> = HashSet::new();
    for taxon in taxon_to_compounds.keys() {
        let mut current = taxon.clone();
        while !relevant.contains(&current) {
            relevant.insert(current.clone());
            match child_to_parent.get(&current) {
                Some(parent) => current = parent.clone(),
                None => break,
            }
        }
    }

    let roots: Vec<String> = relevant
        .iter()
        .filter(|t| {
            child_to_parent
                .get(*t)
                .is_none_or(|p| !relevant.contains(p))
        })
        .cloned()
        .collect();

    let mut tree: Vec<Value> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut ctx = BiologicalNodeCtx {
        visited: &mut visited,
        relevant: &relevant,
        parent_to_children: &parent_to_children,
        taxon_to_compounds: &taxon_to_compounds,
        props,
        taxon_props,
        descriptors,
        lookup,
        compounds_with_taxa,
    };

    let root_nodes: Vec<Value> = roots
        .iter()
        .map(|r| {
            let name = taxon_props
                .get(r)
                .and_then(|t| t.name.as_deref())
                .unwrap_or(r);
            node(name, r)
        })
        .collect();
    for root in sort_nodes(root_nodes) {
        let qid = root.get("QID").and_then(|v| v.as_str()).unwrap_or_default();
        if let Some(node) = build_biological_tree_node(qid, &mut ctx) {
            tree.push(node);
        }
    }

    sort_nodes(tree)
}

/// Build the chemical tree: compounds under chemical compound (Q11173) via P279.
fn build_compound_tree(
    data: &PubChemData,
    props: &HashMap<String, CompoundProps>,
    descriptors: &HashMap<String, Value>,
    compounds_with_taxa: &HashSet<String>,
) -> Vec<Value> {
    let mut child_to_parent: HashMap<String, String> = HashMap::new();
    let mut parent_to_children: HashMap<String, Vec<String>> = HashMap::new();
    for row in &data.compound_parent {
        let child = extract_qid(&cell(row, "compound")).to_string();
        let parent = extract_qid(&cell(row, "compound_parent")).to_string();
        child_to_parent.insert(child.clone(), parent.clone());
        parent_to_children.entry(parent).or_default().push(child);
    }

    let all_compounds: HashSet<String> = child_to_parent.keys().cloned().collect();
    let all_parents: HashSet<String> = parent_to_children.keys().cloned().collect();
    let roots: Vec<String> = all_parents.difference(&all_compounds).cloned().collect();

    let mut tree: Vec<Value> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut ctx = CompoundNodeCtx {
        visited: &mut visited,
        parent_to_children: &parent_to_children,
        props,
        descriptors,
        compounds_with_taxa,
    };

    for root in &roots {
        if let Some(node) = build_compound_tree_node(root, &mut ctx) {
            tree.push(node);
        }
    }

    sort_nodes(tree)
}

/// Get an arbitrary URL's text body (`NPClassifier` cache CSV).
async fn http_get_text<H: Http>(http: &H, url: &str) -> Result<String, ExportError> {
    let response = http.get(url, "text/csv").await?;
    let text = response
        .text()
        .await
        .map_err(|e| ExportError::Fetch(lotus_search::FetchError::Network(e.to_string())))?;
    Ok(text)
}

/// Parse `NPClassifier` CSV rows into `pathway → superclass → class → set[smiles]`.
fn parse_npclassifier_rows(
    rows: &[HashMap<String, String>],
) -> HashMap<String, HashMap<String, HashMap<String, HashSet<String>>>> {
    let mut tree_data: HashMap<String, HashMap<String, HashMap<String, HashSet<String>>>> =
        HashMap::new();

    for row in rows {
        let pathway_raw = row.get("pathway").cloned().unwrap_or_default();
        let superclass_raw = row.get("superclass").cloned().unwrap_or_default();
        let class_raw = row.get("class").cloned().unwrap_or_default();
        let smiles = row.get("smiles").cloned().unwrap_or_default();
        let error = row.get("error").cloned().unwrap_or_default();

        if !error.is_empty() || pathway_raw.is_empty() {
            continue;
        }

        let pathways: Vec<&str> = pathway_raw
            .split(" $ ")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let superclasses: Vec<&str> = superclass_raw
            .split(" $ ")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let classes: Vec<&str> = class_raw
            .split(" $ ")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        if smiles.is_empty() {
            continue;
        }

        for pathway in &pathways {
            let sc_map = tree_data.entry(pathway.to_string()).or_default();
            for (i, superclass) in superclasses.iter().enumerate() {
                let cls_map = sc_map.entry(superclass.to_string()).or_default();
                let cls = classes.get(i);
                if let Some(cls) = cls {
                    if cls.is_empty() {
                        cls_map
                            .entry("_unclassified".to_string())
                            .or_default()
                            .insert(smiles.clone());
                    } else {
                        cls_map
                            .entry(cls.to_string())
                            .or_default()
                            .insert(smiles.clone());
                    }
                } else {
                    cls_map
                        .entry("_unclassified".to_string())
                        .or_default()
                        .insert(smiles.clone());
                }
            }
        }
    }

    tree_data
}

/// Build `SMILES` → `InChIKey` and `SMILES` → `QID` from `compound_taxon` + `props`.
fn build_smiles_maps(
    data: &PubChemData,
    props: &HashMap<String, CompoundProps>,
    inchikey_map: &HashMap<String, Vec<String>>,
) -> (HashMap<String, Vec<String>>, HashMap<String, String>) {
    let mut smiles_to_inchikey: HashMap<String, Vec<String>> = HashMap::new();
    let mut smiles_to_qid: HashMap<String, String> = HashMap::new();

    for row in &data.compound_taxon {
        let qid = extract_qid(&cell(row, "compound")).to_string();
        let inchikeys = inchikey_map.get(&qid).cloned().unwrap_or_default();
        if inchikeys.is_empty() {
            continue;
        }
        let p = props.get(&qid);
        let smiles: Vec<String> = [
            p.and_then(|p| p.smiles_iso.clone()),
            p.and_then(|p| p.smiles_can.clone()),
        ]
        .into_iter()
        .flatten()
        .collect();
        for smi in smiles {
            let entry = smiles_to_inchikey.entry(smi.clone()).or_default();
            for ik in &inchikeys {
                if !entry.contains(ik) {
                    entry.push(ik.clone());
                }
            }
            smiles_to_qid.entry(smi).or_insert_with(|| qid.clone());
        }
    }

    for iks in smiles_to_inchikey.values_mut() {
        iks.sort_unstable();
        iks.dedup();
    }

    (smiles_to_inchikey, smiles_to_qid)
}

/// Build `compound` `QID` → label for `NPClassifier` naming.
fn build_label_map(data: &PubChemData) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for row in &data.compound_label {
        let q = extract_qid(&cell(row, "compound")).to_string();
        map.insert(q, cell(row, "compound_label"));
    }
    map
}

/// Build the `NPClassifier` tree: pathway → superclass → class → `InChIKey` compounds.
fn build_npclassifier_tree(
    npclassifier_rows: &[HashMap<String, String>],
    smiles_to_inchikey: &HashMap<String, Vec<String>>,
    smiles_to_qid: &HashMap<String, String>,
    label_map: &HashMap<String, String>,
) -> Vec<Value> {
    if npclassifier_rows.is_empty() {
        return Vec::new();
    }

    let tree_data = parse_npclassifier_rows(npclassifier_rows);
    if tree_data.is_empty() {
        return Vec::new();
    }

    let mut tree: Vec<Value> = Vec::new();
    let mut pathways: Vec<String> = tree_data.keys().cloned().collect();
    pathways.sort();

    for pathway in pathways {
        let Some(sc_map) = tree_data.get(&pathway) else {
            continue;
        };
        let mut pathway_node = node(&pathway, &pathway);
        {
            if let Some(ident) = pathway_node
                .get_mut("Identifiers")
                .and_then(|v| v.as_object_mut())
            {
                ident.insert(
                    "NPClassifier_Pathway".to_string(),
                    Value::String(pathway.clone()),
                );
            }
        }
        let mut superclass_nodes: Vec<Value> = Vec::new();
        let mut superclasses: Vec<String> = sc_map.keys().cloned().collect();
        superclasses.sort();

        for superclass in superclasses {
            let Some(cls_map) = sc_map.get(&superclass) else {
                continue;
            };
            if let Some(superclass_node) = build_superclass_node(
                &superclass,
                cls_map,
                smiles_to_inchikey,
                smiles_to_qid,
                label_map,
            ) {
                superclass_nodes.push(superclass_node);
            }
        }

        if !superclass_nodes.is_empty() {
            with_children(&mut pathway_node, sort_nodes(superclass_nodes));
            tree.push(pathway_node);
        }
    }

    sort_nodes(tree)
}

/// Build one `NPClassifier` superclass node: superclass → class → `InChIKey` compounds.
fn build_superclass_node(
    superclass: &str,
    cls_map: &HashMap<String, HashSet<String>>,
    smiles_to_inchikey: &HashMap<String, Vec<String>>,
    smiles_to_qid: &HashMap<String, String>,
    label_map: &HashMap<String, String>,
) -> Option<Value> {
    let mut superclass_node = node(superclass, superclass);
    {
        if let Some(ident) = superclass_node
            .get_mut("Identifiers")
            .and_then(|v| v.as_object_mut())
        {
            ident.insert(
                "NPClassifier_Superclass".to_string(),
                Value::String(superclass.to_string()),
            );
        }
    }
    let mut class_nodes: Vec<Value> = Vec::new();

    let mut classes: Vec<String> = cls_map.keys().cloned().collect();
    classes.sort();

    for cls in classes {
        let Some(smiles_set) = cls_map.get(&cls) else {
            continue;
        };
        if cls == "_unclassified" {
            let mut smiles_list: Vec<String> = smiles_set.iter().cloned().collect();
            smiles_list.sort();
            for smi in smiles_list {
                if let Some(leaf) =
                    npclassifier_leaf(&smi, smiles_to_inchikey, smiles_to_qid, label_map)
                {
                    class_nodes.push(leaf);
                }
            }
        } else {
            let mut class_node = node(&cls, &cls);
            {
                if let Some(ident) = class_node
                    .get_mut("Identifiers")
                    .and_then(|v| v.as_object_mut())
                {
                    ident.insert("NPClassifier_Class".to_string(), Value::String(cls.clone()));
                }
            }
            let mut smiles_list: Vec<String> = smiles_set.iter().cloned().collect();
            smiles_list.sort();
            for smi in smiles_list {
                let Some(leaf) =
                    npclassifier_leaf(&smi, smiles_to_inchikey, smiles_to_qid, label_map)
                else {
                    continue;
                };
                let Some(obj) = class_node.as_object_mut() else {
                    continue;
                };
                let children = obj
                    .entry("Children".to_string())
                    .or_insert_with(|| Value::Array(Vec::new()));
                let Some(children_arr) = children.as_array_mut() else {
                    continue;
                };
                children_arr.push(leaf);
            }
            class_nodes.push(class_node);
        }
    }

    // Keep only class nodes that have children, plus bare leaves.
    let mut final_class_nodes: Vec<Value> = Vec::new();
    for c in class_nodes {
        let is_class = c
            .get("Identifiers")
            .and_then(|v| v.get("NPClassifier_Class"))
            .is_some();
        if is_class {
            if c.get("Children")
                .is_some_and(|v| v.as_array().is_some_and(|a| !a.is_empty()))
            {
                final_class_nodes.push(c);
            }
        } else {
            final_class_nodes.push(c);
        }
    }

    if !final_class_nodes.is_empty() {
        with_children(&mut superclass_node, sort_nodes(final_class_nodes));
    }

    if superclass_node.get("Children").is_some() {
        Some(superclass_node)
    } else {
        None
    }
}

/// One `NPClassifier` compound leaf: `InChIKey` + `SMILES` (+ `QID`).
fn npclassifier_leaf(
    smi: &str,
    smiles_to_inchikey: &HashMap<String, Vec<String>>,
    smiles_to_qid: &HashMap<String, String>,
    label_map: &HashMap<String, String>,
) -> Option<Value> {
    let inchikeys = smiles_to_inchikey.get(smi)?;
    if inchikeys.is_empty() {
        return None;
    }
    let qid = smiles_to_qid.get(smi);
    let compound_name = qid
        .and_then(|q| label_map.get(q))
        .cloned()
        .unwrap_or_else(|| smi.to_string());

    let mut obj = Map::new();
    obj.insert("Name".to_string(), Value::String(compound_name));
    let mut ident = Map::new();
    if let Some(q) = qid {
        ident.insert("QID".to_string(), Value::String(q.clone()));
    }
    obj.insert("Identifiers".to_string(), Value::Object(ident));

    let mut desc = Map::new();
    desc.insert("InChIKey".to_string(), simplify(inchikeys));
    desc.insert("SMILES".to_string(), Value::String(smi.to_string()));
    obj.insert("Descriptors".to_string(), Value::Object(desc));

    Some(Value::Object(obj))
}

/// The full output document for one tree: documentation + metadata + tree.
fn full_document(tree_type: &str, tree: &[Value], generated: &str, npclassifier: bool) -> Value {
    let is_biological = tree_type == "biological";
    let (overview, description, notes, root_constraint, classification, source_name) =
        if npclassifier {
            (
                "This JSON contains a hierarchical tree of chemical compounds classified using NPClassifier (pathway → superclass → class).",
                "NPClassifier-based hierarchical classification of natural products",
                vec![
                    "NPClassifier provides a comprehensive classification for natural products.",
                    "Hierarchy: pathway → superclass → class → InChIKey",
                    "Multiple classifications are possible for a single compound.",
                    "See https://npclassifier.gnps2.org/ for more information.",
                ],
                "NPClassifier pathways",
                "NPClassifier",
                NPCLASSIFIER_CACHE_URL,
            )
        } else if is_biological {
            (
                "This JSON contains a hierarchical tree of biological taxa with their associated natural product compounds.",
                "Hierarchical taxonomy of biological organisms with associated natural product compounds",
                vec![
                    "Descriptor values are strings when single, arrays when multiple values exist (very rare).",
                    "All nodes are sorted alphabetically by Name.",
                    "Only nodes with InChIKey associations (directly or via descendants) are included.",
                    "Data queried from Wikidata via QLever SPARQL endpoint.",
                ],
                "Biota (Q2382443)",
                "Wikidata",
                QLEVER_WIKIDATA,
            )
        } else {
            (
                "This JSON contains a hierarchical tree of chemical compound classes with structural descriptors based on Wikidata P279 relationships.",
                "Wikidata-based hierarchical classification of chemical compounds",
                vec![
                    "Note: Wikidata P279 (subclass of) relationships are sparse for natural products.",
                    "Consider using the NPClassifier-based tree (lotus_chemical_tree.json) for better coverage.",
                    "Descriptor values are strings when single, arrays when multiple values exist (very rare).",
                    "All nodes are sorted alphabetically by Name.",
                ],
                "chemical compound (Q11173)",
                "Wikidata",
                QLEVER_WIKIDATA,
            )
        };

    let total_nodes = count_tree_nodes(tree);

    let documentation = build_documentation_map(overview, notes, is_biological);

    let meta_params = MetaParams {
        tree_type,
        description,
        generated,
        npclassifier,
        root_constraint,
        classification,
        source_name,
    };
    let metadata = build_metadata_map(&meta_params, is_biological, tree, total_nodes);

    let mut doc = Map::new();
    doc.insert("_documentation".to_string(), Value::Object(documentation));
    doc.insert("metadata".to_string(), Value::Object(metadata));
    doc.insert("tree".to_string(), Value::Array(tree.to_vec()));

    Value::Object(doc)
}

/// Build the documentation map for a tree output document.
fn build_documentation_map(
    overview: &str,
    notes: Vec<&str>,
    is_biological: bool,
) -> Map<String, Value> {
    let mut documentation = Map::new();
    documentation.insert("overview".to_string(), Value::String(overview.to_string()));
    let mut structure = Map::new();
    structure.insert(
        "tree".to_string(),
        Value::String(
            "Array of root nodes. Each node is an object with Name, Identifiers, and optional Children."
                .to_string(),
        ),
    );
    let mut node_fields = Map::new();
    node_fields.insert(
        "Name".to_string(),
        Value::String("Human-readable name (taxon name or compound label)".to_string()),
    );
    node_fields.insert(
        "Identifiers".to_string(),
        Value::String(
            "External database identifiers (Wikidata QID, NCBI TaxID for taxa, NPClassifier levels)"
                .to_string(),
        ),
    );
    if is_biological {
        node_fields.insert(
            "Compounds".to_string(),
            Value::String(
                "(Biological tree only) Array of compounds found in this taxon".to_string(),
            ),
        );
        node_fields.insert(
            "References".to_string(),
            Value::String(
                "(Biological tree compounds only) Literature references for compound-taxon association"
                    .to_string(),
            ),
        );
    }
    node_fields.insert(
        "Descriptors".to_string(),
        Value::String("Chemical structure representations (InChIKey, SMILES, etc.)".to_string()),
    );
    node_fields.insert(
        "Children".to_string(),
        Value::String("Array of child nodes (same structure, recursive)".to_string()),
    );
    structure.insert("node_fields".to_string(), Value::Object(node_fields));
    let mut desc_fields = Map::new();
    desc_fields.insert(
        "InChIKey".to_string(),
        Value::String("IUPAC International Chemical Identifier Key".to_string()),
    );
    desc_fields.insert(
        "SMILES".to_string(),
        Value::String(
            "Simplified Molecular Input Line Entry System (isomeric preferred over canonical)"
                .to_string(),
        ),
    );
    desc_fields.insert(
        "SMARTS".to_string(),
        Value::String("SMILES Arbitrary Target Specification (substructure patterns)".to_string()),
    );
    desc_fields.insert(
        "CXSMILES".to_string(),
        Value::String("ChemAxon Extended SMILES".to_string()),
    );
    structure.insert("descriptors_fields".to_string(), Value::Object(desc_fields));
    let mut ref_fields = Map::new();
    ref_fields.insert(
        "QID".to_string(),
        Value::String("QID of the reference article in Wikidata".to_string()),
    );
    ref_fields.insert(
        "DOI".to_string(),
        Value::String("Digital Object Identifier of the reference".to_string()),
    );
    ref_fields.insert(
        "PMID".to_string(),
        Value::String("PubMed ID of the reference".to_string()),
    );
    structure.insert("reference_fields".to_string(), Value::Object(ref_fields));
    documentation.insert("structure".to_string(), Value::Object(structure));
    documentation.insert(
        "notes".to_string(),
        Value::Array(
            notes
                .into_iter()
                .map(|s| Value::String(s.to_string()))
                .collect(),
        ),
    );
    documentation
}

/// Parameters for building the metadata map of a tree output document.
struct MetaParams<'a> {
    tree_type: &'a str,
    description: &'a str,
    generated: &'a str,
    npclassifier: bool,
    root_constraint: &'a str,
    classification: &'a str,
    source_name: &'a str,
}

/// Build the metadata map for a tree output document.
fn build_metadata_map(
    params: &MetaParams<'_>,
    is_biological: bool,
    tree: &[Value],
    total_nodes: usize,
) -> Map<String, Value> {
    let mut metadata = Map::new();
    let mut name = format!("LOTUS {} Tree", params.tree_type);
    if params.npclassifier {
        name.push_str(" (NPClassifier)");
    } else if !is_biological {
        name.push_str(" (Wikidata)");
    }
    metadata.insert("name".to_string(), Value::String(name));
    metadata.insert(
        "description".to_string(),
        Value::String(params.description.to_string()),
    );
    metadata.insert(
        "version".to_string(),
        Value::String(APP_VERSION.to_string()),
    );
    metadata.insert(
        "generated".to_string(),
        Value::String(params.generated.to_string()),
    );
    metadata.insert("generator".to_string(), Value::String(APP_NAME.to_string()));
    let mut source_obj = Map::new();
    source_obj.insert("name".to_string(), Value::String("LOTUS".to_string()));
    source_obj.insert(
        "url".to_string(),
        Value::String("https://lotus.nprod.net/".to_string()),
    );
    source_obj.insert(
        "wikidata_item".to_string(),
        Value::String("Q104225190".to_string()),
    );
    source_obj.insert(
        "endpoint".to_string(),
        Value::String(params.source_name.to_string()),
    );
    source_obj.insert(
        "classification".to_string(),
        Value::String(params.classification.to_string()),
    );
    metadata.insert("source".to_string(), Value::Object(source_obj));
    let mut license = Map::new();
    license.insert(
        "data".to_string(),
        Value::String("AGPL-3.0-only".to_string()),
    );
    license.insert(
        "code".to_string(),
        Value::String("AGPL-3.0-only".to_string()),
    );
    metadata.insert("license".to_string(), Value::Object(license));
    let mut constraints = Map::new();
    constraints.insert(
        "root".to_string(),
        Value::String(params.root_constraint.to_string()),
    );
    metadata.insert("constraints".to_string(), Value::Object(constraints));
    let mut stats = Map::new();
    stats.insert("root_nodes".to_string(), Value::Number(tree.len().into()));
    stats.insert("total_nodes".to_string(), Value::Number(total_nodes.into()));
    metadata.insert("statistics".to_string(), Value::Object(stats));
    metadata
}

/// Count every node in a tree (recursive).
fn count_tree_nodes(tree: &[Value]) -> usize {
    let mut total = 0;
    for node in tree {
        total += 1;
        if let Some(children) = node.get("Children").and_then(|v| v.as_array()) {
            total += count_tree_nodes(children);
        }
    }
    total
}

/// Insert a recursive child conversion result into `node_content` under the
/// `"children"` key. Shared by `biological_to_pubchem` and `chemical_to_pubchem`,
/// which differ only in the conversion function they pass.
fn insert_children_result(
    child_nodes: &[Value],
    convert: impl Fn(&[Value]) -> Value,
    node_content: &mut Map<String, Value>,
) {
    let child_result = convert(child_nodes);
    if let Some(child_children) = child_result.as_object().and_then(|o| o.get("children")) {
        node_content.insert("children".to_string(), child_children.clone());
    }
}

/// Convert the biological tree to `PubChem` compact format (`name`-as-key).
fn biological_to_pubchem(tree: &[Value]) -> Value {
    if tree.is_empty() {
        return Value::Object(Map::new());
    }

    let mut children = Map::new();

    for node in tree {
        let name = node
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let ident = node.get("Identifiers").and_then(|v| v.as_object());
        let taxon_qid = ident.and_then(|i| i.get("QID").and_then(|v| v.as_str()));
        let ncbi_id = ident.and_then(|i| i.get("NCBI_TaxID").and_then(|v| v.as_str()));

        let mut node_content = Map::new();
        if let Some(q) = taxon_qid.filter(|q| !q.is_empty()) {
            node_content.insert("QID".to_string(), Value::String(q.to_string()));
        }

        if let Some(child_nodes) = node.get("Children").and_then(|v| v.as_array()) {
            insert_children_result(child_nodes, biological_to_pubchem, &mut node_content);
        }

        if let (Some(ncbi), Some(ncbi_entry)) = (
            ncbi_id.filter(|n| !n.is_empty()),
            build_ncbi_entry(node, name),
        ) {
            if !node_content.contains_key("children") {
                node_content.insert("children".to_string(), Value::Object(Map::new()));
            }
            if let Some(children_map) = node_content
                .get_mut("children")
                .and_then(|v| v.as_object_mut())
            {
                children_map.insert(ncbi.to_string(), Value::Object(ncbi_entry));
            }
        }

        let final_content = if node_content.is_empty() {
            Value::Object(Map::new())
        } else {
            Value::Object(node_content)
        };
        children.insert(name.to_string(), final_content);
    }

    let mut result = Map::new();
    result.insert("children".to_string(), Value::Object(children));
    Value::Object(result)
}

/// Build one `NCBI` taxon entry for `PubChem` compact format from a biological tree node.
fn build_ncbi_entry(node: &Value, name: &str) -> Option<Map<String, Value>> {
    let compounds = node.get("Compounds").and_then(|v| v.as_array())?;
    let mut compounds_dict: Map<String, Value> = Map::new();
    let mut ref_keys: HashMap<String, HashSet<(String, String)>> = HashMap::new();

    for compound in compounds {
        let c_ident = compound.get("Identifiers").and_then(|v| v.as_object());
        let c_qid = c_ident.and_then(|i| i.get("QID").and_then(|v| v.as_str()));
        let c_name = compound
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let c_descriptors = compound.get("Descriptors").and_then(|v| v.as_object());
        let inchikey_val = c_descriptors.and_then(|d| d.get("InChIKey"));
        let refs = compound.get("References").and_then(|v| v.as_array());

        if let Some(ik) = inchikey_val {
            let iks = match ik {
                Value::String(s) => vec![s.clone()],
                Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
                _ => Vec::new(),
            };
            for ik_str in iks {
                let entry = compounds_dict
                    .entry(ik_str.clone())
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Some(e) = entry.as_object_mut() {
                    if !c_name.is_empty() && !e.contains_key("Name") {
                        e.insert("Name".to_string(), Value::String(c_name.to_string()));
                    }
                    if let Some(q) = c_qid.filter(|q| !q.is_empty() && !e.contains_key("QID")) {
                        e.insert("QID".to_string(), Value::String(q.to_string()));
                    }
                    if let Some(ref_arr) = refs {
                        let keys = ref_keys.entry(ik_str.clone()).or_default();
                        let mut refs_out: Vec<Value> = e
                            .get("references")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        for ref_val in ref_arr {
                            let doi = ref_val
                                .get("DOI")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            let pmid = ref_val
                                .get("PMID")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            if keys.insert((doi.clone(), pmid.clone())) {
                                let mut ref_entry = Map::new();
                                if !doi.is_empty() {
                                    ref_entry.insert("DOI".to_string(), Value::String(doi));
                                }
                                if !pmid.is_empty() {
                                    ref_entry.insert("PMID".to_string(), Value::String(pmid));
                                }
                                if !ref_entry.is_empty() {
                                    refs_out.push(Value::Object(ref_entry));
                                }
                            }
                        }
                        e.insert("references".to_string(), Value::Array(refs_out));
                    }
                }
            }
        }
    }

    if compounds_dict.is_empty() {
        return None;
    }

    let mut ncbi_entry = Map::new();
    ncbi_entry.insert(
        "organism_name".to_string(),
        Value::Array(vec![Value::String(name.to_string())]),
    );
    ncbi_entry.insert("compounds".to_string(), Value::Object(compounds_dict));
    Some(ncbi_entry)
}

/// Convert the chemical tree to `PubChem` compact format.
fn chemical_to_pubchem(tree: &[Value]) -> Value {
    if tree.is_empty() {
        return Value::Object(Map::new());
    }

    let mut children = Map::new();

    for node in tree {
        let name = node
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let ident = node.get("Identifiers").and_then(|v| v.as_object());
        let qid = ident.and_then(|i| i.get("QID").and_then(|v| v.as_str()));
        let descriptors = node.get("Descriptors").and_then(|v| v.as_object());
        let inchikey = descriptors.and_then(|d| d.get("InChIKey"));

        let mut node_content = Map::new();
        if let Some(q) = qid.filter(|q| !q.is_empty()) {
            node_content.insert("QID".to_string(), Value::String(q.to_string()));
        }

        if let Some(child_nodes) = node.get("Children").and_then(|v| v.as_array()) {
            insert_children_result(child_nodes, chemical_to_pubchem, &mut node_content);
        }

        if let Some(ik) = inchikey {
            node_content
                .entry("children".to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            let iks = match ik {
                Value::String(s) => vec![s.clone()],
                Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
                _ => Vec::new(),
            };
            for ik_str in iks {
                let mut leaf = Map::new();
                if !name.is_empty() && name != "Unknown" {
                    leaf.insert("Name".to_string(), Value::String(name.to_string()));
                }
                if let Some(q) = qid.filter(|q| !q.is_empty()) {
                    leaf.insert("QID".to_string(), Value::String(q.to_string()));
                }
                if let Some(children_map) = node_content
                    .get_mut("children")
                    .and_then(|v| v.as_object_mut())
                {
                    children_map.insert(ik_str, Value::Object(leaf));
                }
            }
        }

        let final_content = if node_content.is_empty() {
            Value::Object(Map::new())
        } else {
            Value::Object(node_content)
        };
        children.insert(name.to_string(), final_content);
    }

    let mut result = Map::new();
    result.insert("children".to_string(), Value::Object(children));
    Value::Object(result)
}

/// Convert `NPClassifier` tree to `PubChem` compact format.
fn npclassifier_to_pubchem(tree: &[Value]) -> Value {
    let mut children = Map::new();

    for pathway in tree {
        let p_name = pathway
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let mut p_content = Map::new();
        if let Some(p_children) = pathway.get("Children").and_then(|v| v.as_array()) {
            let mut sc_map = Map::new();
            for sc in p_children {
                let sc_name = sc.get("Name").and_then(|v| v.as_str()).unwrap_or_default();
                let mut sc_content = Map::new();
                if let Some(sc_children) = sc.get("Children").and_then(|v| v.as_array()) {
                    let mut cls_map = Map::new();
                    for cls in sc_children {
                        let cls_name = cls.get("Name").and_then(|v| v.as_str()).unwrap_or_default();
                        if cls_name == "_unclassified" {
                            if let Some(leaves) = cls.get("Children").and_then(|v| v.as_array()) {
                                for leaf in leaves {
                                    add_npclassifier_leaf_to_map(leaf, &mut sc_content);
                                }
                            }
                        } else if let Some(cls_children) =
                            cls.get("Children").and_then(|v| v.as_array())
                        {
                            for leaf in cls_children {
                                add_npclassifier_leaf_to_map(leaf, &mut cls_map);
                            }
                            if !cls_map.is_empty() {
                                sc_content
                                    .insert(cls_name.to_string(), Value::Object(cls_map.clone()));
                            }
                        }
                    }
                    if !sc_content.is_empty() {
                        sc_map.insert(sc_name.to_string(), Value::Object(sc_content));
                    }
                }
            }
            if !sc_map.is_empty() {
                p_content.insert("children".to_string(), Value::Object(sc_map));
            }
        }
        if !p_content.is_empty() {
            children.insert(p_name.to_string(), Value::Object(p_content));
        }
    }

    let mut result = Map::new();
    result.insert("children".to_string(), Value::Object(children));
    Value::Object(result)
}

/// Add one `NPClassifier` compound leaf to a name-as-key map, keyed by `InChIKey`.
fn add_npclassifier_leaf_to_map(leaf: &Value, target: &mut Map<String, Value>) {
    let l_name = leaf
        .get("Name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let l_ident = leaf.get("Identifiers").and_then(|v| v.as_object());
    let l_qid = l_ident.and_then(|i| i.get("QID").and_then(|v| v.as_str()));
    let l_desc = leaf.get("Descriptors").and_then(|v| v.as_object());
    let l_ik = l_desc.and_then(|d| d.get("InChIKey"));

    if let Some(ik) = l_ik {
        let iks = match ik {
            Value::String(s) => vec![s.clone()],
            Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        for ik_str in iks {
            let mut leaf_content = Map::new();
            if !l_name.is_empty() && l_name != "Unknown" {
                leaf_content.insert("Name".to_string(), Value::String(l_name.to_string()));
            }
            if let Some(q) = l_qid.filter(|q| !q.is_empty()) {
                leaf_content.insert("QID".to_string(), Value::String(q.to_string()));
            }
            target.insert(ik_str, Value::Object(leaf_content));
        }
    }
}

/// Run the full `PubChem` tree pipeline: fetch, index, build three trees, and
/// serialize each in both full and `PubChem` compact formats.
///
/// Returns (`biological_full`, `chemical_full`, `npclassifier_full`,
/// `biological_pubchem`, `chemical_pubchem`, `npclassifier_pubchem`) as JSON strings.
/// `npclassifier_*` are empty strings when the cache could not be read.
///
/// # Errors
///
/// Returns an error if fetching from the endpoint or cache fails.
pub async fn run_pubchem_tree<H: Http>(
    http: &H,
    endpoint: Endpoint,
    now: &str,
) -> Result<(String, String, String, String, String, String), ExportError> {
    let data = fetch_all_pubchem_data(http, &endpoint).await?;

    let props = build_compound_props(&data);
    let taxon_props = build_taxon_props(&data);
    let (compounds_with_taxa, inchikey_map) = build_compounds_with_taxa(&data);
    let descriptors = build_descriptor_map(&props, &inchikey_map);
    let lookup = build_reference_lookup(&data);

    let biological_tree = build_biological_tree(
        &data,
        &props,
        &taxon_props,
        &compounds_with_taxa,
        &descriptors,
        &lookup,
    );
    let chemical_tree = build_compound_tree(&data, &props, &descriptors, &compounds_with_taxa);

    let npclassifier_rows = http_get_text(http, NPCLASSIFIER_CACHE_URL)
        .await
        .map_or_else(|_| Vec::new(), |text| parse_npclassifier_csv(&text));

    let label_map = build_label_map(&data);
    let (smiles_to_inchikey, smiles_to_qid) = build_smiles_maps(&data, &props, &inchikey_map);
    let npclassifier_tree = build_npclassifier_tree(
        &npclassifier_rows,
        &smiles_to_inchikey,
        &smiles_to_qid,
        &label_map,
    );

    let generated = now.to_string();

    let ser = |v: &Value| -> Result<String, ExportError> {
        serde_json::to_string_pretty(v)
            .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))
    };

    let biological_full = ser(&full_document(
        "biological",
        &biological_tree,
        &generated,
        false,
    ))?;
    let chemical_full = ser(&full_document(
        "chemical",
        &chemical_tree,
        &generated,
        false,
    ))?;
    let npclassifier_full = if npclassifier_tree.is_empty() {
        String::new()
    } else {
        ser(&full_document(
            "chemical",
            &npclassifier_tree,
            &generated,
            true,
        ))?
    };

    let biological_pubchem = ser(&biological_to_pubchem(&biological_tree))?;
    let chemical_pubchem = ser(&chemical_to_pubchem(&chemical_tree))?;
    let npclassifier_pubchem = if npclassifier_tree.is_empty() {
        String::new()
    } else {
        ser(&npclassifier_to_pubchem(&npclassifier_tree))?
    };

    Ok((
        biological_full,
        chemical_full,
        npclassifier_full,
        biological_pubchem,
        chemical_pubchem,
        npclassifier_pubchem,
    ))
}

/// Parse a `NPClassifier` CSV text into rows (column name → value).
fn parse_npclassifier_csv(text: &str) -> Vec<HashMap<String, String>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers: Vec<String> = match reader.headers() {
        Ok(h) => h.iter().map(ToString::to_string).collect(),
        Err(_) => return Vec::new(),
    };
    let mut rows: Vec<HashMap<String, String>> = Vec::new();
    for result in reader.records() {
        let Ok(record) = result else { continue };
        let mut row = HashMap::new();
        for (i, header) in headers.iter().enumerate() {
            row.insert(
                header.clone(),
                record.get(i).unwrap_or_default().to_string(),
            );
        }
        rows.push(row);
    }
    rows
}

#[cfg(test)]
#[path = "pubchem_tree/tests.rs"]
mod tests;
