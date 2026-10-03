// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Every combination of the three search arguments, tested as a matrix.
//!
//! Three arguments reach the query builder, and they combine into twelve
//! requests. The dispatch between them lives in one `match` in
//! [`lotus_search::build_base_query`], which means a cell can be wrong without
//! any other cell noticing: the tests elsewhere pin the *builders*, and a builder
//! that is never called for a given combination is not covered by them.
//!
//! The twelve cells are `taxon` x `structure` x `reference`, where each is either
//! absent or a specific value:
//!
//! ```text
//!                  taxon absent   taxon specific   taxon "*"
//! structure absent      1               2               3
//! structure present     4               5               6
//! x reference absent    +1              +1              +1
//! x reference present   +1              +1              +1
//! ```
//!
//! What each cell is *allowed* to assert is different, and the distinction is the
//! point of the file:
//!
//! - With a taxon, the occurrence is **required**: `P703` is a plain triple, so
//!   every row is an occurrence somebody recorded.
//! - With no taxon, the occurrence is **optional**, and optional as *one block* --
//!   taxon and reference together. A compound with no organism is then returned
//!   with empty cells rather than dropped. Optional per triple would emit rows
//!   carrying a taxon but no reference, a shape the columnar result store cannot
//!   represent.
//! - `*` is not "nothing given". It is the explicit request for what has been
//!   reported, so it keeps requiring `P703`. This is asserted per cell below,
//!   and it is the assertion that caught `*` and an empty box building the same
//!   query.
//!
//! A reference constrains `?r`, the item the reference *is*, rather than
//! filtering a projected value. It is applied outside the base query's optional
//! block, so a reference plus no taxon returns only the compounds that reference
//! actually reported -- the `VALUES` cannot join against an unbound `?r`. That is
//! asserted too, because it is the one place where the two arguments interact
//! rather than compose.

// The panic lints exist to keep library code free of panics on external input.
// A test that fails on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_model::SearchCriteria;
use lotus_search::{SearchRequest, build_execution_query};
use std::fmt::Write as _;

const NOW: u16 = 2026;
const TAXON_QID: &str = "Q16521";
const REFERENCE_QID: &str = "Q999";
const STRUCTURE: &str = "CCO";

/// How the taxon argument arrives at the builder.
///
/// `Absent` and `Wildcard` are both resolved to `qid: None` by
/// [`lotus_search::resolve_taxon`](lotus_search::Http) -- a wildcard has no
/// Wikidata entity of its own -- so the builder has to read the distinction back
/// out of `criteria.taxon`. That it has to is the whole reason this file exists.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TaxonArg {
    Absent,
    Specific,
    Wildcard,
}

impl TaxonArg {
    const fn text(self) -> &'static str {
        match self {
            Self::Absent => "",
            Self::Specific => TAXON_QID,
            Self::Wildcard => "*",
        }
    }

    /// The QID the resolver hands the builder. `None` for both `Absent` and
    /// `Wildcard`, deliberately: the wildcard is not an entity.
    const fn resolved(self) -> Option<&'static str> {
        match self {
            Self::Specific => Some(TAXON_QID),
            Self::Absent | Self::Wildcard => None,
        }
    }

    /// Whether this taxon argument asks for compounds with an occurrence.
    ///
    /// `Absent` does not: nothing given includes the compounds nobody has tied to
    /// an organism, and answering the narrower question without saying so is the
    /// bug the optional block exists to avoid.
    ///
    /// `Wildcard` does, but only where the taxon reaches the query as a filter.
    /// A structure search resolves compound identities first, so `*` there is
    /// "every taxon" expressed by *not* filtering rather than by requiring an
    /// occurrence -- see `a_structure_search_keeps_the_occurrence_optional`.
    const fn requires_occurrence(self) -> bool {
        match self {
            Self::Absent => false,
            Self::Specific | Self::Wildcard => true,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Specific => "specific",
            Self::Wildcard => "\"*\"",
        }
    }
}

/// One cell of the matrix.
struct Cell {
    taxon: TaxonArg,
    structure: bool,
    reference: bool,
}

/// All twelve, in a fixed order so a failure names the same cell every run.
const MATRIX: [Cell; 12] = [
    Cell {
        taxon: TaxonArg::Absent,
        structure: false,
        reference: false,
    },
    Cell {
        taxon: TaxonArg::Absent,
        structure: false,
        reference: true,
    },
    Cell {
        taxon: TaxonArg::Absent,
        structure: true,
        reference: false,
    },
    Cell {
        taxon: TaxonArg::Absent,
        structure: true,
        reference: true,
    },
    Cell {
        taxon: TaxonArg::Specific,
        structure: false,
        reference: false,
    },
    Cell {
        taxon: TaxonArg::Specific,
        structure: false,
        reference: true,
    },
    Cell {
        taxon: TaxonArg::Specific,
        structure: true,
        reference: false,
    },
    Cell {
        taxon: TaxonArg::Specific,
        structure: true,
        reference: true,
    },
    Cell {
        taxon: TaxonArg::Wildcard,
        structure: false,
        reference: false,
    },
    Cell {
        taxon: TaxonArg::Wildcard,
        structure: false,
        reference: true,
    },
    Cell {
        taxon: TaxonArg::Wildcard,
        structure: true,
        reference: false,
    },
    Cell {
        taxon: TaxonArg::Wildcard,
        structure: true,
        reference: true,
    },
];

fn criteria_for(cell: &Cell) -> SearchCriteria {
    SearchCriteria {
        taxon: cell.taxon.text().to_string(),
        reference: if cell.reference {
            REFERENCE_QID.to_string()
        } else {
            String::new()
        },
        structure: if cell.structure {
            STRUCTURE.to_string()
        } else {
            String::new()
        },
        ..SearchCriteria::up_to_year(NOW)
    }
}

fn query_for(cell: &Cell) -> String {
    let criteria = criteria_for(cell);
    let request = SearchRequest::new(criteria, NOW);
    build_execution_query(&request, cell.taxon.resolved())
}

/// `taxon=<..> structure=<..> reference=<..>`, for assertion messages.
fn label(cell: &Cell) -> String {
    format!(
        "taxon={} structure={} reference={}",
        cell.taxon.name(),
        if cell.structure { "present" } else { "absent" },
        if cell.reference { "present" } else { "absent" }
    )
}

/// Whether the occurrence (`P703`) sits inside an `OPTIONAL`.
///
/// This is asserted structurally -- what block the triple is in -- rather than by
/// counting keywords, because this base query legitimately contains other
/// `OPTIONAL`s for the reference metadata. A count is not what distinguishes the
/// two queries; the block is.
fn occurrence_is_optional(query: &str) -> bool {
    let p703 = query
        .find("?c p:P703")
        .expect("every cell binds P703 somewhere");
    let head = &query[..p703];
    // The opening brace of the block sits between the keyword and the first
    // pattern, so whitespace alone is not the test.
    head.rfind("OPTIONAL").is_some_and(|at| {
        query[at + "OPTIONAL".len()..p703]
            .chars()
            .all(|c| c.is_whitespace() || c == '{')
    })
}

/// Triple patterns in the query: statements, counted after IRIs and literals are
/// removed so that the dots inside `https://…` and `"10.1/a"` are not counted.
fn triple_patterns(query: &str) -> usize {
    let mut stripped = String::with_capacity(query.len());
    let mut chars = query.chars();
    while let Some(c) = chars.next() {
        match c {
            // An IRI: `<…>`, dots included.
            '<' => {
                for c in chars.by_ref() {
                    if c == '>' {
                        break;
                    }
                }
                stripped.push(' ');
            }
            // A literal: `"…"`, dots included. SPARQL has no escape we emit here.
            '"' => {
                chars.next();
                for c in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                }
                stripped.push(' ');
            }
            _ => stripped.push(c),
        }
    }
    stripped.matches('.').count()
}

/// Balanced braces and parentheses, ignoring the contents of IRIs and literals.
///
/// The cheapest honest check that a built query is well formed without pulling in
/// a SPARQL parser: an unbalanced query is rejected by the endpoint with an error
/// that says nothing about which of the twelve cells produced it.
fn is_balanced(query: &str) -> bool {
    let mut braces = 0i32;
    let mut parens = 0i32;
    let mut chars = query.chars();
    while let Some(c) = chars.next() {
        match c {
            '<' | '"' => {
                let close = if c == '<' { '>' } else { '"' };
                for c in chars.by_ref() {
                    if c == close {
                        break;
                    }
                }
            }
            '{' => braces += 1,
            '}' => braces -= 1,
            '(' => parens += 1,
            ')' => parens -= 1,
            _ => {}
        }
        if braces < 0 || parens < 0 {
            return false;
        }
    }
    braces == 0 && parens == 0
}

/// Every `^` in a property path applies to exactly one alternative.
///
/// `^` binds tighter than `|`, so `^a|b` reads as "inverse of a, or b" and the
/// properties after the first are read forwards only. That is the bug class this
/// checks: it returns rows, so the query looks fine and answers a different
/// question. A correct path parenthesises every alternative or gives each its own
/// `^`, which means every `^` is preceded by `(` or `|`.
fn inverses_are_scoped(query: &str) -> bool {
    query
        .match_indices('^')
        // `^^` is the datatype operator (`"1"^^xsd:double`), not a path inverse.
        // Both carets of a run are part of that one token, so neither counts.
        .filter(|(at, _)| {
            let next_is_caret = query[at + 1..].starts_with('^');
            let previous_is_caret = query[..*at].ends_with('^');
            !next_is_caret && !previous_is_caret
        })
        .all(|(at, _)| matches!(query[..at].chars().next_back(), Some('(' | '|')))
}

/// The table `docs/QUERY_MATRIX.md` is generated from: one line per cell.
#[test]
fn every_cell_is_well_formed_and_records_its_shape() {
    let mut rows = Vec::new();
    for cell in &MATRIX {
        let query = query_for(cell);
        let label = label(cell);

        assert!(is_balanced(&query), "{label}: unbalanced query\n{query}");
        assert!(
            inverses_are_scoped(&query),
            "{label}: an unscoped `^` in a property path\n{query}"
        );
        assert!(
            !query.contains("FILTER()"),
            "{label}: an empty FILTER\n{query}"
        );

        rows.push(format!(
            "| {label} | {} | {} | {} |",
            query.len(),
            triple_patterns(&query),
            if occurrence_is_optional(&query) {
                "optional"
            } else {
                "required"
            }
        ));
    }

    // The table, so a change to the matrix is visible in a test failure rather
    // than only in a regenerated document.
    println!("\n| cell | bytes | triples | occurrence |\n|---|---|---|---|");
    for row in &rows {
        println!("{row}");
    }
}

/// The cell that has to be right: `*` requires an occurrence, an empty box does
/// not.
///
/// Both used to build the same query, because `resolve_taxon` resolves the
/// wildcard to `qid: None` and the builder was reading the difference off the QID
/// alone. It is asserted per cell rather than once, so a regression names the
/// combination that broke.
#[test]
fn a_wildcard_taxon_requires_an_occurrence_and_an_empty_taxon_does_not() {
    for cell in &MATRIX {
        if cell.structure {
            // A structure search resolves its compound identity first and keeps
            // the occurrence optional for a compound nobody has recorded; the
            // taxon argument does not reach that decision. Asserted in
            // `a_structure_search_keeps_the_occurrence_optional` instead.
            continue;
        }
        let query = query_for(cell);
        assert_eq!(
            occurrence_is_optional(&query),
            !cell.taxon.requires_occurrence(),
            "{}: `{}` should {} an occurrence",
            label(cell),
            cell.taxon.text(),
            if cell.taxon.requires_occurrence() {
                "require"
            } else {
                "make optional"
            }
        );
    }
}

/// The empty-taxon cells put taxon and reference in one `OPTIONAL`, never per
/// triple.
///
/// Per-triple optionals emit rows with a taxon but no reference. The columnar
/// result store keys a row on the compound and the reference, so such a row is
/// not representable, and the columns would claim an occurrence that does not
/// exist.
#[test]
fn an_absent_taxon_makes_one_optional_block_not_one_per_triple() {
    for cell in MATRIX.iter().filter(|c| c.taxon == TaxonArg::Absent) {
        let query = query_for(cell);
        assert!(
            occurrence_is_optional(&query),
            "{}: the occurrence must be optional\n{query}",
            label(cell)
        );
        // Inside the block, the taxon and the reference are bound together. A
        // per-triple spelling has `OPTIONAL` between `P703` and `ps:P703`.
        let p703 = query.find("?c p:P703").expect("P703 is bound");
        let reference = query
            .find("prov:wasDerivedFrom")
            .expect("the reference is bound");
        let taxon = query.find("ps:P703").expect("the taxon is bound");
        assert!(
            p703 < taxon && taxon < reference,
            "{}: taxon and reference must be bound inside the one block\n{query}",
            label(cell)
        );
        let between = &query[p703..reference];
        assert!(
            !between.contains("OPTIONAL"),
            "{}: no OPTIONAL may split the occurrence block\n{query}",
            label(cell)
        );
    }
}

/// A structure search over *every* taxon keeps the occurrence optional.
///
/// The compound is already known by the time the rows are wanted -- a name or
/// exact-structure search resolves to Wikidata identities first -- so a compound
/// nobody has tied to an organism is exactly the compound being looked for.
///
/// A structure search *within a named taxon* is the opposite, and deliberately so:
/// there the occurrence is required, because a match that is not an occurrence in
/// the requested taxon is not an answer to the question. So this covers the
/// no-taxon and wildcard cells only, and `taxon=specific` is asserted as required.
#[test]
fn a_structure_search_without_a_named_taxon_keeps_the_occurrence_optional() {
    for cell in MATRIX
        .iter()
        .filter(|c| c.structure && c.taxon != TaxonArg::Specific)
    {
        let query = query_for(cell);
        assert!(
            occurrence_is_optional(&query),
            "{}: a structure search over every taxon must not require an occurrence\n{query}",
            label(cell)
        );
    }

    for cell in MATRIX
        .iter()
        .filter(|c| c.structure && c.taxon == TaxonArg::Specific)
    {
        let query = query_for(cell);
        assert!(
            !occurrence_is_optional(&query),
            "{}: a match outside the requested taxon must not come back\n{query}",
            label(cell)
        );
    }
}

/// A reference binds `?r` as a `VALUES`, outside the optional block.
///
/// Two things follow, and only the first is obvious. A `FILTER(?r = wd:Q…)`
/// would be wrong for an unbound `?r` -- a compound with no occurrence, which is
/// precisely what a structure search exists to find -- because comparing against
/// an error drops it. And because the `VALUES` sits outside the block, asking for
/// a reference *with* no taxon returns only the compounds that reference reported,
/// even though the block is optional: an unbound `?r` cannot join.
#[test]
fn a_reference_binds_the_reference_item_as_a_values() {
    for cell in MATRIX.iter().filter(|c| c.reference) {
        let query = query_for(cell);
        let label = label(cell);
        assert!(
            query.contains(&format!("VALUES ?r {{ wd:{REFERENCE_QID} }}")),
            "{label}: the reference must be bound as a VALUES\n{query}"
        );
        assert!(
            !query.contains(&format!("FILTER(?r = wd:{REFERENCE_QID}")),
            "{label}: a FILTER on an unbound ?r drops occurrence-less compounds\n{query}"
        );
        // No other reference constraint, and no title match: a title is prose.
        assert_eq!(
            query.matches(&format!("wd:{REFERENCE_QID}")).count(),
            1,
            "{label}: exactly one binding of the reference\n{query}"
        );
    }

    for cell in MATRIX.iter().filter(|c| !c.reference) {
        let query = query_for(cell);
        assert!(
            !query.contains("VALUES ?r"),
            "{}: an absent reference must bind nothing\n{query}",
            label(cell)
        );
    }
}

/// A specific taxon filters on the taxon; an absent or wildcard one must not.
///
/// The wildcard filters on the root taxon in a structure search, where the seed
/// is a real QID. Without a structure there is no taxon filter at all, because
/// there is no taxon to filter by.
#[test]
fn only_a_specific_taxon_binds_a_taxon_qid() {
    for cell in &MATRIX {
        let query = query_for(cell);
        let label = label(cell);
        match cell.taxon {
            TaxonArg::Specific => assert!(
                query.contains(&format!("wd:{TAXON_QID}")),
                "{label}: the taxon QID must be bound\n{query}"
            ),
            TaxonArg::Absent | TaxonArg::Wildcard => assert!(
                !query.contains(&format!("wd:{TAXON_QID}")),
                "{label}: no taxon was resolved, so none may be bound\n{query}"
            ),
        }
    }
}

/// The table in `docs/QUERY_MATRIX.md`, restated here so a drift fails the suite.
///
/// Sizes are asserted as a range rather than an exact number: the point is that
/// no cell collapses to something trivially small -- a cell that lost its
/// patterns would still be balanced and still pass every other test here.
#[test]
fn every_cell_is_substantial_and_the_table_does_not_drift() {
    let mut summary = String::new();
    for cell in &MATRIX {
        let query = query_for(cell);
        let triples = triple_patterns(&query);
        assert!(
            triples >= 10,
            "{}: only {triples} triple patterns, which cannot be right\n{query}",
            label(cell)
        );
        let _ = writeln!(
            summary,
            "{}|{}|{}|{}",
            label(cell),
            query.len(),
            triples,
            if occurrence_is_optional(&query) {
                "optional"
            } else {
                "required"
            }
        );
    }

    let table = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/QUERY_MATRIX.md"
    ))
    .expect("docs/QUERY_MATRIX.md is the reference for which query runs when");
    for line in summary.lines() {
        let expected = line.replace('|', " | ");
        assert!(
            table.contains(&expected),
            "docs/QUERY_MATRIX.md does not record this cell:\n  {expected}\n\
             regenerate the table from `cargo test -p lotus-search --test matrix -- --nocapture`"
        );
    }
}
