// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The table count shown while the bulk exporter fetches sequential requests.

#![allow(clippy::expect_used)]

use indicatif::{InMemoryTerm, ProgressBar, ProgressDrawTarget, ProgressStyle};

use super::{EXPORT_PROGRESS_STYLE, report_export_table};

#[test]
fn the_visible_export_count_advances_between_tables() {
    let terminal = InMemoryTerm::new(2, 100);
    let progress = ProgressBar::with_draw_target(
        Some(lotus_explore_core::export::pipeline::EXPORT_QUERY_COUNT),
        ProgressDrawTarget::term_like(Box::new(terminal.clone())),
    );
    progress.set_style(ProgressStyle::with_template(EXPORT_PROGRESS_STYLE).expect("valid style"));
    let mut completed = 0;

    report_export_table(&progress, &mut completed, "compound_taxon_reference");
    assert!(
        terminal
            .contents()
            .contains("0/16 Fetching compound_taxon_reference"),
        "the first request has started, but none has completed"
    );

    report_export_table(&progress, &mut completed, "compound_smiles_can");
    assert!(
        terminal
            .contents()
            .contains("1/16 Fetching compound_smiles_can"),
        "the first request must count as completed while the second is active"
    );

    report_export_table(&progress, &mut completed, "compound_smiles_iso");
    assert!(
        terminal
            .contents()
            .contains("2/16 Fetching compound_smiles_iso"),
        "the completed count must continue advancing"
    );
}
