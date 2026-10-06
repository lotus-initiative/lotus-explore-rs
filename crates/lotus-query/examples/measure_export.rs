// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Measure what each export format actually costs per row, using the real
//! exporters and a payload shaped like the recorded one.

// A measurement harness, not library code: it casts freely to shape a payload
// and panics on a parse failure, which is the reporting a measurement needs.
#![allow(
    unused_crate_dependencies,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::expect_used,
    clippy::panic
)]
const HEADER: &str = "compound,compoundLabel,compound_inchikey,compound_smiles_conn,compound_smiles_iso,compound_mass,compound_formula,taxon,taxon_name,ref_qid,ref_node,ref_title,ref_doi,ref_year,statement_id\n";

fn payload(rows: usize) -> String {
    use std::fmt::Write as _;
    let mut csv = String::with_capacity(HEADER.len() + rows * 300);
    csv.push_str(HEADER);
    for row in 0..rows {
        let compound = 100_000_000 + row / 2;
        let taxon = 200_000 + (row % 37_771);
        let reference = 300_000 + (row % 91_706);
        let _ = write!(csv, "http://www.wikidata.org/entity/Q{compound},");
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
            let _ = write!(csv, "Taxon-name-{row:06}-of-the-group");
        }
        let _ = write!(csv, ",http://www.wikidata.org/entity/Q{reference},");
        let _ = write!(csv, ",http://www.wikidata.org/entity/Q{reference},");
        if row % 11 != 4 {
            let _ = write!(csv, "A paper about the compound, part {row:06}");
        }
        csv.push(',');
        if row % 13 != 6 {
            let _ = write!(csv, "10.10{row:04}/abcd");
        }
        csv.push(',');
        if row % 18 != 9 {
            let _ = write!(csv, "{}", 1990 + (row % 36));
        }
        let _ = writeln!(csv, ",{reference}-{row:06}");
    }
    csv
}

fn main() {
    use lotus_query::{ExportFormat, RowExporter};
    let rows = 200_000;
    let csv = payload(rows);
    let set = lotus_query::parse_compounds_columnar(csv.as_bytes()).expect("parses");
    assert_eq!(set.row_count(), rows, "row count survived the build");

    println!(
        "=== {rows} rows, source CSV {} bytes ({:.0} B/row) ===",
        csv.len(),
        csv.len() as f64 / rows as f64
    );

    for format in [ExportFormat::Csv, ExportFormat::Json, ExportFormat::Rdf] {
        let mut exporter = RowExporter::new(format, &set);
        let mut bytes = 0usize;
        let mut chunks = 0usize;
        while let Some(chunk) = exporter.next_chunk() {
            bytes += chunk.len();
            chunks += 1;
        }
        println!(
            "{:<5} {bytes:>12} bytes  {:>8.1} B/row  ({chunks} chunks)",
            format.log_name(),
            bytes as f64 / rows as f64
        );
    }
}
