// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::{CurationResultRow, QuickStatementsBundle};

/// Escape a value for a `QuickStatements` scalar.
///
/// The format is pipe-separated with `"`-quoted scalars, and a value containing
/// a quote or a newline will otherwise end the scalar early, so the rest of it
/// is read as though it were further properties -- a compound called `Say "hi"`
/// would write a property named `hi`.
#[must_use]
pub fn escape_quickstatements(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}
use std::collections::HashSet;

/// Bundle a batch of curated rows into statements ready to paste.
///
/// Dependencies are deduplicated and separated from the rows that need them,
/// because a `QuickStatements` run stops at the first failure: a taxon that has
/// to exist before the occurrence statement cannot be submitted in the same
/// block as the statement that uses it.
#[must_use]
pub fn build_quickstatements_bundle(results: &[CurationResultRow]) -> QuickStatementsBundle {
    let mut seen_dependency_blocks = HashSet::<&str>::with_capacity(results.len());
    let mut dependencies = String::new();
    for block in results.iter().flat_map(|r| r.dependency_blocks.iter()) {
        let block = block.as_str();
        if !block.trim().is_empty() && seen_dependency_blocks.insert(block) {
            if !dependencies.is_empty() {
                dependencies.push_str("\n\n");
            }
            dependencies.push_str(block);
        }
    }

    let mut main = String::new();
    for (row_idx, row) in results
        .iter()
        .filter(|r| !r.quickstatements.is_empty())
        .enumerate()
    {
        if row_idx > 0 {
            main.push_str("\n\n");
        }
        for (line_idx, line) in row.quickstatements.iter().enumerate() {
            if line_idx > 0 {
                main.push('\n');
            }
            main.push_str(line);
        }
    }

    QuickStatementsBundle {
        dependencies: std::sync::Arc::<str>::from(dependencies),
        main: std::sync::Arc::<str>::from(main),
    }
}

#[cfg(test)]
#[path = "quickstatements/tests.rs"]
mod tests;
