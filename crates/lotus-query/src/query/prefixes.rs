// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared SPARQL prefixes, projection columns, and reusable property blocks.

pub(super) const PREFIXES: &str = "\
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
pub(super) const PREFIXES_WITH_STRUCTURE: &str = "\
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

/// The base prefixes plus `skos`, for the alias lookup.
pub(super) const PREFIXES_WITH_SKOS: &str = "\
PREFIX xsd:    <http://www.w3.org/2001/XMLSchema#>
PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
PREFIX wd:     <http://www.wikidata.org/entity/>
PREFIX wdt:    <http://www.wikidata.org/prop/direct/>
PREFIX skos:   <http://www.w3.org/2004/02/skos/core#>
";

/// The names `SELECT_CLAUSE` projects, in order.
///
/// The parser finds its columns by name, so this list and the projection must be the
/// same list. They were kept in step by hand and by nothing else, across two files: a
/// projected name the parser does not know is not an error, it is a column that silently
/// arrives empty. `the_projection_is_exactly_these_names` fails when text and list disagree.
///
/// `compound_smiles_conn` and `compound_smiles_iso` are both here because both are
/// projected and the parser picks between them; a coalesced column would replace the pair
/// rather than sit beside it.
///
/// `?ref` is the reference *node* URI -- `http://www.wikidata.org/reference/<hex>` --
/// and **not** a QID, so it is projected whole rather than through the
/// `STRAFTER`/`xsd:integer` treatment every QID column gets. That cast is a type error on
/// every row, which `QLever` reports as an unbound cell rather than a failed query, so
/// the column would look present and be empty throughout. `?ref_qid` is the publication
/// the node points at via `pr:P248`; both are needed, since the `CONSTRUCT` template emits
/// `?statement prov:wasDerivedFrom ?ref` and the rows carry the node so provenance can be
/// rebuilt without re-querying.
///
/// `?ref` is projected but never shown: user-facing columns are named separately (see
/// `export_rows::COLUMNS`). Projecting it costs payload -- it was the largest single column
/// on an unnarrowed result, and `QLever` computes, serialises and sends it on every row.
pub const SELECT_COLUMNS: [&str; 15] = [
    "compound",
    "compoundLabel",
    "compound_inchikey",
    "compound_smiles_conn",
    "compound_smiles_iso",
    "compound_mass",
    "compound_formula",
    "taxon",
    "taxon_name",
    "ref_qid",
    "ref_node",
    "ref_title",
    "ref_doi",
    "ref_year",
    "statement_id",
];

/// The result columns.
///
/// QIDs and the publication year are projected as inline 64-bit integers
/// (`Datatype::Int` inside `QLever`'s 8-byte `ValueId`) rather than raw
/// `VocabIndex` IRIs or `xsd:dateTime` literals, eliminating over two million
/// on-disk vocabulary lookups and 81 MB of URI/date text during CSV streaming
/// (`writeResultToStream`) without allocating any `LocalVocab` strings.
/// Statement and reference-node URIs are projected directly without `STRAFTER`
/// so `QLever` never materialises dynamic `LocalVocab` strings; the streaming
/// parser strips their prefixes in `O(1)` on arrival.
pub(super) const SELECT_CLAUSE: &str = r#"
SELECT
  (xsd:integer(STRAFTER(STR(?c), "Q")) AS ?compound)
  ?compoundLabel
  ?compound_inchikey
  ?compound_smiles_conn
  ?compound_smiles_iso
  ?compound_mass
  ?compound_formula
  (xsd:integer(STRAFTER(STR(?t), "Q")) AS ?taxon)
  ?taxon_name
  (xsd:integer(STRAFTER(STR(?r), "Q")) AS ?ref_qid)
  ?ref_node
  ?ref_title
  ?ref_doi
  (YEAR(?ref_date) AS ?ref_year)
  ?statement_id
"#;

/// Reference metadata: the title, DOI and date of the citing reference.
///
/// **`BIND(COALESCE(?r, wd:Q0) AS ?_ref_source)` is load-bearing.** `?r` is bound by
/// the optional occurrence block, so a compound with no occurrence leaves it unbound, and
/// an unbound variable entering an `OPTIONAL` gets a fresh binding: `OPTIONAL { ?r
/// wdt:P1476 ?ref_title }` silently becomes *every reference in Wikidata*, and the query
/// then dies on the sort. Measured on `Q161495`: **54,116,026 rows** unguarded, and
/// `QLever` answered `Operation timed out. Last operation: Sort (internal order)` with
/// `resultsize: 0` -- an empty result that looks like "no occurrences" and is really "the
/// query could not be answered".
///
/// The guard is a sentinel, not `FILTER(BOUND(?ref_node))` per line: a `FILTER` inside an
/// `OPTIONAL` is evaluated against *that `OPTIONAL`'s own solutions* before the join
/// with the left side, so a variable bound only outside is unbound there and the filter
/// rejects everything. Measured on `Q153`, 87 rows carried a title before the guard and
/// **0** after, so every title, DOI and date came back empty -- which read as "the
/// references have no metadata" rather than as a bug. Any variable bound nowhere gives
/// the same 0.
///
/// `COALESCE` binds `?r` to `wd:Q0`, which is not a Wikidata item, so `wd:Q0 wdt:P1476 ?x`
/// matches nothing after one lookup instead of enumerating every reference. When there
/// *is* an occurrence `?r` is bound, `COALESCE` passes it through, and a row whose
/// reference has no title, DOI or date still comes back with those cells empty. Verified:
/// `Q153` 88 rows with a title (62 ms), `Q161495` 1 row (24 ms).
///
/// The guard lives inside this constant so [`counts_query`](super::counts_query), which removes this whole block
/// by string match, still removes all of it.
pub(super) const REFERENCE_METADATA: &str = "
  BIND(COALESCE(?r, wd:Q0) AS ?_ref_source)
  OPTIONAL { ?_ref_source wdt:P1476 ?ref_title. }
  OPTIONAL { ?_ref_source wdt:P356 ?ref_doi. }
  OPTIONAL { ?_ref_source wdt:P577 ?ref_date. }
";

pub(super) const COMPOUND_PROPERTIES: &str = r#"
  OPTIONAL { ?c rdfs:label ?compoundLabelEn. FILTER(LANG(?compoundLabelEn) = "en") }
  OPTIONAL { ?c rdfs:label ?compoundLabelMul. FILTER(LANG(?compoundLabelMul) = "mul") }
  BIND(COALESCE(?compoundLabelMul, ?compoundLabelEn) AS ?compoundLabel)
  OPTIONAL { ?c wdt:P233 ?compound_smiles_conn. }
  OPTIONAL { ?c wdt:P2017 ?compound_smiles_iso. }
  OPTIONAL { ?c wdt:P2067 ?compound_mass. }
  OPTIONAL { ?c wdt:P274 ?compound_formula. }
"#;

/// Subscript digits, so that `C₁₇H₁₂O₇` and `C17H12O7` compare equal.
pub(super) const SUBSCRIPTS: [(char, char); 10] = [
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
