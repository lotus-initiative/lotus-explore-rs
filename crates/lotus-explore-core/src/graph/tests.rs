// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for the pure-Rust network graph engine.

use super::*;
use lotus_model::{ColumnarResultSet, CompoundEntry};
use std::sync::Arc;

fn make_row(
    compound_qid: &str,
    compound_name: &str,
    taxon_qid: &str,
    taxon_name: &str,
    ref_qid: &str,
    ref_title: Option<&str>,
) -> CompoundEntry {
    CompoundEntry {
        compound_qid: Arc::from(compound_qid),
        name: Arc::from(compound_name),
        inchikey: None,
        smiles: None,
        mass: Some(302.24),
        formula: Some(Arc::from("C15H10O7")),
        taxon_qid: Arc::from(taxon_qid),
        taxon_name: Arc::from(taxon_name),
        reference_qid: Arc::from(ref_qid),
        reference_node: Arc::from(""),
        ref_title: ref_title.map(Arc::from),
        ref_doi: None,
        pub_year: Some(2021),
        statement: None,
    }
}

fn all_rows(set: &ColumnarResultSet) -> Vec<u32> {
    (0..set.row_count())
        .filter_map(|i| u32::try_from(i).ok())
        .collect()
}

fn sample_corroboration_entries() -> Vec<CompoundEntry> {
    vec![
        // Same (C1, T10) pair reported twice in SAME reference R100 -> weight 1,
        // and once more in a DIFFERENT reference R200 -> weight 2.
        make_row(
            "Q1",
            "Gentiopicroside",
            "Q10",
            "Gentiana lutea",
            "Q100",
            Some("Paper A"),
        ),
        make_row(
            "Q1",
            "Gentiopicroside",
            "Q10",
            "Gentiana lutea",
            "Q100",
            Some("Paper A"),
        ),
        make_row(
            "Q1",
            "Gentiopicroside",
            "Q10",
            "Gentiana lutea",
            "Q200",
            Some("Paper B"),
        ),
        // (C2, T10) with NO reference across two rows -> falls back to occurrence count 2.
        make_row("Q2", "Amarogentin", "Q10", "Gentiana lutea", "", None),
        make_row("Q2", "Amarogentin", "Q10", "Gentiana lutea", "", None),
        // (C1, T20) reported once with R300 -> weight 1.
        make_row(
            "Q1",
            "Gentiopicroside",
            "Q20",
            "Gentiana acaulis",
            "Q300",
            Some("Paper C"),
        ),
    ]
}

#[test]
fn builds_compound_taxon_graph_with_deduplicated_corroboration_weights() {
    let entries = sample_corroboration_entries();
    let set = ColumnarResultSet::from_entries(&entries);
    let rows = all_rows(&set);

    let model = build_graph(
        &set,
        &rows,
        GraphProjection::CompoundTaxon,
        GraphScope::Overview { max_nodes: 100 },
    );

    assert_eq!(
        model.effective_projection,
        GraphProjection::CompoundTaxon,
        "explicit CompoundTaxon projection must stay CompoundTaxon"
    );
    assert_eq!(
        model.nodes.len(),
        4,
        "expected 2 compounds + 2 taxa in CompoundTaxon mode"
    );
    assert_eq!(model.edges.len(), 3, "expected 3 deduplicated edges");
    assert!(
        !model.capped,
        "4 nodes must not be marked capped under limit 100"
    );
    assert_eq!(
        model.empty_reason, None,
        "non-empty graph has no empty reason"
    );
    assert_eq!(
        model.kind_count(NodeKind::Compound),
        2,
        "two compound vertices"
    );
    assert_eq!(model.kind_count(NodeKind::Taxon), 2, "two taxon vertices");
    assert_eq!(
        model.kind_count(NodeKind::Reference),
        0,
        "no reference vertices in CompoundTaxon"
    );
    assert_eq!(
        model.available_kind_count(NodeKind::Compound),
        2,
        "two distinct compounds in filtered rows"
    );
    assert_eq!(
        model.available_kind_count(NodeKind::Taxon),
        2,
        "two distinct taxa in filtered rows"
    );
    assert_eq!(
        model.available_kind_count(NodeKind::Reference),
        3,
        "all three distinct references remain reported via available_kind_count"
    );

    let c1 = model
        .index_of(NodeKey::new(NodeKind::Compound, 1))
        .unwrap_or(usize::MAX);
    let c2 = model
        .index_of(NodeKey::new(NodeKind::Compound, 2))
        .unwrap_or(usize::MAX);
    let t10 = model
        .index_of(NodeKey::new(NodeKind::Taxon, 10))
        .unwrap_or(usize::MAX);
    let t20 = model
        .index_of(NodeKey::new(NodeKind::Taxon, 20))
        .unwrap_or(usize::MAX);

    let weight_between = |u: usize, v: usize| -> Option<u32> {
        let (a, b) = if u <= v { (u, v) } else { (v, u) };
        model
            .edges
            .iter()
            .find(|e| e.source as usize == a && e.target as usize == b)
            .map(|e| e.weight)
    };

    assert_eq!(
        weight_between(c1, t10),
        Some(2),
        "C1-T10 weight must equal distinct corroborating references (2, not 3 rows)"
    );
    assert_eq!(
        weight_between(c2, t10),
        Some(2),
        "C2-T10 without references falls back to occurrence count (2)"
    );
    assert_eq!(
        weight_between(c1, t20),
        Some(1),
        "C1-T20 has one corroborating reference"
    );
}

#[test]
fn auto_projection_picks_two_mode_projection_with_structure() {
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 12, 1, 8),
        GraphProjection::CompoundReference,
        "single taxon with multiple compounds and references should project CompoundReference"
    );
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 1, 9, 5),
        GraphProjection::TaxonReference,
        "single compound with multiple taxa and references should project TaxonReference"
    );
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 4, 3, 5),
        GraphProjection::CompoundTaxon,
        "multiple compounds and multiple taxa should project CompoundTaxon"
    );
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 1, 1, 1),
        GraphProjection::CompoundTaxon,
        "single entity on each axis falls back to CompoundTaxon"
    );
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 1, 1, 4),
        GraphProjection::CompoundTaxon,
        "one compound and one taxon with references falls back to CompoundTaxon"
    );
    assert_eq!(
        resolve_projection(GraphProjection::Tripartite, 12, 1, 8),
        GraphProjection::Tripartite,
        "explicit projection must override Auto heuristics"
    );
}

#[test]
fn caps_nodes_deterministically_with_no_dangling_edges() {
    let entries = vec![
        make_row("Q1", "Comp1", "Q10", "Taxon10", "Q100", Some("Ref100")),
        make_row("Q1", "Comp1", "Q11", "Taxon11", "Q100", Some("Ref100")),
        make_row("Q1", "Comp1", "Q12", "Taxon12", "Q100", Some("Ref100")),
        make_row("Q2", "Comp2", "Q10", "Taxon10", "Q100", Some("Ref100")),
        make_row("Q3", "Comp3", "Q12", "Taxon12", "Q100", Some("Ref100")),
    ];
    let set = ColumnarResultSet::from_entries(&entries);
    let rows = all_rows(&set);

    let model = build_graph(
        &set,
        &rows,
        GraphProjection::CompoundTaxon,
        GraphScope::Overview { max_nodes: 3 },
    );

    assert!(
        model.capped,
        "6 participating nodes capped to 3 must set capped = true"
    );
    assert_eq!(
        model.total_distinct_nodes, 6,
        "total_distinct_nodes reports uncapped count"
    );
    assert_eq!(
        model.total_distinct_edges, 5,
        "total_distinct_edges reports uncapped count"
    );
    assert_eq!(model.nodes.len(), 3, "overview cap keeps exactly max_nodes");
    for edge in &model.edges {
        assert!(
            (edge.source as usize) < model.nodes.len()
                && (edge.target as usize) < model.nodes.len(),
            "retained edge endpoints must never dangle outside capped node list"
        );
    }

    // Re-running with the same input produces identical deterministic order and coordinates.
    let repeat = build_graph(
        &set,
        &rows,
        GraphProjection::CompoundTaxon,
        GraphScope::Overview { max_nodes: 3 },
    );
    assert_eq!(model, repeat, "graph projection must be deterministic");
}

#[test]
fn structure_only_rows_yield_no_occurrence_edges_reason() {
    let entries = vec![
        make_row("Q1", "Comp1", "", "", "", None),
        make_row("Q2", "Comp2", "", "", "", None),
    ];
    let set = ColumnarResultSet::from_entries(&entries);
    let rows = all_rows(&set);

    let model = build_graph(
        &set,
        &rows,
        GraphProjection::Auto,
        GraphScope::Overview { max_nodes: 100 },
    );

    assert!(model.nodes.is_empty(), "no fake hub nodes may be invented");
    assert!(model.edges.is_empty(), "structure-only rows have no edges");
    assert_eq!(
        model.empty_reason,
        Some(GraphEmptyReason::NoOccurrenceEdges),
        "must explain that rows lack occurrence links"
    );
    assert_eq!(model.available_kind_count(NodeKind::Compound), 2);
    assert_eq!(model.available_kind_count(NodeKind::Taxon), 0);

    let no_rows = build_graph(&set, &[], GraphProjection::Auto, GraphScope::default());
    assert_eq!(
        no_rows.empty_reason,
        Some(GraphEmptyReason::NoRows),
        "empty row slice must yield NoRows"
    );
    assert_eq!(no_rows.available_kind_count(NodeKind::Compound), 0);
}

#[test]
fn tripartite_projection_links_all_three_axes() {
    let entries = vec![
        make_row("Q1", "Comp1", "Q10", "Taxon10", "Q100", Some("Ref100")),
        make_row("Q2", "Comp2", "Q10", "Taxon10", "Q100", Some("Ref100")),
    ];
    let set = ColumnarResultSet::from_entries(&entries);
    let rows = all_rows(&set);

    let model = build_graph(
        &set,
        &rows,
        GraphProjection::Tripartite,
        GraphScope::Overview { max_nodes: 100 },
    );

    assert_eq!(
        model.effective_projection,
        GraphProjection::Tripartite,
        "effective projection is Tripartite"
    );
    assert_eq!(model.nodes.len(), 4, "2 compounds + 1 taxon + 1 reference");
    assert_eq!(
        model.edges.len(),
        5,
        "expected C1-T10, C2-T10, C1-R100, C2-R100, T10-R100"
    );
}

#[test]
fn search_all_nodes_finds_entities_outside_cap_and_ego_focuses_exact_neighborhood() {
    let entries = vec![
        // Hubs C1 and C2 each connect to T10, T11, T12 (all 5 nodes have degree >= 2)
        make_row("Q1", "HubCompound", "Q10", "TaxonA", "Q100", Some("RefA")),
        make_row("Q1", "HubCompound", "Q11", "TaxonB", "Q100", Some("RefA")),
        make_row("Q1", "HubCompound", "Q12", "TaxonC", "Q100", Some("RefA")),
        make_row("Q2", "HubTwo", "Q10", "TaxonA", "Q100", Some("RefA")),
        make_row("Q2", "HubTwo", "Q11", "TaxonB", "Q100", Some("RefA")),
        make_row("Q2", "HubTwo", "Q12", "TaxonC", "Q100", Some("RefA")),
        // Low-degree pair outside a 3-node overview cap: Q99 ("RareAlkaloid") <-> Q98 ("RareMoss") (degree 1)
        make_row(
            "Q99",
            "RareAlkaloid",
            "Q98",
            "RareMoss",
            "Q200",
            Some("RefRare"),
        ),
    ];
    let set = ColumnarResultSet::from_entries(&entries);
    let rows = all_rows(&set);

    let overview = build_graph(
        &set,
        &rows,
        GraphProjection::CompoundTaxon,
        GraphScope::Overview { max_nodes: 3 },
    );
    let rare_key = NodeKey::new(NodeKind::Compound, 99);
    assert_eq!(
        overview.index_of(rare_key),
        None,
        "RareAlkaloid should sit outside the 3-node overview cap"
    );

    let matches = search_all_nodes(&set, &rows, "rarealk", 10);
    assert_eq!(
        matches.len(),
        1,
        "global node search must find RareAlkaloid outside overview cap"
    );
    assert_eq!(
        matches.first().map(|m| m.key),
        Some(rare_key),
        "match key must identify Q99"
    );

    let qid_matches = search_all_nodes(&set, &rows, "Q98", 10);
    assert_eq!(
        qid_matches.first().map(|m| m.key),
        Some(NodeKey::new(NodeKind::Taxon, 98)),
        "global node search must match bare QIDs"
    );

    let ego = build_graph(
        &set,
        &rows,
        GraphProjection::CompoundTaxon,
        GraphScope::Ego {
            center: rare_key,
            max_neighbors: 50,
        },
    );
    assert_eq!(
        ego.nodes.len(),
        2,
        "ego scope around RareAlkaloid keeps center + RareMoss"
    );
    assert_eq!(
        ego.nodes.first().map(|n| n.key),
        Some(rare_key),
        "ego center must sit at index 0"
    );
    assert_eq!(ego.edges.len(), 1, "ego scope keeps exact 1-hop edge");

    let missing_ego = build_graph(
        &set,
        &rows,
        GraphProjection::CompoundTaxon,
        GraphScope::Ego {
            center: NodeKey::new(NodeKind::Compound, 99999),
            max_neighbors: 50,
        },
    );
    assert_eq!(
        missing_ego.empty_reason,
        Some(GraphEmptyReason::CenterNotInRows),
        "missing ego center must yield CenterNotInRows"
    );
}

#[test]
fn payload_parallel_arrays_and_neighbors_match_model() {
    let entries = vec![
        make_row("Q1", "Alpha", "Q10", "TaxonOne", "Q100", Some("Ref1")),
        make_row("Q1", "Alpha", "Q10", "TaxonOne", "Q101", Some("Ref2")),
        make_row("Q1", "Alpha", "Q11", "TaxonTwo", "Q100", Some("Ref1")),
    ];
    let set = ColumnarResultSet::from_entries(&entries);
    let rows = all_rows(&set);
    let model = build_graph(
        &set,
        &rows,
        GraphProjection::CompoundTaxon,
        GraphScope::Overview { max_nodes: 100 },
    );

    let payload = model.to_payload();
    let n = model.nodes.len();
    let e = model.edges.len();
    assert_eq!(
        payload.kinds.len(),
        n,
        "kinds array length matches node count"
    );
    assert_eq!(
        payload.radii.len(),
        n,
        "radii array length matches node count"
    );
    assert_eq!(
        payload.degrees.len(),
        n,
        "degrees array length matches node count"
    );
    assert_eq!(payload.init_pos.len(), n * 3, "init_pos holds xyz per node");
    assert_eq!(
        payload.labels.len(),
        n,
        "labels array length matches node count"
    );
    assert_eq!(
        payload.hub_index.len(),
        n,
        "hub_index array length matches node count"
    );
    assert_eq!(
        payload.edges.len(),
        e * 2,
        "edges array holds two endpoints per edge"
    );
    assert_eq!(
        payload.edge_weights.len(),
        e,
        "edge_weights length matches edge count"
    );
    assert!(
        !payload.top_hubs.is_empty(),
        "non-empty graph reports top_hubs"
    );

    let c1_idx = model
        .index_of(NodeKey::new(NodeKind::Compound, 1))
        .unwrap_or(usize::MAX);
    let neighbors = model.neighbors_of(c1_idx);
    assert_eq!(neighbors.len(), 2, "C1 has two taxon neighbors");
    assert_eq!(
        neighbors.first().map(|(_, w)| *w),
        Some(2),
        "heaviest neighbor T10 (weight 2) sorts first"
    );
}

#[test]
fn pivot_criteria_and_starters_are_exact_and_valid() {
    let year = 2026;
    let c = pivot_criteria_for_node(NodeKind::Compound, "Q121802", year);
    assert_eq!(
        c.structure, "Q121802",
        "compound pivot sets structure to QID"
    );
    assert_eq!(c.taxon, "*", "compound pivot unlocks all taxa with '*'");
    assert!(
        lotus_model::validate_criteria(&c, year).is_ok(),
        "compound pivot must validate"
    );

    let t = pivot_criteria_for_node(NodeKind::Taxon, "Q158572", year);
    assert_eq!(t.taxon, "Q158572", "taxon pivot sets taxon to QID");
    assert!(t.structure.is_empty(), "taxon pivot leaves structure empty");
    assert!(
        lotus_model::validate_criteria(&t, year).is_ok(),
        "taxon pivot must validate"
    );

    let r = pivot_criteria_for_node(NodeKind::Reference, "Q34460861", year);
    assert_eq!(
        r.reference, "Q34460861",
        "reference pivot sets reference to QID"
    );
    assert!(r.taxon.is_empty(), "reference pivot leaves taxon empty");
    assert!(
        lotus_model::validate_criteria(&r, year).is_ok(),
        "reference pivot must validate"
    );

    for starter in NETWORK_STARTERS {
        let sc = starter.to_criteria(year);
        assert!(
            sc.is_searchable(),
            "starter {} must be searchable",
            starter.id
        );
        assert!(
            lotus_model::validate_criteria(&sc, year).is_ok(),
            "starter {} must validate cleanly",
            starter.id
        );
    }
}

#[test]
fn graph_projection_as_str_and_parse_round_trip() {
    for proj in [
        GraphProjection::Auto,
        GraphProjection::CompoundTaxon,
        GraphProjection::CompoundReference,
        GraphProjection::TaxonReference,
        GraphProjection::Tripartite,
    ] {
        assert_eq!(
            GraphProjection::parse(proj.as_str()),
            Some(proj),
            "projection {proj:?} must round-trip via as_str/parse"
        );
    }
    assert_eq!(
        GraphProjection::parse(" compound_taxon "),
        Some(GraphProjection::CompoundTaxon),
        "trimmed projection tag must parse"
    );
    assert_eq!(
        GraphProjection::parse("unknown"),
        None,
        "invalid projection must return None"
    );
}

#[test]
fn search_all_nodes_matches_inchikey_doi_and_handles_empty_edges() {
    let entries = vec![
        CompoundEntry {
            inchikey: Some(Arc::from("GRWFGVWFFZKLTI-UHFFFAOYSA-N")),
            ref_doi: Some(Arc::from("10.1234/pine.42")),
            ..make_row("Q10", "Pinene", "Q20", "Pinus", "Q30", Some("Pinene paper"))
        },
        CompoundEntry {
            inchikey: Some(Arc::from("XMGQYMWWDOXHJM-UHFFFAOYSA-N")),
            ref_doi: Some(Arc::from("10.1234/citrus.99")),
            ..make_row(
                "Q11",
                "Limonene",
                "Q21",
                "Citrus",
                "Q31",
                Some("Citrus paper"),
            )
        },
    ];

    let set = ColumnarResultSet::from_entries(&entries);
    let rows = all_rows(&set);

    let ik_hits = search_all_nodes(&set, &rows, "grwfgvwffzklti", 10);
    assert_eq!(
        ik_hits.len(),
        1,
        "InChIKey prefix must match single compound"
    );
    assert_eq!(
        ik_hits.first().map(|m| m.key),
        Some(NodeKey::new(NodeKind::Compound, 10))
    );

    let doi_hits = search_all_nodes(&set, &rows, "10.1234/citrus.99", 10);
    assert_eq!(doi_hits.len(), 1, "DOI must match its Reference node");
    assert_eq!(
        doi_hits.first().map(|m| m.key),
        Some(NodeKey::new(NodeKind::Reference, 31))
    );

    assert!(
        search_all_nodes(&set, &rows, "   ", 10).is_empty(),
        "blank query returns empty"
    );
    assert!(
        search_all_nodes(&set, &rows, "Pinene", 0).is_empty(),
        "limit 0 returns empty"
    );
    assert!(
        search_all_nodes(&set, &[], "Pinene", 10).is_empty(),
        "empty rows returns empty"
    );
}

#[test]
fn graph_projection_kind_membership_omission_and_toggle_cover_all_axes() {
    assert!(GraphProjection::Auto.includes_kind(NodeKind::Compound));
    assert!(GraphProjection::Auto.includes_kind(NodeKind::Taxon));
    assert!(GraphProjection::Auto.includes_kind(NodeKind::Reference));
    assert_eq!(GraphProjection::Auto.omitted_kind(), None);
    assert_eq!(
        GraphProjection::toggle_kind(GraphProjection::Auto, NodeKind::Compound),
        None
    );

    assert!(GraphProjection::Tripartite.includes_kind(NodeKind::Compound));
    assert!(GraphProjection::Tripartite.includes_kind(NodeKind::Taxon));
    assert!(GraphProjection::Tripartite.includes_kind(NodeKind::Reference));
    assert_eq!(GraphProjection::Tripartite.omitted_kind(), None);
    assert_eq!(
        GraphProjection::toggle_kind(GraphProjection::Tripartite, NodeKind::Reference),
        Some(GraphProjection::CompoundTaxon)
    );
    assert_eq!(
        GraphProjection::toggle_kind(GraphProjection::Tripartite, NodeKind::Taxon),
        Some(GraphProjection::CompoundReference)
    );
    assert_eq!(
        GraphProjection::toggle_kind(GraphProjection::Tripartite, NodeKind::Compound),
        Some(GraphProjection::TaxonReference)
    );

    let pairs = [
        (
            GraphProjection::CompoundTaxon,
            NodeKind::Compound,
            NodeKind::Taxon,
            NodeKind::Reference,
        ),
        (
            GraphProjection::CompoundReference,
            NodeKind::Compound,
            NodeKind::Reference,
            NodeKind::Taxon,
        ),
        (
            GraphProjection::TaxonReference,
            NodeKind::Taxon,
            NodeKind::Reference,
            NodeKind::Compound,
        ),
    ];
    for (proj, a, b, omitted) in pairs {
        assert!(proj.includes_kind(a));
        assert!(proj.includes_kind(b));
        assert!(!proj.includes_kind(omitted));
        assert_eq!(proj.omitted_kind(), Some(omitted));
        assert_eq!(
            GraphProjection::toggle_kind(proj, omitted),
            Some(GraphProjection::Tripartite)
        );
        assert_eq!(GraphProjection::toggle_kind(proj, a), None);
        assert_eq!(GraphProjection::toggle_kind(proj, b), None);
    }
}

#[test]
fn node_kind_and_node_key_url_tokens_round_trip_and_reject_invalid() {
    for kind in [NodeKind::Compound, NodeKind::Taxon, NodeKind::Reference] {
        let tag = kind.as_str();
        assert_eq!(NodeKind::parse(tag), Some(kind));
        assert_eq!(
            NodeKind::parse(&format!(" {} ", tag.to_ascii_uppercase())),
            Some(kind)
        );

        let key = NodeKey::new(kind, 121_802);
        let token = key.to_url_token();
        assert_eq!(token, format!("{tag}-Q121802"));
        assert_eq!(NodeKey::parse_url_token(&token), Some(key));
    }

    assert_eq!(NodeKind::parse("unknown"), None);
    assert_eq!(
        NodeKey::parse_url_token("taxon:q158572"),
        Some(NodeKey::new(NodeKind::Taxon, 158_572))
    );
    assert_eq!(
        NodeKey::parse_url_token("reference-Q99"),
        Some(NodeKey::new(NodeKind::Reference, 99))
    );
    assert_eq!(
        GraphProjection::parse("TRIPARTITE"),
        Some(GraphProjection::Tripartite)
    );

    for bad in [
        "compound-Q0",
        "compound-Q+123",
        "compound-Q",
        "compound-121802",
        "unknown-Q1",
        "Q121802",
        "",
    ] {
        assert_eq!(
            NodeKey::parse_url_token(bad),
            None,
            "invalid token '{bad}' must return None"
        );
    }
}

#[test]
fn resolve_projection_payload_neighbors_and_node_layout_are_exact() {
    assert_eq!(NodeKind::Compound.as_u8(), 0);
    assert_eq!(NodeKind::Taxon.as_u8(), 1);
    assert_eq!(NodeKind::Reference.as_u8(), 2);

    assert_eq!(
        resolve_projection(GraphProjection::Auto, 2, 2, 5),
        GraphProjection::CompoundTaxon
    );
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 1, 1, 1),
        GraphProjection::CompoundTaxon
    );
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 1, 1, 2),
        GraphProjection::CompoundTaxon
    );
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 2, 1, 2),
        GraphProjection::CompoundReference
    );
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 1, 2, 2),
        GraphProjection::TaxonReference
    );
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 2, 1, 1),
        GraphProjection::CompoundTaxon
    );
    assert_eq!(
        resolve_projection(GraphProjection::Auto, 1, 2, 1),
        GraphProjection::CompoundTaxon
    );

    // Minimal 2-node overview: pins exact seeded_coords/splitmix64 & radius math.
    let pair = ColumnarResultSet::from_entries(&[make_row(
        "Q1",
        "Quercetin",
        "Q10",
        "Gentiana lutea",
        "Q100",
        Some("Paper A"),
    )]);
    let two_node = build_graph(
        &pair,
        &[0],
        GraphProjection::CompoundTaxon,
        GraphScope::Overview { max_nodes: 2 },
    );
    assert!(!two_node.capped, "max_nodes == node_count is not capped");
    assert_eq!(two_node.top_hubs.len(), 2);
    assert!(two_node.nodes.iter().all(|n| n.degree == 1));
    assert_eq!(two_node.neighbors_of(0), vec![(1, 1)]);
    assert_eq!(two_node.neighbors_of(1), vec![(0, 1)]);

    let payload = two_node.to_payload();
    assert_eq!(payload.kinds, vec![0, 1]);
    assert_eq!(payload.hub_index, vec![1, 2]);
    assert_eq!(payload.edges, vec![0, 1]);

    if let (Some(c0), Some(t1)) = (two_node.nodes.first(), two_node.nodes.get(1)) {
        assert_eq!(t1.label, "Gentiana lutea");
        assert!((c0.init_x - 71.574_01_f32).abs() < 1e-2, "{}", c0.init_x);
        assert!((c0.init_y - 41.657_79_f32).abs() < 1e-2, "{}", c0.init_y);
        assert!((c0.init_z - 6.130_126_f32).abs() < 1e-2, "{}", c0.init_z);
        assert!((t1.init_x + 61.412_77_f32).abs() < 1e-2, "{}", t1.init_x);
        assert!((t1.init_y + 41.484_66_f32).abs() < 1e-2, "{}", t1.init_y);
        assert!((t1.init_z - 22.550_97_f32).abs() < 1e-2, "{}", t1.init_z);
        let expected_r = 4.5_f32 + (2.0_f32.ln() * 2.4_f32).min(11.5_f32);
        assert!((c0.radius - expected_r).abs() < 1e-4, "{}", c0.radius);
    }
    assert_eq!(two_node.nodes.len(), 2);
}

#[test]
fn ego_graph_checks_exact_cap_boundaries_and_origin() {
    let pair = ColumnarResultSet::from_entries(&[make_row(
        "Q1",
        "Quercetin",
        "Q10",
        "Gentiana lutea",
        "Q100",
        Some("Paper A"),
    )]);

    // Ego exact-cap boundary and zero-centered hub origin.
    let ego_exact = build_graph(
        &pair,
        &[0],
        GraphProjection::CompoundTaxon,
        GraphScope::Ego {
            center: NodeKey::new(NodeKind::Compound, 1),
            max_neighbors: 1,
        },
    );
    assert!(
        !ego_exact.capped,
        "max_neighbors == neighbor_count is not capped"
    );
    let ego_under = build_graph(
        &pair,
        &[0],
        GraphProjection::CompoundTaxon,
        GraphScope::Ego {
            center: NodeKey::new(NodeKind::Compound, 1),
            max_neighbors: 2,
        },
    );
    assert!(
        !ego_under.capped,
        "max_neighbors > neighbor_count is not capped"
    );
    let ego_zero = build_graph(
        &pair,
        &[0],
        GraphProjection::CompoundTaxon,
        GraphScope::Ego {
            center: NodeKey::new(NodeKind::Compound, 1),
            max_neighbors: 0,
        },
    );
    assert!(
        ego_zero.capped,
        "max_neighbors < neighbor_count must mark capped"
    );
    if let (Some(center), Some(nb)) = (ego_exact.nodes.first(), ego_exact.nodes.get(1)) {
        assert!(
            center.init_x.abs() < 1e-6 && center.init_y.abs() < 1e-6 && center.init_z.abs() < 1e-6
        );
        assert!(nb.init_x.abs() > 1e-3 || nb.init_y.abs() > 1e-3 || nb.init_z.abs() > 1e-3);
    }
}

fn rich_metadata_set() -> ColumnarResultSet {
    ColumnarResultSet::from_entries(&[
        CompoundEntry {
            compound_qid: Arc::from("Q2"),
            name: Arc::from("Beta"),
            inchikey: Some(Arc::from("ABCDEFGHIJKLMN-OPQRSTUVWX-Y")),
            smiles: None,
            mass: None,
            formula: Some(Arc::from("C10H16")),
            taxon_qid: Arc::from("Q20"),
            taxon_name: Arc::from("   "),
            reference_qid: Arc::from("Q200"),
            reference_node: Arc::from(""),
            ref_title: Some(Arc::from("   ")),
            ref_doi: Some(Arc::from("10.1234/ALPHA")),
            pub_year: Some(2024),
            statement: None,
        },
        CompoundEntry {
            compound_qid: Arc::from("Q3"),
            name: Arc::from("Gamma"),
            inchikey: None,
            smiles: None,
            mass: Some(180.16),
            formula: Some(Arc::from("   ")),
            taxon_qid: Arc::from("Q20"),
            taxon_name: Arc::from("Taxus"),
            reference_qid: Arc::from("Q201"),
            reference_node: Arc::from(""),
            ref_title: Some(Arc::from("Study Title")),
            ref_doi: None,
            pub_year: None,
            statement: None,
        },
        CompoundEntry {
            compound_qid: Arc::from("Q4"),
            name: Arc::from("Delta"),
            inchikey: None,
            smiles: None,
            mass: None,
            formula: None,
            taxon_qid: Arc::from("Q20"),
            taxon_name: Arc::from("Taxus"),
            reference_qid: Arc::from("Q202"),
            reference_node: Arc::from(""),
            ref_title: None,
            ref_doi: Some(Arc::from(" ")),
            pub_year: Some(1999),
            statement: None,
        },
    ])
}

#[test]
fn build_graph_formats_secondary_metadata_and_searches_all_fields() {
    // Exercise secondary text formatting combinations, blank strings, and search_all_nodes paths.
    let rich = rich_metadata_set();
    let all = all_rows(&rich);
    let tri = build_graph(
        &rich,
        &all,
        GraphProjection::Tripartite,
        GraphScope::Overview { max_nodes: 20 },
    );
    assert_eq!(
        tri.nodes
            .iter()
            .find(|n| n.key == NodeKey::new(NodeKind::Compound, 2))
            .and_then(|n| n.secondary.as_deref()),
        Some("C10H16")
    );
    assert_eq!(
        tri.nodes
            .iter()
            .find(|n| n.key == NodeKey::new(NodeKind::Compound, 3))
            .and_then(|n| n.secondary.as_deref()),
        Some("180.16 Da")
    );
    assert_eq!(
        tri.nodes
            .iter()
            .find(|n| n.key == NodeKey::new(NodeKind::Compound, 4))
            .and_then(|n| n.secondary.as_deref()),
        None
    );
    assert_eq!(
        tri.nodes
            .iter()
            .find(|n| n.key == NodeKey::new(NodeKind::Reference, 200))
            .map(|n| (n.label.as_str(), n.secondary.as_deref())),
        Some(("10.1234/ALPHA", Some("2024")))
    );
    assert_eq!(
        tri.nodes
            .iter()
            .find(|n| n.key == NodeKey::new(NodeKind::Reference, 201))
            .map(|n| n.label.as_str()),
        Some("Study Title")
    );
    assert_eq!(
        tri.nodes
            .iter()
            .find(|n| n.key == NodeKey::new(NodeKind::Reference, 202))
            .map(|n| n.label.as_str()),
        Some("Q202")
    );

    assert_eq!(search_all_nodes(&rich, &all, "", 5), Vec::new());
    assert_eq!(search_all_nodes(&rich, &all, "bet", 0), Vec::new());
    assert_eq!(search_all_nodes(&rich, &[], "bet", 10), Vec::new());
    assert_eq!(
        search_all_nodes(&rich, &all, "C10H16", 5)
            .first()
            .map(|m| m.key),
        Some(NodeKey::new(NodeKind::Compound, 2))
    );
    assert_eq!(
        search_all_nodes(&rich, &all, "1234/alpha", 5)
            .first()
            .map(|m| m.key),
        Some(NodeKey::new(NodeKind::Reference, 200))
    );
    assert_eq!(
        search_all_nodes(&rich, &all, "gamm", 5)
            .first()
            .map(|m| m.key),
        Some(NodeKey::new(NodeKind::Compound, 3))
    );
}

#[test]
fn search_all_nodes_ranks_prefix_and_formula_matches_ahead_of_substrings() {
    // Prefix match (1 occurrence) must outrank substring-only higher occurrence (2 occurrences)
    // and secondary formula prefix match on `Compound` (no label prefix) must also mark starts_with.
    let ranked_set = ColumnarResultSet::from_entries(&[
        CompoundEntry {
            formula: Some(Arc::from("C8H10")),
            ..make_row("Q10", "abeta", "Q50", "TaxonOne", "Q500", None)
        },
        CompoundEntry {
            formula: Some(Arc::from("C8H10")),
            ..make_row("Q10", "abeta", "Q51", "TaxonTwo", "Q501", None)
        },
        CompoundEntry {
            formula: Some(Arc::from("C9H12")),
            ..make_row("Q11", "beta", "Q50", "TaxonOne", "Q500", None)
        },
        CompoundEntry {
            formula: Some(Arc::from("XC9H12")),
            ..make_row("Q12", "omega", "Q50", "TaxonOne", "Q500", None)
        },
        CompoundEntry {
            formula: Some(Arc::from("XC9H12")),
            ..make_row("Q12", "omega", "Q51", "TaxonTwo", "Q501", None)
        },
    ]);
    let ranked_rows = all_rows(&ranked_set);
    assert_eq!(
        search_all_nodes(&ranked_set, &ranked_rows, "beta", 5)
            .first()
            .map(|m| m.key),
        Some(NodeKey::new(NodeKind::Compound, 11))
    );
    assert_eq!(
        search_all_nodes(&ranked_set, &ranked_rows, "C9H12", 5)
            .first()
            .map(|m| m.key),
        Some(NodeKey::new(NodeKind::Compound, 11))
    );
}
