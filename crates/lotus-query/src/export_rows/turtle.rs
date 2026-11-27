// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! RDF/Turtle provenance triple formatting for [`RowExporter`](super::RowExporter).

use std::fmt::Write as _;

use lotus_model::{ColumnarResultSet, WIKIDATA_REFERENCE_BASE, WIKIDATA_STATEMENT_BASE};

use super::statement_text;

/// The Turtle preamble.
///
/// The same prefixes the endpoint's `CONSTRUCT` relies on, so a consumer that already
/// parses an endpoint export parses this one without change.
const PREFIXES: &str = "@prefix wd: <http://www.wikidata.org/entity/> .\n\
@prefix wdt: <http://www.wikidata.org/prop/direct/> .\n\
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
@prefix prov: <http://www.w3.org/ns/prov#> .\n\
@prefix p: <http://www.wikidata.org/prop/> .\n\
@prefix ps: <http://www.wikidata.org/prop/statement/> .\n\
@prefix pr: <http://www.wikidata.org/prop/reference/> .\n\
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\n";

pub(super) fn push_preamble(out: &mut String) {
    out.push_str(PREFIXES);
}

/// One occurrence as Turtle, mirroring the `CONSTRUCT` the endpoint would run.
pub(super) fn push_triples(out: &mut String, set: &ColumnarResultSet, row: usize) {
    let Some(compound) = set.compound_qid_text(row) else {
        // No compound QID means the row could not have been built; `push` drops
        // those, so reaching here would mean the two disagree.
        return;
    };
    let compound = compound.trim();
    if compound.is_empty() {
        return;
    }

    let subject = format!("wd:{compound}");
    emit_quoted(out, &subject, "wdt:P235", set.inchikey(row));
    if let Some(smiles) = set.smiles(row) {
        let predicate = if smiles.contains(['@', '/', '\\']) {
            "wdt:P2017"
        } else {
            "wdt:P233"
        };
        emit_quoted(out, &subject, predicate, Some(smiles));
    }
    emit(out, &subject, "wdt:P2067", &number(set.mass(row)));
    emit_quoted(out, &subject, "wdt:P274", set.formula(row));
    if let Some(label) = set.compound_label(row) {
        emit_quoted(out, &subject, "rdfs:label", Some(label));
    }

    // The occurrence is emitted as the reified statement chain the endpoint's
    // CONSTRUCT produces, so a consumer that parses one parses the other:
    //
    //     compound p:P703  statement
    //     statement ps:P703              taxon
    //     statement prov:wasDerivedFrom reference-node
    //     reference-node pr:P248         publication
    let taxon = set
        .taxon_qid_text(row)
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    let statement = statement_text(set, row);
    // Bracketed: a bare URI is not a Turtle IRI, and the endpoint's own export
    // emits them bracketed. Unbracketed, a strict parser rejects the file.
    let statement = (!statement.trim().is_empty())
        .then(|| bracket_iri(WIKIDATA_STATEMENT_BASE, statement.trim()));

    match (&statement, &taxon) {
        (Some(statement_uri), Some(taxon)) => {
            emit(out, &subject, "p:P703", statement_uri);
            emit(out, statement_uri, "ps:P703", &format!("wd:{taxon}"));
        }
        // No statement to hang it on: fall back to the direct taxon edge, which
        // is lossy but keeps the occurrence in the graph.
        (None, Some(taxon)) => emit(out, &subject, "wdt:P703", &format!("wd:{taxon}")),
        _ => {}
    }

    if let Some(taxon) = &taxon
        && let Some(label) = set.taxon_label(row)
    {
        emit_quoted(out, &format!("wd:{taxon}"), "wdt:P225", Some(label));
    }

    // `prov:wasDerivedFrom` names the reference *node*, and `pr:P248` links
    // that node to the publication. They are different identifiers and the
    // graph needs both.
    let reference_node = set
        .reference_node_text(row)
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .map(|hash| {
            if hash.starts_with("http://") || hash.starts_with("https://") {
                bracket_iri("", &hash)
            } else {
                bracket_iri(WIKIDATA_REFERENCE_BASE, &hash)
            }
        });
    let publication = set
        .reference_qid_text(row)
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .map(|qid| format!("wd:{qid}"));

    let reference_term = reference_node.as_deref().or(publication.as_deref());

    if let Some(statement_uri) = &statement
        && let Some(reference) = reference_term
    {
        emit(out, statement_uri, "prov:wasDerivedFrom", reference);
    }
    if let (Some(node), Some(publication)) = (&reference_node, &publication) {
        emit(out, node, "pr:P248", publication);
    }

    // Title, DOI and year live on the publication, not on the reference node.
    if let Some(publication) = &publication {
        emit_quoted(out, publication, "wdt:P1476", set.reference_title(row));
        emit_quoted(out, publication, "wdt:P356", set.reference_doi(row));
        if let Some(year) = set.pub_year(row) {
            let year_lit = if year < 0 {
                format!("\"-{:04}\"^^xsd:gYear", year.unsigned_abs())
            } else {
                format!("\"{year:04}\"^^xsd:gYear")
            };
            emit(out, publication, "wdt:P577", &year_lit);
        }
    }
}

fn emit(out: &mut String, subject: &str, predicate: &str, object: &str) {
    // An empty object means the value was absent, and a triple whose object is
    // unbound is not emitted -- which is what the endpoint's CONSTRUCT does, and
    // what makes this file readable by the same tooling.
    if object.is_empty() || subject.is_empty() {
        return;
    }
    // `writeln!` rather than `write!` with a trailing `\n`: the clippy lint exists
    // because the two forms drift, and this one has no reason to be the exception.
    let _ = writeln!(out, "{subject} {predicate} {object} .");
}

pub(super) fn render_mass(value: Option<f64>) -> String {
    value.map_or_else(String::new, |mass| {
        // `{}` on an `f64` already gives the shortest representation that round-trips,
        // so a trailing `.0` is only added for a whole number to keep the column
        // looking like the others.
        if mass.fract() == 0.0 && mass.is_finite() {
            format!("{mass:.1}")
        } else {
            format!("{mass}")
        }
    })
}

/// A Turtle typed number, or nothing when there is none.
pub(super) fn number(value: Option<f64>) -> String {
    value
        .filter(|mass| mass.is_finite())
        .map_or_else(String::new, |mass| {
            format!("\"{}\"^^xsd:decimal", render_mass(Some(mass)))
        })
}

fn bracket_iri(base: &str, local: &str) -> String {
    let mut iri = String::with_capacity(base.len() + local.len() + 2);
    iri.push('<');
    iri.push_str(base);
    for ch in local.chars() {
        if ch <= '\u{20}' || matches!(ch, '<' | '>' | '"' | '{' | '}' | '|' | '^' | '`' | '\\') {
            let _ = write!(iri, "%{:02X}", u32::from(ch));
        } else {
            iri.push(ch);
        }
    }
    iri.push('>');
    iri
}

/// Write a Turtle triple whose object is a quoted string literal directly into `out`.
fn emit_quoted(out: &mut String, subject: &str, predicate: &str, value: Option<&str>) {
    let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return;
    };
    if subject.is_empty() {
        return;
    }
    let _ = write!(out, "{subject} {predicate} ");
    super::push_escaped_quoted(out, value);
    out.push_str(" .\n");
}
