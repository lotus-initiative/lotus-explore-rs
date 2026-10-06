// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `native`, in their own file.

#![allow(clippy::expect_used)]

use super::*;

#[test]
fn the_whole_body_becomes_a_set_with_exact_counts() {
    let payload = {
        use std::io::Write;

        let mut file = tempfile::NamedTempFile::new().expect("tempfile create");
        file.write_all(
            b"compound,compoundLabel,taxon,ref_qid\nhttp://www.wikidata.org/entity/Q1,One,http://www.wikidata.org/entity/Q10,http://www.wikidata.org/entity/Q20\nhttp://www.wikidata.org/entity/Q2,Two,http://www.wikidata.org/entity/Q11,http://www.wikidata.org/entity/Q21\n",
        )
        .expect("tempfile write");
        FetchedResultsPayload::TempFile(file)
    };

    let processed = process_full_results_csv(payload).expect("csv should parse");
    let stats = processed.set.stats();

    assert_eq!(
        processed.set.row_count(),
        2,
        "both rows are kept: nothing caps the native path either"
    );
    assert_eq!(stats.n_entries, 2);
    assert_eq!(stats.n_entries_unique, 2);
    assert_eq!(stats.n_compounds, 2);
}
