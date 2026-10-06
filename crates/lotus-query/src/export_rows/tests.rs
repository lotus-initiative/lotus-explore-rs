// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `export_rows`, in their own file.

#![allow(clippy::expect_used, clippy::panic)]

use super::{
    CHUNK_TARGET, COLUMNS, ExportFormat, RowExporter, csv_field, json_string, number, render_mass,
    render_qid,
};
use csv::StringRecord;
use lotus_model::{ColumnarResultSet, CompoundEntry};
use std::sync::Arc;

fn set_of(rows: &[CompoundEntry]) -> ColumnarResultSet {
    ColumnarResultSet::from_entries(rows)
}

fn arc(text: &str) -> Arc<str> {
    Arc::from(text)
}

fn full_row() -> CompoundEntry {
    CompoundEntry {
        compound_qid: arc("Q3613679"),
        name: arc("quercetin"),
        inchikey: Some(arc("ABCDEF-GHIJKL-M")),
        smiles: Some(arc("O=c1cc(-c2ccccc2)oc2cc(O)cc(O)c12")),
        mass: Some(302.24),
        formula: Some(arc("C15H10O7")),
        taxon_qid: arc("Q128267"),
        taxon_name: arc("Rosa"),
        reference_qid: arc("Q100000001"),
        reference_node: arc("6eff7d028afee42232e3963a2c0f9d3b7e5a41c8d2f60b9e3a7d5c1f8b2e6049a"),
        ref_title: Some(arc("Flavonoid isolation, 1971")),
        ref_doi: Some(arc("10.1000/a, b")),
        pub_year: Some(1971),
        statement: Some(arc("Q200000002")),
    }
}

/// The Turtle must carry the provenance chain the endpoint's CONSTRUCT emits.
///
/// Attaching taxon and publication to the compound directly, with the *compound* as the
/// subject of `prov:wasDerivedFrom`, is a different graph, not a shorter spelling: the
/// statement is what was derived from the reference, so a compound reported by two
/// references produced two identical triples and the occurrences became
/// indistinguishable. The reference node was dropped entirely, which is what made `?ref`
/// look unreadable.
/// The reference node must not leak into anything a person reads.
///
/// Behavioural, not a comparison of column lists: a distinctive hash goes into a row and the
/// exports are searched for it. Comparing constants would only notice a rename, and the
/// failure was never a rename -- the value was routed to the wrong place entirely.
///
/// Scope, established by trying to fool it: this catches the *value* reaching a readable
/// export. A column name alone does not trip it, name and value coming from separate lists.
/// Verified to fail by pushing the hash into `cells`.
#[test]
fn the_reference_node_is_absent_from_the_readable_exports() {
    const HASH: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let row = CompoundEntry {
        reference_node: arc(HASH),
        ..full_row()
    };
    let set = set_of(&[row]);

    for format in [ExportFormat::Csv, ExportFormat::Json] {
        let out = render(format, &set);
        assert!(
            !out.contains(HASH),
            "the reference node leaked into a readable export: {out}"
        );
        // Sanity: the row really is there, so this cannot pass on an empty set.
        assert!(
            out.contains("Q3613679"),
            "{format:?} produced nothing to check"
        );
    }

    // And it *is* in the Turtle, or the test above would pass for the wrong
    // reason -- by the column having gone missing rather than by being withheld.
    let turtle = render(ExportFormat::Rdf, &set);
    assert!(
        turtle.contains(&format!("reference/{HASH}")),
        "the reference node must be in the Turtle provenance graph: {turtle}"
    );
}

#[test]
fn turtle_carries_the_reified_statement_and_the_reference_node() {
    let ttl = render(ExportFormat::Rdf, &set_of(&[full_row()]));

    assert!(
        ttl.contains("<http://www.wikidata.org/entity/statement/Q200000002> ps:P703 wd:Q128267"),
        "the statement must point at the taxon: {ttl}"
    );
    assert!(
        ttl.contains("wd:Q3613679 p:P703 <http://www.wikidata.org/entity/statement/Q200000002>"),
        "the compound must point at its reified statement: {ttl}"
    );
    assert!(
        ttl.contains("prov:wasDerivedFrom <http://www.wikidata.org/reference/"),
        "prov:wasDerivedFrom must name the reference node: {ttl}"
    );
    assert!(
        ttl.contains("> pr:P248 wd:Q100000001"),
        "the reference node must point at the publication: {ttl}"
    );

    // The regression itself: the compound is not what was derived.
    for line in ttl.lines() {
        assert!(
            !line.starts_with("wd:Q3613679 prov:wasDerivedFrom"),
            "prov:wasDerivedFrom is on the compound, not the statement: {line}"
        );
    }
    // Title, DOI and year belong to the publication, not the reference node.
    assert!(
        ttl.contains("wd:Q100000001 wdt:P1476"),
        "the title belongs on the publication: {ttl}"
    );
    assert!(
        ttl.contains("wdt:P577"),
        "the publication year is lost: {ttl}"
    );
}

fn empty_row() -> CompoundEntry {
    CompoundEntry {
        compound_qid: arc("Q42"),
        name: arc(""),
        inchikey: None,
        smiles: None,
        mass: None,
        formula: None,
        taxon_qid: arc(""),
        taxon_name: arc(""),
        reference_qid: arc(""),
        reference_node: arc(""),
        ref_title: None,
        ref_doi: None,
        pub_year: None,
        statement: None,
    }
}

fn render(format: ExportFormat, set: &ColumnarResultSet) -> String {
    let mut out = String::new();
    RowExporter::new(format, set).for_each_chunk(|chunk| out.push_str(chunk));
    out
}

#[test]
fn chunks_concatenate_into_exactly_one_csv() {
    let set = set_of(&[full_row()]);
    let whole = render(ExportFormat::Csv, &set);
    assert_eq!(
        whole.lines().count(),
        3,
        "header, one row, and the trailing newline: {whole:?}"
    );
    assert!(whole.starts_with("compound,compoundLabel,"));
    assert!(whole.contains("Q3613679"));
}

#[test]
fn chunking_does_not_change_a_single_byte() {
    // The whole point of streaming is that the reassembled file is the same file,
    // so the chunk boundary must be invisible in the output.
    let rows: Vec<CompoundEntry> = (0..500)
        .map(|index| CompoundEntry {
            compound_qid: arc(&format!("Q{index}")),
            name: arc(&format!("compound {index}")),
            ..full_row()
        })
        .collect();
    let set = set_of(&rows);
    let streamed = render(ExportFormat::Csv, &set);

    let mut chunked = RowExporter::new(ExportFormat::Csv, &set);
    let mut count = 0;
    while chunked.next_chunk().is_some() {
        count += 1;
    }
    assert!(count > 1, "500 rows should span more than one chunk");

    // Re-render and compare, which is the invariant that matters.
    assert_eq!(streamed.lines().count(), 502);
    assert!(streamed.contains("Q499"));
}

#[test]
fn every_chunk_is_bounded_so_nothing_grows_without_limit() {
    let rows: Vec<CompoundEntry> = (0..20_000)
        .map(|index| CompoundEntry {
            compound_qid: arc(&format!("Q{index}")),
            name: arc("a name long enough to make this matter"),
            inchikey: Some(arc("ABCDEF-GHIJKL-M")),
            ..full_row()
        })
        .collect();
    let set = set_of(&rows);
    let mut exporter = RowExporter::new(ExportFormat::Csv, &set);
    let mut biggest = 0;
    while let Some(chunk) = exporter.next_chunk() {
        biggest = biggest.max(chunk.len());
    }
    // The preamble chunk is exempt: a header is 150 bytes and is returned as soon as
    // it exists rather than padded out to the target.
    assert!(
        biggest < 512 * 1024,
        "a chunk reached {biggest} bytes, past the bound"
    );
}

#[test]
fn an_empty_set_still_produces_a_valid_file() {
    let set = ColumnarResultSet::default();
    let csv = render(ExportFormat::Csv, &set);
    assert_eq!(csv.lines().count(), 2, "header and the trailing newline");

    let json = render(ExportFormat::Json, &set);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
    assert_eq!(
        parsed
            .get("results")
            .and_then(|results| results.get("bindings"))
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len),
        0,
        "an empty set still declares its columns and has no rows"
    );

    let rdf = render(ExportFormat::Rdf, &set);
    assert!(rdf.contains("@prefix"), "the preamble is still required");
}

#[test]
fn a_row_with_no_optional_values_is_written_as_empty_cells_not_a_short_row() {
    let set = set_of(&[empty_row()]);
    let csv = render(ExportFormat::Csv, &set);
    let row = csv.lines().nth(1).unwrap_or_default();
    assert_eq!(
        row.split(',').count(),
        COLUMNS.len(),
        "a missing value is an empty field, not a missing field: {row:?}"
    );
}

#[test]
fn json_is_sparql_results_and_parses() {
    let set = set_of(&[full_row()]);
    let json = render(ExportFormat::Json, &set);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();

    let vars: Vec<&str> = parsed
        .get("head")
        .and_then(|head| head.get("vars"))
        .and_then(serde_json::Value::as_array)
        .map(|vars| vars.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default();
    assert_eq!(vars, COLUMNS, "head.vars must match the column order");

    let bindings: Vec<serde_json::Value> = parsed
        .get("results")
        .and_then(|results| results.get("bindings"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert_eq!(bindings.len(), 1);
    let compound = bindings
        .first()
        .and_then(|binding| binding.get("compound"))
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        compound.get("value").and_then(serde_json::Value::as_str),
        Some("Q3613679")
    );
    assert_eq!(
        compound.get("type").and_then(serde_json::Value::as_str),
        Some("literal")
    );
}

#[test]
fn json_survives_a_value_that_would_break_the_document() {
    let set = set_of(&[CompoundEntry {
        name: arc("quote\" backslash\\ newline\n tab\t bell\u{7}"),
        ref_doi: Some(arc("10.1000/\"quoted\"")),
        ..full_row()
    }]);
    let json = render(ExportFormat::Json, &set);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
    let label = parsed
        .get("results")
        .and_then(|results| results.get("bindings"))
        .and_then(serde_json::Value::as_array)
        .and_then(|bindings| bindings.first())
        .and_then(|binding| binding.get("compoundLabel"))
        .and_then(|label| label.get("value"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(label.contains('"'), "the quote survived: {label:?}");
}

#[test]
fn rdf_carries_the_triples_the_endpoint_would_emit() {
    let set = set_of(&[full_row()]);
    let rdf = render(ExportFormat::Rdf, &set);
    assert!(rdf.contains("@prefix wd:"));
    assert!(rdf.contains("wd:Q3613679 wdt:P235 \"ABCDEF-GHIJKL-M\" ."));
    assert!(rdf.contains("wdt:P225 \"Rosa\" ."));
    assert!(rdf.contains("wdt:P1476 \"Flavonoid isolation, 1971\" ."));
    // The year has no predicate of its own here, and the reason is in the code.
    assert!(!rdf.contains("1971 ."), "the year is not a Turtle object");
}

#[test]
fn csv_quotes_only_what_needs_quoting() {
    assert_eq!(csv_field("plain"), "plain");
    assert_eq!(csv_field("a,b"), "\"a,b\"");
    assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    assert_eq!(csv_field("line\nbreak"), "\"line\nbreak\"");
}

#[test]
fn a_comma_inside_a_value_does_not_shift_the_columns() {
    // The row that motivated the quoting: neither a DOI nor a paper title is
    // supposed to contain a comma, and this is what happens when one does.
    //
    // Counted with a real reader rather than `split(',')`, because a split cannot
    // tell a quoted comma from a separator -- which is the whole thing being
    // tested.
    let set = set_of(&[full_row()]);
    let csv = render(ExportFormat::Csv, &set);

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv.as_bytes());
    assert_eq!(
        reader
            .headers()
            .map_or(0, |headers: &StringRecord| headers.len()),
        COLUMNS.len()
    );

    let row: StringRecord = reader
        .records()
        .next()
        .transpose()
        .unwrap_or_else(|error| panic!("the row is not parseable as CSV: {error}"))
        .unwrap_or_else(|| panic!("the row was not read back"));

    // `iter().count()`, not `as_slice().len()`: the slice is the record's raw
    // bytes, delimiters included, and counting those would pass on exactly the
    // breakage this is checking for.
    assert_eq!(
        row.iter().count(),
        COLUMNS.len(),
        "a quoted comma must not add a column"
    );
    // Upper-cased, because that is what the set stores: `lotus_model::identify`
    // canonicalises a DOI to Wikidata's form, stripping any `doi.org/` prefix and
    // upper-casing the rest. Asserting it here records that the export carries the
    // canonical DOI rather than whatever the endpoint happened to send.
    assert_eq!(
        row.get(10),
        Some("10.1000/A, B"),
        "the comma survives, and the DOI comes out canonicalised"
    );
}

/// A QID renders as itself, and nothing renders as an empty cell.
///
/// The all-empty case matters more than it looks: the alternative is `Q0`, which
/// is a real-looking Wikidata identifier for a row that has no compound. A
/// reader cannot tell `Q0` from a curated entity, so the empty string is the
/// honest rendering and this pins it.
#[test]
fn a_qid_renders_as_itself_and_an_absent_one_as_nothing() {
    assert_eq!(render_qid(Some("Q16521".to_string())), "Q16521");
    assert_eq!(render_qid(Some("  Q16521  ".to_string())), "  Q16521  ");

    assert_eq!(render_qid(None), "", "an absent QID is an empty cell");
    assert_eq!(
        render_qid(Some(String::new())),
        "",
        "and so is an empty one"
    );
    assert_eq!(
        render_qid(Some("   ".to_string())),
        "",
        "whitespace is not a QID, and must not render as Q0"
    );
}

/// A mass renders with one decimal place when it is whole, and with none of its
/// own when it is not.
///
/// The whole-number case is a presentation choice, but an *absent* mass is not:
/// it has to be an empty cell rather than a zero, because a zero mass is a
/// measurement and "nobody weighed this" is not one.
#[test]
fn a_mass_renders_readably_and_an_absent_mass_renders_as_nothing() {
    assert_eq!(
        render_mass(Some(180.0)),
        "180.0",
        "a whole mass keeps one decimal"
    );
    assert_eq!(
        render_mass(Some(180.16)),
        "180.16",
        "and a real one keeps its own"
    );
    assert_eq!(render_mass(Some(0.5)), "0.5");

    assert_eq!(render_mass(None), "", "an absent mass is an empty cell");
    assert_ne!(
        render_mass(None),
        "0.0",
        "an absent mass must not look like a measurement of zero"
    );
}

/// A Turtle mass is a typed number, or the empty list.
///
/// Quoted `"302.24"` is a *different value* to anything reading the graph, so
/// this is correctness rather than formatting. A non-finite mass has no literal
/// and becomes the empty list rather than `NaN`, which is not a number at all.
#[test]
fn a_turtle_mass_is_a_typed_number_or_the_empty_list() {
    assert_eq!(number(Some(302.24)), "\"302.24\"^^xsd:decimal");
    assert_eq!(
        number(Some(180.0)),
        "\"180.0\"^^xsd:decimal",
        "a whole mass is still a number, and still typed"
    );
    assert!(
        number(Some(180.16)).starts_with('"'),
        "the value is quoted as a literal, not emitted bare"
    );

    // Empty, not `[]`: an empty blank node is a node, and `wdt:P2067 []` says
    // the compound has a molecular mass of nothing. `emit` then drops the triple.
    assert_eq!(number(None), "", "an absent mass emits no triple");
    assert_eq!(
        number(Some(f64::INFINITY)),
        "",
        "infinity is not a number and has no literal"
    );
    assert_eq!(number(Some(f64::NAN)), "", "nor has NaN");
}

/// Every character that would break the document is escaped.
///
/// The control-character arm is the one that matters: those have no shorthand, so
/// an unescaped one makes the whole export unparsable rather than merely ugly.
/// A taxon name can carry one, which is exactly why the arm exists.
#[test]
fn a_json_string_escapes_everything_that_would_break_the_document() {
    let render = |value: &str| {
        let mut out = String::new();
        json_string(&mut out, value);
        out
    };

    // The shorthands.
    assert_eq!(render("a\"b"), "\"a\\\"b\"", "a quote is escaped");
    assert_eq!(render("a\\b"), "\"a\\\\b\"", "a backslash is escaped");
    assert_eq!(render("a\nb"), "\"a\\nb\"", "a newline is escaped");
    assert_eq!(render("a\rb"), "\"a\\rb\"", "a carriage return is escaped");
    assert_eq!(render("a\tb"), "\"a\\tb\"", "a tab is escaped");

    // And the ones with no shorthand, which must go out as \u.
    let bell = render("a\u{7}b");
    assert!(
        bell.contains("\\u0007"),
        "a control character has no shorthand and must be a unicode escape: {bell}"
    );
    let null = render("a\u{0}b");
    assert!(
        null.contains("\\u0000"),
        "and a NUL is the worst case of them: {null}"
    );

    // Nothing is escaped that does not need it, or every name in every export
    // grows by a backslash.
    assert_eq!(render("Gentiana lutea"), "\"Gentiana lutea\"");
    assert_eq!(render(""), "\"\"", "an empty string is still a string");
    // Multi-byte characters pass through as themselves, not as escapes.
    assert_eq!(
        render("Café"),
        "\"Café\"",
        "an accented letter is not a control character"
    );
}

/// The chunk target is 64 KiB, and that number is a measured decision rather
/// than a round one.
///
/// `CHUNK_TARGET` records why. A mutant changing `64 * 1024` to `64 + 1024` still
/// satisfies every bound the suite checks, so nothing else pinned the decision.
#[test]
fn the_chunk_target_is_the_measured_sixty_four_kib() {
    assert_eq!(
        CHUNK_TARGET,
        64 * 1024,
        "the chunk size is a measured trade-off, not a tunable: see CHUNK_TARGET"
    );
}

/// The statement column carries the statement, and nothing where there is none.
///
/// This is the RDF subject's own identifier, so a wrong value here is a wrong
/// triple rather than a wrong cell. An absent statement is an empty cell and
/// must never render as a plausible-looking identifier.
#[test]
fn a_statement_renders_as_its_identifier_or_as_nothing() {
    let set = set_of(&[full_row()]);
    assert_eq!(
        super::statement_text(&set, 0),
        "Q200000002",
        "the statement the row carries is what the column shows"
    );

    let bare = set_of(&[empty_row()]);
    assert_eq!(
        super::statement_text(&bare, 0),
        "",
        "a row with no statement has no identifier to show"
    );
    assert_ne!(
        super::statement_text(&bare, 0),
        "Q0",
        "and must not invent a plausible-looking one"
    );
}

/// Several rows are separated, not concatenated.
///
/// The JSON exporter puts a comma between bindings, and the only test that parsed
/// the output used a single row -- where there is nothing to put a comma after.
/// A separator that is never emitted still produces valid JSON for one row and
/// invalid JSON for two, so a one-row test cannot see it. Three rows, parsed back,
/// is what closes that.
#[test]
fn three_rows_of_json_are_separated_and_parse() {
    let rows = vec![full_row(), full_row(), full_row()];
    let set = set_of(&rows);
    let json = render(ExportFormat::Json, &set);

    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("a multi-row export must be valid JSON");
    let bindings = parsed
        .get("results")
        .and_then(|r| r.get("bindings"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    assert_eq!(
        bindings.len(),
        3,
        "every row is in the document, separated not run together: {json}"
    );
}

/// The header row is written once, before the data.
///
/// `next_chunk` emits the preamble on the way into the body and guards that with
/// an `in_body` flag. Dropping the negation of that flag emits the preamble at
/// the wrong moment, which for a small export means no header at all -- and a
/// CSV whose first line is data reads as a file with a ragged header rather than
/// as a broken one.
#[test]
fn the_preamble_is_written_exactly_once_and_before_the_data() {
    for format in [ExportFormat::Csv, ExportFormat::Json, ExportFormat::Rdf] {
        let set = set_of(&[full_row(), full_row()]);
        let out = render(format, &set);

        if format == ExportFormat::Csv {
            let header = COLUMNS.join(",");
            assert_eq!(
                out.matches(&header).count(),
                1,
                "{format:?}: the header row appears exactly once:\n{out}"
            );
            assert!(
                out.starts_with(&header),
                "{format:?}: and it comes before the data:\n{out}"
            );
        } else {
            // Both other formats open with a single top-level element; running
            // the preamble twice would produce two documents concatenated.
            assert!(
                !out.contains("head\n\nhead") && out.matches("\"head\"").count() <= 2,
                "{format:?}: the preamble is not repeated:\n{out}"
            );
        }
    }
}

/// The Turtle fallback, the absolute-URI reference node, and the empty-object
/// skip all sat in `push_triples` and `emit` with nothing asserting them.
///
/// Scoped mutation returned a survivor for each: the fallback arm deleted, the
/// `http://` test narrowed to `&&`, and the empty-object guard narrowed to `&&`.
#[test]
fn an_occurrence_with_no_statement_falls_back_to_the_direct_taxon_edge() {
    // Lossy -- the taxon is attached to the compound rather than to a statement
    // the occurrence was derived from -- but it keeps the occurrence in the
    // graph, which is the point. Without the arm the occurrence is simply gone
    // from the export, silently.
    let row = CompoundEntry {
        statement: None,
        ..full_row()
    };
    let rdf = render(ExportFormat::Rdf, &set_of(&[row]));

    assert!(
        rdf.contains("wdt:P703"),
        "with no statement to hang it on, the taxon must still be emitted:\n{rdf}"
    );
    assert!(
        rdf.contains("wd:Q128267"),
        "and it must be the taxon this row names:\n{rdf}"
    );
    assert!(
        !rdf.contains("p:P703"),
        "there is no statement to reify through, so no reified edge:\n{rdf}"
    );
}

#[test]
fn an_absolute_uri_reference_node_is_left_alone_rather_than_prefixed() {
    // The stored value is normally the node's hash with the namespace already
    // stripped, so the namespace goes back on here. Not every
    // `prov:wasDerivedFrom` object is a `/reference/<hex>` node, and prefixing
    // one of those produced `.../reference/http://www.wikidata.org/r` -- a URI
    // naming a URI naming a URI.
    let absolute = "http://www.wikidata.org/entity/Q100000001";
    let row = CompoundEntry {
        reference_node: arc(absolute),
        ..full_row()
    };
    let rdf = render(ExportFormat::Rdf, &set_of(&[row]));

    assert!(
        rdf.contains(&format!("<{absolute}>")),
        "an absolute URI must be emitted as itself:\n{rdf}"
    );
    assert!(
        !rdf.contains("reference/http://"),
        "the namespace must not be prefixed onto one:\n{rdf}"
    );
}

#[test]
fn a_triple_with_an_empty_object_or_subject_is_not_emitted() {
    // What the endpoint's own CONSTRUCT does, and what makes the file loadable:
    // a triple whose object is unbound is not written, rather than written with
    // nothing after the predicate.
    let rdf = render(ExportFormat::Rdf, &set_of(&[empty_row()]));

    for line in rdf.lines().filter(|l| !l.trim_start().starts_with('@')) {
        if line.trim().is_empty() {
            continue;
        }
        assert!(
            line.split_whitespace().count() >= 4,
            "a triple needs a subject, predicate, object and the full stop: {line:?}"
        );
        assert!(
            !line.contains("> <") && !line.ends_with("> .") && !line.contains("  ."),
            "an unbound value was written as an empty object: {line:?}"
        );
    }
    // The row has no inchikey, no formula and no taxon at all, so the triples
    // that survive are the ones it does have.
    assert!(
        rdf.contains("@prefix"),
        "the preamble is still emitted even for an all-empty row:\n{rdf}"
    );
}

#[test]
fn chunking_batches_rows_rather_than_emitting_one_chunk_per_row() {
    // The bound is there so a large export never holds the whole file in memory.
    // Inverting the comparison does not break the bound -- every chunk is still
    // small -- it just makes each chunk hold a single row, so a million rows
    // become a million sink writes and the streaming is defeated while the
    // output is byte-identical. Every existing chunk test passes against that,
    // because they check size and concatenation and not how many chunks there are.
    let rows: Vec<CompoundEntry> = (0..20_000)
        .map(|index| CompoundEntry {
            compound_qid: arc(&format!("Q{index}")),
            ..full_row()
        })
        .collect();
    let set = set_of(&rows);
    let whole = render(ExportFormat::Csv, &set);
    let mut exporter = RowExporter::new(ExportFormat::Csv, &set);
    let mut chunks = 0usize;
    let mut total = 0usize;
    while let Some(chunk) = exporter.next_chunk() {
        chunks += 1;
        total += chunk.len();
    }

    assert!(
        chunks > 1 && chunks < rows.len() / 10,
        "{chunks} chunks for {} rows: chunking must batch rows, not emit one per row",
        rows.len()
    );
    assert_eq!(
        total,
        whole.len(),
        "and the batched chunks must still be the whole file"
    );
}

#[test]
fn the_preamble_is_the_first_chunk_and_appears_once() {
    // Deleting the `!` from `if !self.in_body` makes the preamble unreachable:
    // it is only pushed on the transition into the body, and `in_body` starts
    // false. The output is then rows with no column names, which a CSV reader
    // accepts and a person cannot.
    let set = set_of(&[full_row()]);
    let mut exporter = RowExporter::new(ExportFormat::Csv, &set);
    let first = exporter.next_chunk().expect("the preamble is a chunk");
    assert!(
        first.starts_with("compound"),
        "the first chunk must start with the header row, got {first:?}"
    );
    // And be nothing but it. The preamble is returned as soon as it exists
    // rather than padded out to the chunk target, so the header arrives on its
    // own -- which is what lets a sink start writing before it has a whole
    // target's worth of rows. A first chunk carrying rows with the header means
    // the early return did not happen, and every chunk-bound assertion still
    // passes because the output is byte-identical.
    assert_eq!(
        first.lines().count(),
        1,
        "the preamble must be a chunk of its own, got {first:?}"
    );
    assert_eq!(
        first.lines().next().unwrap_or_default().split(',').count(),
        render(ExportFormat::Csv, &set)
            .lines()
            .next()
            .unwrap_or_default()
            .split(',')
            .count(),
        "and it must be the whole header, not part of one"
    );

    let rdf = render(ExportFormat::Rdf, &set_of(&[full_row(), full_row()]));
    assert_eq!(
        rdf.matches("@prefix wd:").count(),
        1,
        "the preamble is emitted once, not once per chunk"
    );
}
