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
use std::fmt::Write as _;

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

/// Create each new compound once per batch, however many rows name it.
///
/// A row for a compound Wikidata does not have carries a `CREATE` with its
/// occurrence inside it, because only `LAST` can point at an item that has no
/// QID yet. Two rows for one new compound -- the same molecule in two taxa --
/// therefore made two `CREATE`s, and pasting them made two items with one
/// `InChIKey`. The first row for each key keeps its `CREATE` and gains every
/// later row's `P703` occurrence line; the later rows keep no statements and
/// say where theirs went.
///
/// Works on both shapes the front ends emit: one block per element (the CLI's
/// `## name\nCREATE\n  LAST|...`) and one line per element (the web app's).
pub fn merge_new_compound_creates(rows: &mut [CurationResultRow]) {
    let mut first_for_key = std::collections::HashMap::<String, usize>::new();
    for idx in 0..rows.len() {
        let Some(row) = rows.get(idx) else { continue };
        if row.status != crate::CurationStatus::NewCompound {
            continue;
        }
        let Some(key) = row.inchikey.clone().filter(|k| !k.trim().is_empty()) else {
            continue;
        };
        if create_at(&row.quickstatements).is_none() {
            continue;
        }
        let Some(&first) = first_for_key.get(&key) else {
            first_for_key.insert(key, idx);
            continue;
        };

        let Some(later) = rows.get_mut(idx) else {
            continue;
        };
        let moved: Vec<String> = later
            .quickstatements
            .iter()
            .flat_map(|element| element.lines())
            .map(str::trim)
            .filter(|line| line.starts_with(OCCURRENCE_PREFIX))
            .map(str::to_owned)
            .collect();
        later.quickstatements.clear();
        let Some(first_row) = rows.get_mut(first) else {
            continue;
        };
        let first_name = first_row.input.name.trim().to_owned();
        for line in moved {
            add_to_create(&mut first_row.quickstatements, &line);
        }
        if let Some(later) = rows.get_mut(idx) {
            if !later.note.is_empty() {
                later.note.push_str("; ");
            }
            let _ = write!(
                later.note,
                "the same new compound as \"{first_name}\"; its occurrence is in that row's statements"
            );
        }
    }
}

/// `LAST|P703|`: an occurrence line for the item a `CREATE` just made.
const OCCURRENCE_PREFIX: &str = "LAST|P703|";

/// The index of the element that holds the row's `CREATE`.
fn create_at(statements: &[String]) -> Option<usize> {
    statements
        .iter()
        .position(|element| element.lines().any(|line| line.trim() == "CREATE"))
}

/// Add `line` to the `CREATE` in `statements`, unless it is already there.
fn add_to_create(statements: &mut Vec<String>, line: &str) {
    let already = statements
        .iter()
        .flat_map(|element| element.lines())
        .any(|existing| existing.trim() == line);
    let Some(at) = create_at(statements) else {
        return;
    };
    if already {
        return;
    }
    let Some(element) = statements.get_mut(at) else {
        return;
    };
    if element.contains('\n') {
        // Block form: the `LAST` lines are inside this element.
        element.push_str("\n  ");
        element.push_str(line);
    } else {
        // Line form: insert after the run of `LAST` lines that follows `CREATE`,
        // before anything that might start another item.
        let run_end = statements
            .iter()
            .skip(at + 1)
            .take_while(|element| element.trim_start().starts_with("LAST|"))
            .count();
        statements.insert(at + 1 + run_end, line.to_owned());
    }
}

#[cfg(test)]
#[path = "quickstatements/tests.rs"]
mod tests;
