// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::{CurationResultRow, QuickStatementsBundle};
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
    let mut dependencies = Vec::with_capacity(results.len());
    for block in results.iter().flat_map(|r| r.dependency_blocks.iter()) {
        let block = block.as_str();
        if block.trim().is_empty() {
            continue;
        }
        if seen_dependency_blocks.insert(block) {
            dependencies.push(block);
        }
    }

    let mut main_sections = Vec::with_capacity(results.len());
    for row in results.iter().filter(|r| !r.quickstatements.is_empty()) {
        main_sections.push(row.quickstatements.join("\n"));
    }

    QuickStatementsBundle {
        dependencies: std::sync::Arc::<str>::from(dependencies.join("\n\n")),
        main: std::sync::Arc::<str>::from(main_sections.join("\n\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CurationInputRow, CurationStatus};

    #[test]
    fn deduplicates_dependencies_and_joins_sections() {
        let rows = vec![
            CurationResultRow {
                input: CurationInputRow {
                    name: "A".into(),
                    smiles: "C".into(),
                    taxon: None,
                    doi: None,
                },
                canonical_smiles: None,
                inchikey: None,
                inchi: None,
                formula: None,
                exact_mass: None,
                mass_warning: None,
                wikidata_qid: None,
                status: CurationStatus::NewCompound,
                note: String::new(),
                dependency_blocks: vec!["DEP-1".into(), "DEP-1".into()],
                quickstatements: vec!["MAIN-1A".into(), "MAIN-1B".into()],
            },
            CurationResultRow {
                input: CurationInputRow {
                    name: "B".into(),
                    smiles: "N".into(),
                    taxon: None,
                    doi: None,
                },
                canonical_smiles: None,
                inchikey: None,
                inchi: None,
                formula: None,
                exact_mass: None,
                mass_warning: None,
                wikidata_qid: None,
                status: CurationStatus::NewCompound,
                note: String::new(),
                dependency_blocks: vec!["DEP-1".into(), "DEP-2".into()],
                quickstatements: vec!["MAIN-2".into()],
            },
        ];

        let bundle = build_quickstatements_bundle(&rows);
        assert_eq!(bundle.dependencies.as_ref(), "DEP-1\n\nDEP-2");
        assert_eq!(bundle.main.as_ref(), "MAIN-1A\nMAIN-1B\n\nMAIN-2");
    }
}
