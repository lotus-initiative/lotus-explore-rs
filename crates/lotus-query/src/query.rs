// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Query construction.
//!
//! Every query this crate builds is a `SELECT` over the LOTUS projection in
//! Wikidata, shaped as nested subqueries so that `QLever` can filter before it
//! enriches. The nesting is load-bearing and is asserted in
//! `tests/query_contract.rs`; those tests assert the builders' behaviour rather
//! than their bytes, so whitespace may change but structure may not.

use lotus_model::{ElementState, SearchCriteria, SmilesSearchType, classify_structure};
use std::fmt::Write as _;

const PREFIXES: &str = "\
PREFIX xsd:    <http://www.w3.org/2001/XMLSchema#>
PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
PREFIX prov:   <http://www.w3.org/ns/prov#>
PREFIX wd:     <http://www.wikidata.org/entity/>
PREFIX wdt:    <http://www.wikidata.org/prop/direct/>
PREFIX p:      <http://www.wikidata.org/prop/>
PREFIX ps:     <http://www.wikidata.org/prop/statement/>
PREFIX pq:     <http://www.wikidata.org/prop/qualifier/>
PREFIX pr:     <http://www.wikidata.org/prop/reference/>
PREFIX wikibase: <http://wikiba.se/ontology#>
PREFIX schema: <http://schema.org/>
";

/// The base prefixes plus the two the structure-search service needs.
const PREFIXES_WITH_STRUCTURE: &str = "\
PREFIX xsd:    <http://www.w3.org/2001/XMLSchema#>
PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
PREFIX prov:   <http://www.w3.org/ns/prov#>
PREFIX wd:     <http://www.wikidata.org/entity/>
PREFIX wdt:    <http://www.wikidata.org/prop/direct/>
PREFIX p:      <http://www.wikidata.org/prop/>
PREFIX ps:     <http://www.wikidata.org/prop/statement/>
PREFIX pq:     <http://www.wikidata.org/prop/qualifier/>
PREFIX pr:     <http://www.wikidata.org/prop/reference/>
PREFIX wikibase: <http://wikiba.se/ontology#>
PREFIX schema: <http://schema.org/>
PREFIX sachem: <http://bioinfo.uochb.cas.cz/rdf/v1.0/sachem#>
PREFIX idsm:   <https://idsm.elixir-czech.cz/sparql/endpoint/>
";

/// The result columns. QIDs are projected as integers so that the CSV they come
/// back in is already the bare `Q…` form the parsers expect.
#[must_use]
fn select_clause() -> String {
    format!(
        r#"
SELECT DISTINCT
  (xsd:integer(STRAFTER(STR(?c), "Q")) AS ?compound)
  ?compoundLabel
  ?compound_inchikey
  ?compound_smiles_conn
  ?compound_smiles_iso
  ?compound_mass
  ({} AS ?compound_formula)
  (xsd:integer(STRAFTER(STR(?t), "Q")) AS ?taxon)
  ?taxon_name
  (xsd:integer(STRAFTER(STR(?r), "Q")) AS ?ref_qid)
  ?ref
  ?ref_title
  ?ref_doi
  ?ref_date
  ?statement
"#,
        normalize_digits_expr("?compound_formula_raw")
    )
}

/// The innermost projection: only what the occurrence itself asserts.
const CORE_VARS: &str =
    "?c ?compound_inchikey ?compound_smiles_conn ?t ?taxon_name ?r ?ref ?statement";

/// The middle projection: the core plus everything the display rows show.
const ENRICHED_VARS: &str = "\
?c ?compound_inchikey ?compound_smiles_conn
      ?compound_smiles_iso ?compound_mass ?compound_formula_raw
      ?compoundLabel
      ?t ?taxon_name
      ?r ?ref
      ?ref_title ?ref_doi ?ref_date
      ?statement";

const REFERENCE_METADATA: &str = "
  OPTIONAL { ?r wdt:P1476 ?ref_title. }
  OPTIONAL { ?r wdt:P356 ?ref_doi. }
  OPTIONAL { ?r wdt:P577 ?ref_date. }
";

const COMPOUND_PROPERTIES: &str = r#"
  OPTIONAL { ?c wdt:P2017 ?compound_smiles_iso. }
  OPTIONAL { ?c wdt:P2067 ?compound_mass. }
  OPTIONAL { ?c wdt:P274 ?compound_formula_raw. }
  OPTIONAL { ?c rdfs:label ?compoundLabelMul. FILTER(LANG(?compoundLabelMul) = "mul") }
  OPTIONAL { ?c rdfs:label ?compoundLabelEn. FILTER(LANG(?compoundLabelEn) = "en") }
  BIND(COALESCE(?compoundLabelMul, ?compoundLabelEn) AS ?compoundLabel)
"#;

/// Subscript digits, so that `C₁₇H₁₂O₇` and `C17H12O7` compare equal.
const SUBSCRIPTS: [(char, char); 10] = [
    ('₀', '0'),
    ('₁', '1'),
    ('₂', '2'),
    ('₃', '3'),
    ('₄', '4'),
    ('₅', '5'),
    ('₆', '6'),
    ('₇', '7'),
    ('₈', '8'),
    ('₉', '9'),
];

/// Every compound the LOTUS projection knows, across all taxa.
#[must_use]
pub fn all_compounds_query() -> String {
    compounds_query(None)
}

/// Compounds found in `taxon_qid` and its descendants (`P171*`).
#[must_use]
pub fn compounds_by_taxon_query(taxon_qid: &str) -> String {
    compounds_query(Some(taxon_qid))
}

fn compounds_query(taxon_qid: Option<&str>) -> String {
    let ancestry = taxon_qid.map_or_else(String::new, |qid| {
        format!(
            "\n          ?t (wdt:P171*) wd:{} .",
            escape_sparql_string(qid)
        )
    });

    format!(
        "{PREFIXES}
{}
WHERE {{
  {{
    SELECT
      {ENRICHED_VARS}
    WHERE {{
      {{
        SELECT {CORE_VARS}
        WHERE {{
          ?c wdt:P235 ?compound_inchikey ;
             wdt:P233 ?compound_smiles_conn .
          ?c p:P703 ?statement .
          ?statement ps:P703 ?t ;
                     prov:wasDerivedFrom ?ref .
          ?ref pr:P248 ?r .
          ?t wdt:P225 ?taxon_name .{ancestry}
        }}
      }}
      {REFERENCE_METADATA}
      {COMPOUND_PROPERTIES}
    }}
  }}
}}",
        select_clause(),
    )
}

/// A `VALUES` lookup of taxa by scientific name (P225).
///
/// An exact match, with no `FILTER` and no `LCASE`: `VALUES` is served from an
/// index, and a case-folding scan of every label is the one thing that makes
/// this slow.
#[must_use]
pub fn taxon_lookup_query(name: &str) -> String {
    format!(
        "PREFIX wdt: <http://www.wikidata.org/prop/direct/>
SELECT ?taxon ?taxon_name
WHERE {{
  VALUES ?taxon_name {{ \"{}\" }}
  ?taxon wdt:P225 ?taxon_name .
}}",
        escape_sparql_string(name)
    )
}

/// A structure search through the IDSM/Sachem service.
///
/// Without `taxon_qid` the occurrence triple is optional, so a compound with
/// no occurrence data is still returned — structure search is how such
/// compounds are found in the first place.
#[must_use]
pub fn structure_search_query(
    structure: &str,
    search: SmilesSearchType,
    threshold: f64,
    taxon_qid: Option<&str>,
) -> String {
    let literal = escape_structure_literal(structure);
    let is_multiline = literal.starts_with("'''");

    let service = match search {
        SmilesSearchType::Similarity => format!(
            "SERVICE idsm:wikidata {{
    ?c sachem:similarCompoundSearch [
      sachem:query {literal};
      sachem:cutoff \"{threshold}\"^^xsd:double
    ].
  }}"
        ),
        SmilesSearchType::Substructure if is_multiline => format!(
            "SERVICE idsm:wikidata {{
    [ sachem:compound ?c; sachem:score ?_sachem_score ]
      sachem:scoredSubstructureSearch [
        sachem:query {literal};
        sachem:searchMode sachem:substructureSearch;
        sachem:chargeMode sachem:defaultChargeAsAny;
        sachem:isotopeMode sachem:ignoreIsotopes;
        sachem:aromaticityMode sachem:aromaticityDetectIfMissing;
        sachem:stereoMode sachem:ignoreStereo;
        sachem:tautomerMode sachem:ignoreTautomers;
        sachem:radicalMode sachem:ignoreSpinMultiplicity;
        sachem:topn \"-1\"^^xsd:integer;
        sachem:internalMatchingLimit \"1000000\"^^xsd:integer
      ].
  }}"
        ),
        SmilesSearchType::Substructure => format!(
            "SERVICE idsm:wikidata {{
    ?c sachem:substructureSearch [
      sachem:query {literal}
    ].
  }}"
        ),
    };

    // With a taxon, the service runs in its own subquery so that QLever can
    // pre-filter before enriching, and the occurrence triple is *required* —
    // a match outside the requested taxon should not come back.
    //
    // Without one, the service joins the body directly and the occurrence
    // triple is `OPTIONAL`: a structure search is how a compound with no
    // occurrence data gets found in the first place, and requiring a taxon
    // would hide exactly those.
    let body = taxon_qid.map_or_else(
        || {
            format!(
                "
  {service}

  ?c wdt:P235 ?compound_inchikey ;
     wdt:P233 ?compound_smiles_conn .

  OPTIONAL {{
    ?c p:P703 ?statement .
    ?statement ps:P703 ?t ;
               prov:wasDerivedFrom ?ref .
    ?ref pr:P248 ?r .
    ?t wdt:P225 ?taxon_name .
    {REFERENCE_METADATA}
  }}

  {COMPOUND_PROPERTIES}
"
            )
        },
        |qid| {
            format!(
                "
  {{
    SELECT DISTINCT ?c
    WHERE {{
      {service}
    }}
  }}

  ?c wdt:P235 ?compound_inchikey ;
     wdt:P233 ?compound_smiles_conn .
  ?c p:P703 ?statement .
  ?statement ps:P703 ?t ;
             prov:wasDerivedFrom ?ref .
  ?ref pr:P248 ?r .
  ?t wdt:P225 ?taxon_name .
  ?t (wdt:P171*) wd:{} .

  {REFERENCE_METADATA}
  {COMPOUND_PROPERTIES}
",
                escape_sparql_string(qid)
            )
        },
    );

    format!(
        "{PREFIXES_WITH_STRUCTURE}
{}
WHERE {{
{body}
}}",
        select_clause(),
    )
}

/// Wrap a `SELECT` in a `COUNT`, keeping the filters but dropping everything the
/// count does not depend on.
///
/// The display `OPTIONAL`s are stripped, and that is the point: `rdfs:label`
/// with two `FILTER(LANG(…))` passes is the most expensive thing in the query,
/// and a count that walked it would be slow enough to be rate-limited. The
/// variables it leaves unbound project as `NULL`, which the `COUNT`s ignore.
#[must_use]
pub fn counts_query(base: &str) -> String {
    let Some(select_at) = base.find("SELECT") else {
        return base.to_string();
    };
    let prefixes = &base[..select_at];
    let stripped = base[select_at..]
        .replace(REFERENCE_METADATA, "")
        .replace(COMPOUND_PROPERTIES, "");

    format!(
        "{prefixes}
SELECT
  (COUNT(*) AS ?n_entries)
  (COUNT(DISTINCT CONCAT(
    STR(?compound), \"\\u001F\", COALESCE(STR(?taxon), \"\"), \"\\u001F\", COALESCE(STR(?ref_qid), \"\")
  )) AS ?n_entries_unique)
  (COUNT(DISTINCT ?compound) AS ?n_compounds)
  (COUNT(DISTINCT ?taxon) AS ?n_taxa)
  (COUNT(DISTINCT ?ref_qid) AS ?n_references)
WHERE {{
  {{
    {}
  }}
}}",
        stripped.trim()
    )
}

/// Append a `LIMIT`.
#[must_use]
pub fn limit_query(base: &str, limit: usize) -> String {
    format!("{}\nLIMIT {limit}", base.trim_end())
}

/// Apply the active filters to a base query.
///
/// The fragments go after the base query's closing brace, not inside the
/// innermost subquery. That is where `inject` puts them and where the endpoint
/// has always been asked to evaluate them; moving them would change the query
/// plan, and the plan is tuned.
#[must_use]
pub fn with_filters(base: &str, criteria: &SearchCriteria, year_max: u16) -> String {
    let mut required = String::new();
    let mut filters = String::new();

    if criteria.has_mass_filter() {
        let _ = write!(
            filters,
            "FILTER(?compound_mass >= {:.6} && ?compound_mass <= {:.6})",
            criteria.mass_min, criteria.mass_max
        );
        let _ = writeln!(required, "?c wdt:P2067 ?compound_mass .");
    }

    if criteria.has_year_filter(year_max) {
        let _ = write!(
            filters,
            "FILTER(YEAR(?ref_date) >= {} && YEAR(?ref_date) <= {})",
            criteria.year_min, criteria.year_max
        );
        let _ = writeln!(required, "?r wdt:P577 ?ref_date .");
    }

    formula_filter(criteria, &mut filters);

    if required.is_empty() && filters.is_empty() {
        return base.to_string();
    }

    // The base query's own closing brace becomes the filters' closing brace, so
    // the fragments land in its outermost WHERE rather than in a new block.
    let Some(without_closing_brace) = trimmed_strip_closing_brace(base) else {
        return format!("{base}\n{required}{filters}");
    };
    format!("{without_closing_brace}\n{required}{filters}\n}}")
}

fn trimmed_strip_closing_brace(base: &str) -> Option<&str> {
    base.trim_end().strip_suffix('}')
}

/// The formula filter: normalise the formula once, then count elements against
/// the normalised string.
fn formula_filter(criteria: &SearchCriteria, out: &mut String) {
    if !criteria.has_formula_filter() {
        return;
    }

    // Without the BOUND guard, an unbound `?compound_formula_raw` makes every
    // comparison below evaluate against an error rather than a value.
    //
    // The variable names here are part of the query text, and a query's bytes
    // are what a cache key and a shared link are derived from, so they are not
    // renamed for tidiness.
    let _ = writeln!(out, "FILTER(BOUND(?compound_formula_raw))");
    let _ = writeln!(out, "BIND(STR(?compound_formula_raw) AS ?_formula_raw)");
    let _ = writeln!(
        out,
        r#"BIND(REPLACE(?_formula_raw, " ", "") AS ?_formula_nospace)"#
    );
    let _ = writeln!(
        out,
        "BIND({} AS ?_formula_norm)",
        normalize_digits_expr("?_formula_nospace")
    );
    // Split before each capital so element symbols become addressable tokens.
    let _ = writeln!(
        out,
        r#"BIND(REPLACE(?_formula_norm, "([A-Z])", "|$1") AS ?_formula_tokens)"#
    );

    // Every BIND first, then every FILTER: a group pattern's FILTERs see the
    // BINDs regardless of order, but interleaving them makes the query harder
    // to read and to diff.
    let mut ranged = Vec::new();
    for (symbol, min, max, default_max) in criteria.element_ranges() {
        if min > 0 || max < default_max {
            let var = bind_element_count(out, symbol);
            ranged.push(format!("FILTER({var} >= {min} && {var} <= {max})"));
        }
    }

    let mut halogens = Vec::new();
    for (symbol, state) in criteria.halogen_states() {
        if state == ElementState::Allowed {
            continue;
        }
        let var = bind_element_count(out, symbol);
        halogens.push(match state {
            ElementState::Required => format!("FILTER({var} > 0)"),
            ElementState::Excluded => format!("FILTER({var} = 0)"),
            ElementState::Allowed => unreachable!("the `Allowed` case is skipped above"),
        });
    }

    for filter in ranged.iter().chain(&halogens) {
        let _ = writeln!(out, "{filter}");
    }

    if let Some(exact) = normalized_exact_formula(criteria) {
        let _ = writeln!(out, "FILTER(?_formula_norm = \"{exact}\")");
    }
}

/// The exact formula to compare against, with subscripts folded to ASCII and
/// whitespace removed. The comparison happens against the endpoint's normalised
/// formula, so a subscript in the input would never match.
fn normalized_exact_formula(criteria: &SearchCriteria) -> Option<String> {
    let normalized: String = criteria
        .formula_exact
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '₀' => '0',
            '₁' => '1',
            '₂' => '2',
            '₃' => '3',
            '₄' => '4',
            '₅' => '5',
            '₆' => '6',
            '₇' => '7',
            '₈' => '8',
            '₉' => '9',
            other => other,
        })
        .collect();
    (!normalized.is_empty()).then(|| escape_sparql_string(&normalized))
}

/// A `BIND` yielding the count of `symbol` in the tokenised formula, or `0`.
/// Append a `BIND` yielding `symbol`'s count in the tokenised formula, or `0`,
/// and return the variable it binds.
///
/// The token separator is emitted as `\\|` because the pattern goes into a SPARQL
/// string literal, where a backslash is itself escaped. That is the pattern the
/// endpoint has always been sent, and changing it changes what matches.
fn bind_element_count(out: &mut String, symbol: &str) -> String {
    let var = format!("?_count_{}", symbol.to_ascii_lowercase());
    let pattern = format!(r"\\|{symbol}([0-9]*)(\\||$)");
    let captured = format!(r#"REPLACE(?_formula_tokens, ".*{pattern}.*", "$1")"#);
    let _ = writeln!(
        out,
        "BIND(IF(REGEX(?_formula_tokens, \"{pattern}\"), IF(STRLEN({captured}) = 0, 1, xsd:integer({captured})), 0) AS {var})"
    );
    var
}

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
    // Splice in the source's whole trailing block, minus its final brace: the
    // formula BIND goes inside it, and the brace is re-emitted below.
    let where_block = select[where_at..].trim();
    let Some(where_block) = where_block.strip_suffix('}') else {
        return select.to_string();
    };

    // `where_block` already begins with `WHERE`, so only the CONSTRUCT template
    // goes above it; the formula BIND is appended inside that same block.
    format!(
        "{prefixes}
CONSTRUCT {{
  ?c wdt:P235 ?compound_inchikey .
  ?c wdt:P233 ?compound_smiles_conn .
  ?c wdt:P2017 ?compound_smiles_iso .
  ?c wdt:P2067 ?compound_mass .
  ?c wdt:P274 ?compound_formula .
  ?c rdfs:label ?compoundLabel .
  ?c p:P703 ?statement .
  ?statement ps:P703 ?t ;
             prov:wasDerivedFrom ?ref .
  ?ref pr:P248 ?r .
  ?t wdt:P225 ?taxon_name .
  ?r wdt:P1476 ?ref_title .
  ?r wdt:P356 ?ref_doi .
  ?r wdt:P577 ?ref_date .
}}
{where_block}
  BIND({} AS ?compound_formula)
}}
",
        normalize_digits_expr("?compound_formula_raw")
    )
}

/// Which Wikidata service a fallback query needs.
///
/// This is a routing decision about the query, not about the network, so it
/// belongs here rather than in the crate that holds the URLs. Naming the
/// service instead of the endpoint also means the URLs stay overridable: a
/// caller running its own mirror picks the URL and still gets the right
/// rewrite.
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
  SERVICE <https://query-scholarly.wikidata.org/sparql> {
    OPTIONAL { ?r wdt:P1476 ?ref_title. }
    OPTIONAL { ?r wdt:P356 ?ref_doi. }
    OPTIONAL { ?r wdt:P577 ?ref_date. }
  }
";

    if is_reference_lookup(query) {
        (
            FallbackService::Scholarly,
            query.replace("{CURATION_SPARQL_PREFIXES}\n", ""),
        )
    } else {
        let rewritten = query
            .replace(
                REFERENCE_METADATA,
                &REFERENCE_METADATA.replace("?r ", "?ref "),
            )
            .replace(REFERENCE_METADATA, SCHOLARLY_REF);
        (FallbackService::Main, rewritten)
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
        format!("'''{escaped}'''")
    } else {
        let quoted = escaped.replace('"', "\\\"");
        format!("\"{quoted}\"")
    }
}
