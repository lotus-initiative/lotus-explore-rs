// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Splitting a response body into records without assembling it.
//!
//! The property under test throughout is that **a record is a record regardless
//! of where the chunks fall**. Every case below is a payload fed one byte at a
//! time as well as in one piece, because a splitter that is correct only when the
//! transport happens to align is not correct.

// The panic lints keep library code free of panics on external input. A test
// that fails on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies, clippy::expect_used, clippy::panic)]

use lotus_query::{CsvColumnarReader, CsvSplitter, parse_compounds_columnar};

fn split_in_one_go(payload: &[u8]) -> Vec<Vec<Vec<u8>>> {
    let mut splitter = CsvSplitter::new();
    let mut out = Vec::new();
    splitter.feed(payload, &mut out);
    out
}

/// Split `payload` one byte at a time, which is the worst case a chunk boundary
/// can produce.
fn split_one_byte_at_a_time(payload: &[u8]) -> Vec<Vec<Vec<u8>>> {
    let mut splitter = CsvSplitter::new();
    let mut out = Vec::new();
    for byte in payload {
        splitter.feed(std::slice::from_ref(byte), &mut out);
    }
    out
}

fn as_strings(records: &[Vec<Vec<u8>>]) -> Vec<Vec<String>> {
    records
        .iter()
        .map(|record| {
            record
                .iter()
                .map(|f| String::from_utf8_lossy(f).into_owned())
                .collect()
        })
        .collect()
}

fn assert_splits_the_same_way(payload: &[u8]) {
    assert_eq!(
        as_strings(&split_in_one_go(payload)),
        as_strings(&split_one_byte_at_a_time(payload)),
        "the split must not depend on where the chunk boundaries fall:\n{}",
        String::from_utf8_lossy(payload)
    );
}

#[test]
fn a_simple_payload_splits_into_records_and_fields() {
    assert_eq!(
        as_strings(&split_in_one_go(b"a,b\nc,d\n")),
        vec![vec!["a", "b"], vec!["c", "d"]]
    );
}

#[test]
fn a_quoted_field_keeps_its_delimiters() {
    // Reference titles contain commas and quotes. A splitter that does not honour
    // quoting shifts every column to the right of the title.
    let payload = b"Q1,\"a, b\",Q3\nQ4,\"say \"\"hi\"\"\",Q6\n";

    assert_eq!(
        as_strings(&split_in_one_go(payload)),
        vec![vec!["Q1", "a, b", "Q3"], vec!["Q4", r#"say "hi""#, "Q6"],]
    );
    assert_splits_the_same_way(payload);
}

#[test]
fn a_newline_inside_a_quoted_field_is_not_a_record_boundary() {
    let payload = b"Q1,\"line one\nline two\",Q3\n";

    assert_eq!(
        as_strings(&split_in_one_go(payload)),
        vec![vec!["Q1", "line one\nline two", "Q3"]]
    );
    assert_splits_the_same_way(payload);
}

#[test]
fn a_quote_split_across_two_chunks_is_still_a_quote() {
    let mut splitter = CsvSplitter::new();
    let mut out = Vec::new();

    splitter.feed(b"Q1,\"say ", &mut out);
    assert!(out.is_empty(), "the record is not finished");
    splitter.feed(b"\"", &mut out);
    assert!(
        out.is_empty(),
        "one quote is ambiguous: it is either an escape or the end of the field"
    );
    splitter.feed(b"\" still\",Q3\n", &mut out);

    assert_eq!(
        as_strings(&out),
        vec![vec!["Q1", r#"say " still"#, "Q3"]],
        "the escaped quote survived the boundary"
    );
}

#[test]
fn a_payload_ending_inside_a_quote_still_yields_its_record() {
    // An unterminated quote is *reported*, not tolerated. This used to be flushed as if
    // it were a complete row, which is how a response that died mid-body came back as a
    // shorter result set rather than as an error.
    let mut splitter = CsvSplitter::new();
    let mut out = Vec::new();
    splitter.feed(b"Q1,\"unterminated", &mut out);

    assert!(out.is_empty(), "the record is genuinely incomplete");

    let truncated = splitter.end_of_input(&mut out);
    assert!(
        truncated,
        "ending inside a quoted field must be reported as truncation"
    );
    // The bytes are still flushed, because the flag is what the caller acts on.
    assert_eq!(as_strings(&out), vec![vec!["Q1", "unterminated"]]);
}

#[test]
fn a_final_row_without_a_trailing_newline_is_not_truncation() {
    // The other half of the contract, and the reason truncation is only detectable
    // inside a quote: CSV lets the last record omit its terminator, so a body ending on
    // a complete row is indistinguishable from a body cut between fields -- and treating
    // it as truncated would reject well-formed responses from an endpoint that simply
    // does not end with a newline.
    let mut splitter = CsvSplitter::new();
    let mut out = Vec::new();
    splitter.feed(b"Q1,name\nQ2,other", &mut out);

    assert!(
        !splitter.end_of_input(&mut out),
        "a clean ending is not truncation"
    );
    assert_eq!(
        as_strings(&out),
        vec![vec!["Q1", "name"], vec!["Q2", "other"]]
    );
}

#[test]
fn a_quoted_final_field_is_not_truncation() {
    // A body ending on the closing quote of a quoted field leaves the quote pending, and
    // a pending quote is not an open one.
    let mut splitter = CsvSplitter::new();
    let mut out = Vec::new();
    splitter.feed(b"Q1,\"a name\"", &mut out);

    assert!(
        !splitter.end_of_input(&mut out),
        "a closed quoted field is a complete row"
    );
}

#[test]
fn carriage_returns_do_not_end_up_in_the_last_field() {
    assert_eq!(
        as_strings(&split_in_one_go(b"a,b\r\nc,d\r\n")),
        vec![vec!["a", "b"], vec!["c", "d"]]
    );
    assert_splits_the_same_way(b"a,b\r\nc,d\r\n");
}

#[test]
fn an_empty_field_is_a_field_not_a_missing_one() {
    assert_eq!(
        as_strings(&split_in_one_go(b"a,,c\n")),
        vec![vec!["a", "", "c"]]
    );
    assert_splits_the_same_way(b"a,,c\n");
}

#[test]
fn a_utf8_character_split_across_chunks_is_not_corrupted() {
    // The splitter matches only ASCII delimiters and copies everything else
    // through, so a multi-byte character is reassembled by the record, not by
    // the chunk boundary.
    let payload = "a,café,z\n".as_bytes();

    assert_eq!(
        as_strings(&split_in_one_go(payload)),
        vec![vec!["a", "café", "z"]]
    );
    assert_splits_the_same_way(payload);
}

#[test]
fn the_splitter_reports_whether_it_is_mid_record() {
    let mut splitter = CsvSplitter::new();
    let mut out = Vec::new();

    splitter.feed(b"a,b\n", &mut out);
    assert!(
        !splitter.is_mid_record(),
        "a finished record leaves nothing pending"
    );

    splitter.feed(b"c,d", &mut out);
    assert!(splitter.is_mid_record());
}

const HEADER: &str = "compound,compoundLabel,compound_inchikey,compound_mass,compound_formula,taxon,taxon_name,ref_qid,statement\n";

#[test]
fn a_whole_payload_becomes_a_columnar_set_with_exact_counts() {
    let payload = format!(
        "{HEADER}\
         Q1,first,,120.5,C2H6O,Q10,Homo,Q100,http://www.wikidata.org/entity/statement/Q1-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE\n\
         Q2,second,,300,C6H12O6,Q10,Homo,Q100,\n\
         Q1,first,,120.5,C2H6O,Q10,Homo,Q100,\n"
    );
    let set = parse_compounds_columnar(payload.as_bytes()).expect("the payload is well formed");
    let stats = set.stats();

    assert_eq!(stats.n_entries, 3, "every row is kept, duplicates included");
    assert_eq!(
        stats.n_entries_unique, 2,
        "the first and third rows are the same triple"
    );
    assert_eq!(stats.n_compounds, 2);
    assert_eq!(stats.n_taxa, 1);
    assert_eq!(stats.n_references, 1);
}

#[test]
fn streaming_a_payload_in_arbitrary_chunks_gives_the_same_set() {
    let payload = format!(
        "{HEADER}\
         Q1,\"first, compound\",ABCDEF-1,120.5,C2H6O,Q10,\"Homo, sapiens\",Q100,http://www.wikidata.org/entity/statement/Q1-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE\n\
         Q2,second,ZZZZZZ-2,300,C6H12O6,Q11,Plant,Q200,\n"
    );
    let whole = parse_compounds_columnar(payload.as_bytes()).expect("whole payload parses");

    // Every chunk size, from one byte upwards, must land on the same set. The
    // sizes that matter are the ones that split a quote, a comma and a multi-byte
    // character.
    for size in 1..=payload.len() {
        let mut reader = CsvColumnarReader::new();
        let bytes = payload.as_bytes();
        for chunk in bytes.chunks(size) {
            reader
                .feed(chunk)
                .expect("every chunk size is a valid split");
        }
        let streamed = reader.finish().expect("the stream finishes");

        assert_eq!(
            streamed.stats(),
            whole.stats(),
            "chunk size {size} changed the counts"
        );
        assert_eq!(
            streamed.entry(0),
            whole.entry(0),
            "chunk size {size} changed row 0"
        );
        assert_eq!(
            streamed.entry(1),
            whole.entry(1),
            "chunk size {size} changed row 1"
        );
    }
}

#[test]
fn the_reader_reports_progress_and_the_header_it_saw() {
    let payload = format!("{HEADER}Q1,first,,120.5,C2H6O,Q10,Homo,Q100,\n");
    let mut reader = CsvColumnarReader::new();

    assert_eq!(reader.row_count(), 0);
    reader
        .feed(payload.as_bytes())
        .expect("the payload is well formed");

    assert_eq!(reader.row_count(), 1);
    assert_eq!(reader.byte_count(), payload.len());
    assert_eq!(
        reader.header().map(<[String]>::len),
        Some(9),
        "the header names every column the query selects"
    );
}

#[test]
fn a_payload_with_no_header_is_refused_rather_than_read_as_empty() {
    // Every column would resolve to absent and the set would come back empty
    // with no rows in it, which is indistinguishable from a search that found
    // nothing. That is a wrong answer the reader cannot see is wrong.
    let error = parse_compounds_columnar(&b"Q1,first\n"[..]).expect_err("a header is required");

    assert!(
        error.to_string().contains("expected columns"),
        "the error should say what was missing: {error}"
    );
}

#[test]
fn a_row_shorter_than_its_header_reads_as_missing_trailing_columns() {
    // The forgiving contract the row-at-a-time parser has: a payload that reads
    // but has the wrong shape degrades to fewer columns rather than failing.
    let payload = format!("{HEADER}Q1,only-a-name\n");
    let set = parse_compounds_columnar(payload.as_bytes()).expect("a short row is not an error");

    assert_eq!(set.row_count(), 1);
    assert_eq!(set.compound_qid_text(0).as_deref(), Some("Q1"));
    assert_eq!(set.taxon_qid(0), None);
    assert_eq!(set.stats().n_taxa, 0);
}

#[test]
fn a_row_with_no_compound_is_dropped() {
    let payload = format!("{HEADER},nameless,,,,,,\nQ1,first,,,,Q10,Homo,Q100,\n");
    let set = parse_compounds_columnar(payload.as_bytes()).expect("the payload is well formed");

    assert_eq!(set.row_count(), 1, "a row with no compound cannot be shown");
}

#[test]
fn a_bare_integer_qid_is_read_as_a_qid() {
    // The query projects QIDs as `xsd:integer(STRAFTER(STR(?x), "Q"))`, so a real
    // response holds `16521` where the item is Q16521.
    let payload = format!("{HEADER}16521,first,,,,16521,Homo,100,\n");
    let set = parse_compounds_columnar(payload.as_bytes()).expect("the payload is well formed");

    assert_eq!(
        set.compound_qid_text(0).as_deref(),
        Some("Q16521"),
        "a bare integer is projected back as the QID it stands for"
    );
    assert_eq!(set.taxon_qid_text(0).as_deref(), Some("Q16521"));
}

#[test]
fn the_isomeric_smiles_wins_over_the_connection_table() {
    let header = "compound,compound_smiles_iso,compound_smiles_conn\n";
    let payload = format!("{header}Q1,CCO,CCO\nQ2,,CCC\n");

    let mut reader = CsvColumnarReader::new();
    reader
        .feed(payload.as_bytes())
        .expect("the payload is well formed");
    let set = reader.finish().expect("the stream finishes");

    assert_eq!(
        set.entry(0).and_then(|e| e.smiles.map(|s| s.to_string())),
        Some("CCO".to_string()),
    );
    assert_eq!(
        set.entry(1).and_then(|e| e.smiles.map(|s| s.to_string())),
        Some("CCC".to_string()),
        "a compound with only the connection-table SMILES still has one"
    );
}
