// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! How long the hot paths take, and how much memory they hold.
//!
//! Run with `cargo test -p lotus-query --release -- --ignored --nocapture bench`.
//! Release profile, because a debug build of a sort says nothing about a phone.
//!
//! The fixture is generated rather than recorded, at the fill rates and widths
//! measured against `QLever` on a 50,000-row export (see the `columnar` module
//! docs): an `InChIKey` on 20.7% of rows, a SMILES on 15.0%, a mass on 10.3%, a
//! formula on 8.3%, and about 1.2 rows per compound. Generating it keeps the file
//! out of the repository and lets the row count be a parameter, which is what the
//! interesting comparisons need.
//!
//! Every measurement prints its own result rather than asserting on a threshold:
//! a benchmark that fails on a slow machine is a test that gets deleted, and a
//! benchmark that passes silently tells you nothing.

// The panic lints keep library code free of panics on external input; a benchmark
// that trips one is reporting, not unwinding.
#![allow(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "a benchmark: one long function, and byte counts cast to f64 to divide \
              into megabytes"
)]

use lotus_model::{ColumnarResultSet, FilterSpec, Range};
use std::fmt::Write as _;
use std::time::Instant;

/// The current projection, in order. Written out rather than taken from
/// `SELECT_COLUMNS` because the row template below is positional, and a header
/// that has drifted from the template measures a row shape the code no longer
/// parses -- which is how the numbers in the module docs came to describe a
/// shape without `ref_node`, `ref_year` or `compound_smiles_conn`.
const HEADER: &str = "compound,compoundLabel,compound_inchikey,compound_smiles_conn,compound_smiles_iso,compound_mass,compound_formula,taxon,taxon_name,ref_qid,ref_node,ref_title,ref_doi,ref_year,statement_id\n";

/// A CSV payload shaped like the recorded one, at `rows` rows.
fn payload(rows: usize) -> String {
    let mut csv = String::with_capacity(HEADER.len() + rows * 300);
    csv.push_str(HEADER);
    for row in 0..rows {
        // ~1.2 rows per compound, matching the graph.
        let compound = 100_000_000 + row / 2;
        let taxon = 200_000 + (row % 37_771);
        let reference = 300_000 + (row % 91_706);
        let _ = write!(csv, "http://www.wikidata.org/entity/Q{compound},");
        // A label on 86% of rows, 36 characters on average.
        if row % 7 != 0 {
            let _ = write!(csv, "Compound-name-{row:06}-padding-to-width");
        }
        csv.push(',');
        if row % 5 == 0 {
            let _ = write!(csv, "ABCDEF-{row:06}-GH");
        }
        csv.push(',');
        if row % 7 == 2 {
            let _ = write!(csv, "C1=CC=CC=C1CCN{row:06}CCCCO");
        }
        csv.push(',');
        if row % 10 == 3 {
            let _ = write!(csv, "{}", 100.0 + f64::from((row % 400) as u16));
        }
        csv.push(',');
        if row % 12 == 4 {
            let _ = write!(csv, "C{}H{}O", row % 30, row % 60);
        }
        let _ = write!(csv, ",http://www.wikidata.org/entity/Q{taxon},");
        if row % 16 != 5 {
            let _ = write!(csv, "Taxon-name-{taxon}-binomial");
        }
        let _ = write!(csv, ",http://www.wikidata.org/entity/Q{reference},");
        // The reference node, now carried per row: a 40-hex identity.
        if row % 20 != 7 {
            let _ = write!(csv, "{:040x}", row.wrapping_mul(2_654_435_761) as u64);
        }
        csv.push(',');
        // A title, which is the largest single value in a real payload.
        if row % 4 == 0 {
            let _ = write!(csv, "A paper about compound {reference} and some padding");
        }
        csv.push(',');
        if row % 4 == 0 {
            let _ = write!(csv, "10.1000/paper-{reference}");
        }
        csv.push(',');
        // The publication year alone, as the query now projects it.
        if row % 3 == 0 {
            let _ = write!(csv, "{}", 1900 + (row % 120));
        }
        // A statement on 98.5% of rows, with a random-looking UUID suffix.
        if row % 67 != 33 {
            let _ = write!(csv, ",Q{compound}-{row:08X}-1111-2222-3333-444455556666");
        }
        csv.push('\n');
    }
    csv
}

fn millis(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Report the resident size of a finished set, as the row model predicts.
///
/// Walks the same fields the struct holds rather than asking the allocator, which
/// would report wasm or system noise instead of what this type costs.
fn resident_bytes(set: &ColumnarResultSet) -> usize {
    let rows = set.row_count();
    // The four per-row columns: three ids and a 16-byte statement (or 4-byte
    // fallback id), rounded up by the enum's discriminant.
    let per_row = 3 * 4 + 20;
    let mut total = rows * per_row;
    // Dictionaries: their string bytes plus a slot each.
    total += set.total_dictionary_bytes();
    total
}

fn bench(rows: usize) {
    let csv = payload(rows);
    let csv_bytes = csv.len();
    println!("\n=== {rows} rows, {csv_bytes} bytes of CSV ===");

    // Split the build into its two halves, because they are different work: the
    // splitter walks every byte, and the builder interns every field. Optimising
    // the wrong one is how a benchmark gets slower.
    let start = Instant::now();
    let split_records = {
        let mut splitter = lotus_query::CsvSplitter::new();
        let mut records = Vec::new();
        let mut fields = 0;
        for chunk in csv.as_bytes().chunks(64 * 1024) {
            records.clear();
            splitter.feed(chunk, &mut records);
            fields += records.iter().map(std::vec::Vec::len).sum::<usize>();
        }
        fields
    };
    let split = start.elapsed();
    println!(
        "split records     {:>9.1} ms   ({split_records} fields, no building)",
        millis(split)
    );

    let start = Instant::now();
    let set = lotus_query::parse_compounds_columnar(csv.as_bytes()).expect("the payload parses");
    let build = start.elapsed();
    println!(
        "parse + build     {:>9.1} ms   ({:.1} MB/s)",
        millis(build),
        csv_bytes as f64 / 1_048_576.0 / build.as_secs_f64()
    );
    println!(
        "  of which build  {:>9.1} ms   (interning, after the {:.1} ms split)",
        millis(build.saturating_sub(split)),
        millis(split)
    );

    let start = Instant::now();
    let stats = set.stats();
    let stats_time = start.elapsed();
    println!(
        "exact counts      {:>9.1} ms   (entries {} unique {})",
        millis(stats_time),
        stats.n_entries,
        stats.n_entries_unique
    );

    // Resident size, and what it came to per row.
    println!(
        "resident          {:>9.1} MB   ({:.1} B/row, {:.0}% of the CSV)",
        resident_bytes(&set) as f64 / 1_048_576.0,
        resident_bytes(&set) as f64 / rows as f64,
        100.0 * resident_bytes(&set) as f64 / csv_bytes as f64
    );

    // Where the resident bytes actually are, largest first.
    let mut costs = set.dictionary_costs();
    costs.sort_by_key(|c| std::cmp::Reverse(c.2));
    println!("  breakdown:");
    for (label, entries, bytes) in &costs {
        if *bytes > 1_000_000 {
            println!(
                "    {label:<22} {entries:>9} entries {:>8.1} MB",
                *bytes as f64 / 1_048_576.0
            );
        }
    }
    let per_row_cols = rows * 32;
    println!(
        "    {:<22} {:>9} rows  {:>8.1} MB   (the id columns)",
        "per-row id columns",
        rows,
        per_row_cols as f64 / 1_048_576.0
    );

    // A text filter, which scans the compound dictionary on every keystroke.
    let spec = FilterSpec {
        compound: "padding-to-width".to_owned(),
        ..Default::default()
    };
    let start = Instant::now();
    let plan = set.plan_filter(&spec);
    let plan_time = start.elapsed();
    let start = Instant::now();
    let survivors = plan.surviving_rows(&set);
    let scan_time = start.elapsed();
    println!(
        "plan text filter  {:>9.1} ms   (dictionary scan)",
        millis(plan_time)
    );
    println!(
        "scan rows         {:>9.1} ms   ({} of {rows} survive)",
        millis(scan_time),
        survivors.len()
    );

    // A numeric filter, over the compound dictionary.
    let spec = FilterSpec {
        mass: Some(Range {
            min: Some(150.0),
            max: Some(350.0),
        }),
        ..Default::default()
    };
    let start = Instant::now();
    let plan = set.plan_filter(&spec);
    println!(
        "plan mass filter  {:>9.1} ms   ({} survive)",
        millis(start.elapsed()),
        plan.surviving_count(&set)
    );

    // Sorting is the expensive one: it is a full sort of every row, cached per
    // column, so this is what a click on a column header costs.
    let start = Instant::now();
    let order = sorted_order(&set);
    println!(
        "sort by name      {:>9.1} ms   ({} rows)",
        millis(start.elapsed()),
        order.len()
    );
}

/// Every row offset, sorted by compound label.
fn sorted_order(set: &ColumnarResultSet) -> Vec<u32> {
    let mut order: Vec<u32> = (0..u32::try_from(set.row_count()).unwrap_or(0)).collect();
    order.sort_unstable_by(|&a, &b| {
        set.compound_label(a as usize)
            .cmp(&set.compound_label(b as usize))
            .then_with(|| a.cmp(&b))
    });
    order
}

#[test]
#[ignore = "a benchmark, not a test: run with --ignored --nocapture"]
fn bench_a_million_rows() {
    bench(1_000_000);
}

#[test]
#[ignore = "a benchmark, not a test: run with --ignored --nocapture"]
fn bench_two_million_rows() {
    bench(2_000_000);
}

#[test]
#[ignore = "a benchmark, not a test: run with --ignored --nocapture"]
fn bench_the_whole_graph() {
    bench(2_990_730);
}

/// Is the sort linear, or is there a cliff at a round number of rows?
///
/// The main `bench` above times `sorted_order` once per size, and once told a story: 8.6 ms
/// at 1,000,000 rows against 840.9 ms at 2,000,000. A 98x jump for twice the data is not a
/// property of a sort, and 8.6 ms for a million-row string sort implies ~2 ns per
/// comparison, which is not achievable either. One of the two numbers was wrong, and a
/// single sample cannot say which.
///
/// This sweeps sizes and repeats because the shape of the curve is the question: a linear
/// sort shows flat `ns per row`, a cliff one size where it jumps. Median and spread, set
/// built once per size so the measurement is the sort and not the parse.
///
/// ```bash
/// cargo test -p lotus-query --release --locked -- --ignored --nocapture bench_sort_scaling
/// ```
#[test]
#[ignore = "a benchmark: prints its numbers, asserts nothing"]
fn bench_sort_scaling() {
    use std::time::Instant;

    const SAMPLES: usize = 15;
    const SIZES: [usize; 7] = [
        50_000, 100_000, 250_000, 500_000, 1_000_000, 2_000_000, 2_990_730,
    ];

    println!("\n== sort by compound label: {SAMPLES} samples per size ==");
    println!(
        "{:>10} {:>11} {:>11} {:>11} {:>13}",
        "rows", "median_ms", "min_ms", "max_ms", "ns_per_row"
    );

    for rows in SIZES {
        let csv = payload(rows);
        let set =
            lotus_query::parse_compounds_columnar(csv.as_bytes()).expect("the payload parses");
        drop(csv);

        // One untimed pass, so the first timed sample is not paying for a cold
        // cache and a cold allocator.
        let _ = sorted_order(&set);

        let mut samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let start = Instant::now();
            let order = sorted_order(&set);
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(order.len());
        }
        samples.sort_by(f64::total_cmp);

        // Iterator access rather than indexing: the workspace denies
        // `clippy::indexing_slicing`, and `SAMPLES` is a non-zero const so these
        // are never absent.
        let mid = samples.len() / 2;
        let median = samples.get(mid).copied().unwrap_or_default();
        let min = samples.first().copied().unwrap_or_default();
        let max = samples.last().copied().unwrap_or_default();
        // `median` is milliseconds, so ms * 1e6 is nanoseconds; divide by rows for
        // ns per row. This is the number that makes the shape readable: flat means
        // linear, a jump means a cliff.
        let per_row = (median * 1_000_000.0) / (rows as f64);
        println!("{rows:>10} {median:>11.1} {min:>11.1} {max:>11.1} {per_row:>13.1}");
    }
}
