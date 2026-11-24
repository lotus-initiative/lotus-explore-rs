// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Query rewriters (`CONSTRUCT` export, WDQS fallback) and SPARQL literal escapers.

use lotus_model::classify_structure;

use super::prefixes::{REFERENCE_METADATA, SUBSCRIPTS};

/// Prepare a `SELECT` for export in `format`.
#[must_use]
pub fn export_query(select: &str, as_rdf: bool) -> String {
    if as_rdf {
        construct_from_select(select)
    } else {
        select.to_string()
    }
}

/// Rewrite a `SELECT` as a `CONSTRUCT` that emits the provenance graph.
#[must_use]
pub fn construct_from_select(select: &str) -> String {
    let Some(select_at) = select.find("SELECT") else {
        return select.to_string();
    };
    let Some(where_at) = select[select_at..].find("WHERE") else {
        return select.to_string();
    };
    let where_at = select_at + where_at;
    let prefixes = &select[..select_at];
    // Splice in the source's whole trailing block up to its final closing
    // brace so the formula BIND lands inside WHERE and any trailing clause
    // (such as LIMIT or OFFSET) stays after it.
    let trailing = select[where_at..].trim();
    let Some((where_body, suffix)) = trailing.rsplit_once('}') else {
        return select.to_string();
    };

    format!(
        "{prefixes}
CONSTRUCT {{
  ?c wdt:P235 ?compound_inchikey .
  ?c wdt:P233 ?compound_smiles_conn .
  ?c wdt:P2017 ?compound_smiles_iso .
  ?c wdt:P2067 ?compound_mass .
  ?c wdt:P274 ?compound_formula_norm .
  ?c rdfs:label ?compoundLabel .
  ?c p:P703 ?statement_id .
  ?statement_id ps:P703 ?t ;
                prov:wasDerivedFrom ?ref_node .
  ?ref_node pr:P248 ?r .
  ?t wdt:P225 ?taxon_name .
  ?r wdt:P1476 ?ref_title .
  ?r wdt:P356 ?ref_doi .
  ?r wdt:P577 ?ref_date .
}}
{where_body}
  BIND({} AS ?compound_formula_norm)
}}{suffix}
",
        normalize_digits_expr("?compound_formula")
    )
}

/// Which Wikidata service a fallback query needs.
///
/// A routing decision about the query, not the network, so it belongs here rather than with
/// the URLs. Naming the service keeps the URLs overridable: a caller running its own mirror
/// picks the URL and still gets the right rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackService {
    /// The main `WDQS` endpoint.
    Main,
    /// The scholarly subgraph, which answers `P356` quickly.
    Scholarly,
}

/// Route a query to `WDQS` after `QLever` failed, returning the service to use
/// and the rewritten query.
///
/// Reference lookups go to the scholarly subgraph, which is the only WDQS
/// service that answers P356 quickly; everything else goes to the main
/// endpoint with its reference `OPTIONAL`s moved inside a `SERVICE` block, so
/// that the bibliographic properties are still fetched.
#[must_use]
pub fn wdqs_fallback(query: &str) -> (FallbackService, String) {
    const SCHOLARLY_REF: &str = "
  SERVICE <https://query-scholarly-next.wikidata.org/sparql> {
    OPTIONAL { ?r wdt:P1476 ?ref_title. }
    OPTIONAL { ?r wdt:P356 ?ref_doi. }
    OPTIONAL { ?r wdt:P577 ?ref_date. }
  }
";

    if is_reference_lookup(query) {
        (FallbackService::Scholarly, query.to_string())
    } else {
        (
            FallbackService::Main,
            query.replace(REFERENCE_METADATA, SCHOLARLY_REF),
        )
    }
}

/// A bare `SELECT ?ref … wdt:P356` with no `SERVICE` or `OPTIONAL`, i.e. a
/// DOI-to-reference lookup.
#[must_use]
pub fn is_reference_lookup(query: &str) -> bool {
    query.contains("SELECT ?ref WHERE {")
        && query.contains("wdt:P356")
        && !query.contains("SERVICE")
        && !query.contains("OPTIONAL")
}

/// A nested `REPLACE` chain folding subscript digits to ASCII.
#[must_use]
pub fn normalize_digits_expr(var: &str) -> String {
    SUBSCRIPTS
        .iter()
        .fold(format!("STR({var})"), |acc, (from, to)| {
            format!(r#"REPLACE({acc}, "{from}", "{to}")"#)
        })
}

/// Escape a value for a SPARQL string literal.
#[must_use]
pub fn escape_sparql_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// Escape a structure for a SPARQL literal: triple-quoted if it spans lines
/// (a molfile), double-quoted otherwise.
///
/// Both delimiters are escaped for whichever one is used. A `'''` inside a
/// triple-quoted payload closes the literal early and turns the rest of an
/// uploaded molfile into query text, which is a silent wrong answer rather than
/// an error -- SPARQL's escape for a quote inside a long literal is a backslash.
#[must_use]
pub fn escape_structure_literal(structure: &str) -> String {
    let normalized = structure.replace("\r\n", "\n").replace('\r', "\n");
    let is_molfile = classify_structure(&normalized).is_molfile();
    let body = if is_molfile {
        normalized
    } else {
        normalized.trim().to_string()
    };

    let escaped = body.replace('\\', r"\\");
    if is_molfile || body.contains('\n') {
        // `'''` is the terminator, so an apostrophe has to be escaped as well.
        format!("'''{}'''", escaped.replace('\'', r"\'"))
    } else {
        let quoted = escaped.replace('"', "\\\"");
        format!("\"{quoted}\"")
    }
}
