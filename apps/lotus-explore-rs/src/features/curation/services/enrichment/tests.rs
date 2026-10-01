// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the curation pipeline, kept in their own file.
//!
//! These assert the exact `QuickStatements` lines a row produces, because that
//! string is what a curator submits: a wrong property or a missing reference QID
//! is a wrong edit to Wikidata, and nothing downstream would notice. The two
//! mock repositories differ only in whether Wikidata already records the
//! occurrence, so what is under test is the statement the row would produce.

// Test code: a failing assertion is how it reports.
#![allow(clippy::panic)]

use super::*;
use crate::features::curation::repositories::{BoxedFuture, ResolveTaxonResult};
use futures::executor::block_on;
use std::collections::HashMap as Map;

/// The backing values a `Row` borrows, so a test can keep them alive.
struct Fixture {
    input: CurationInputRow,
    structure: Converted,
}

impl Fixture {
    /// A structure as `RDKit` would have returned it, so the statement
    /// builders can be exercised without the toolkit.
    fn new() -> Self {
        Self {
            input: CurationInputRow {
                name: "ethanol".to_owned(),
                smiles: "CCO".to_owned(),
                taxon: None,
                doi: None,
            },
            structure: Converted {
                canonical_smiles: "CCO".to_owned(),
                isomeric_smiles: "C[C@H](O)C".to_owned(),
                inchikey: "LFQSCWFLJHTTHZ-UHFFFAOYSA-N".to_owned(),
                inchi: "InChI=1S/C2H6O/c1-2-3/h3H,2H2,1H3".to_owned(),
                formula: Some("C2H6O".to_owned()),
            },
        }
    }

    fn row(&self, dependencies: Option<DependencyResolution>) -> Row<'_> {
        Row {
            locale: Locale::En,
            input: &self.input,
            structure: &self.structure,
            doi: None,
            dependencies,
        }
    }

    fn row_with_doi<'a>(
        &'a self,
        taxon: Option<&'a str>,
        reference: Option<&'a str>,
        doi: Option<&'a str>,
    ) -> Row<'a> {
        Row {
            locale: Locale::En,
            input: &self.input,
            structure: &self.structure,
            doi,
            dependencies: Some(DependencyResolution {
                taxon_qid: taxon.map(str::to_owned),
                reference_qid: reference.map(str::to_owned),
                dependency_blocks: Vec::new(),
                pending_messages: Vec::new(),
            }),
        }
    }

    /// An item Wikidata has, missing every field the row could fill.
    fn bare_item() -> WikidataCompound {
        WikidataCompound {
            qid: "Q1503".to_owned(),
            canonical_smiles: None,
            isomeric_smiles: None,
            inchi: None,
            formula: None,
            mass: None,
        }
    }
}

#[test]
fn every_missing_field_gets_a_statement_and_nothing_else_does() {
    let fixture = Fixture::new();
    let (lines, changes) =
        missing_statements(&fixture.row(None), &Fixture::bare_item(), Some(180.16));

    let inferred = &QS_REF_INFERRED_FROM_SMILES;
    assert_eq!(changes, 5, "five fields were missing");
    assert_eq!(
        lines,
        vec![
            // P233 cites Q123282952, not the plain inferred-from-SMILES
            // reference, because it was inferred from the isomeric form.
            "Q1503|P233|\"CCO\"|S887|Q123282952".to_owned(),
            format!("Q1503|P234|\"{}\"|S887|{inferred}", fixture.structure.inchi),
            format!("Q1503|P274|\"C2H6O\"|S887|{inferred}"),
            format!("Q1503|P2067|+180.160000U483261|S887|{inferred}"),
            "Q1503|P2017|\"C[C@H](O)C\"".to_owned(),
        ],
        "identity first, then mass, then stereochemistry"
    );
}

#[test]
fn a_field_wikidata_already_has_is_not_re_offered() {
    let fixture = Fixture::new();
    let mut complete = Fixture::bare_item();
    complete.canonical_smiles = Some("CCO".to_owned());
    complete.isomeric_smiles = Some("C[C@H](O)C".to_owned());
    complete.inchi = Some(fixture.structure.inchi.clone());
    complete.formula = Some("C2H6O".to_owned());
    complete.mass = Some(180.16);

    let (lines, changes) = missing_statements(&fixture.row(None), &complete, Some(180.16));

    assert_eq!(changes, 0, "nothing is missing, so nothing is written");
    assert!(lines.is_empty(), "got {lines:?}");
}

#[test]
fn a_mass_wikidata_already_records_is_not_overwritten() {
    // The toolkit still reports a mass, and it must not replace Wikidata's.
    let fixture = Fixture::new();
    let mut item = Fixture::bare_item();
    item.mass = Some(180.16);
    item.formula = Some("C2H6O".to_owned());

    let (lines, _) = missing_statements(&fixture.row(None), &item, Some(1.0));

    assert!(
        !lines.iter().any(|line| line.starts_with("Q1503|P2067")),
        "got {lines:?}"
    );
}

#[test]
fn a_row_naming_no_taxon_has_no_dependencies() {
    let fixture = Fixture::new();
    let row = fixture.row(None);
    assert_eq!(pending_messages(&row).len(), 0, "expected no entries");
    assert_eq!(row.dependency_blocks().len(), 0, "expected no entries");
}

#[test]
fn an_unresolved_taxon_puts_the_row_in_pending() {
    let fixture = Fixture::new();
    let row = fixture.row(Some(DependencyResolution {
        taxon_qid: None,
        reference_qid: None,
        dependency_blocks: vec!["CREATE".to_owned()],
        pending_messages: vec!["taxon not found".to_owned()],
    }));

    let (status, note) = row.pending_outcome();

    assert_eq!(status, CurationStatus::PendingDependencies);
    assert!(
        note.contains("taxon not found"),
        "the curator has to be told which taxon: {note}"
    );
}

/// A repository that has no occurrence recorded, so the statement is offered.
struct NotRecorded;

/// A repository that has the occurrence already, so it is not offered again.
struct AlreadyRecorded;

macro_rules! repository_answering {
    ($name:ident, $occurrence:literal) => {
        impl CurationKnowledgeRepository for $name {
            fn fetch_compound_by_inchikey(
                &self,
                _inchikey: &str,
            ) -> BoxedFuture<'_, Result<Option<WikidataCompound>, CurationError>> {
                Box::pin(async { Ok(None) })
            }
            fn resolve_or_create_taxon(
                &self,
                _name: &str,
                _pre_resolved_qid: Option<&str>,
            ) -> BoxedFuture<'_, ResolveTaxonResult> {
                Box::pin(async { Ok((None, Vec::new())) })
            }
            fn resolve_reference_qid(
                &self,
                _doi: &str,
            ) -> BoxedFuture<'_, Result<Option<String>, CurationError>> {
                Box::pin(async { Ok(None) })
            }
            fn compound_has_taxon_with_ref(
                &self,
                _compound: &str,
                _taxon: &str,
                _reference: &str,
            ) -> BoxedFuture<'_, Result<bool, CurationError>> {
                Box::pin(async { Ok($occurrence) })
            }
            fn compound_has_taxon(
                &self,
                _compound: &str,
                _taxon: &str,
            ) -> BoxedFuture<'_, Result<bool, CurationError>> {
                Box::pin(async { Ok($occurrence) })
            }
            fn resolve_taxon_qids_batch(
                &self,
                _names: &[String],
            ) -> BoxedFuture<'_, Result<Map<String, String>, CurationError>> {
                Box::pin(async { Ok(Map::new()) })
            }
            fn resolve_reference_qids_batch(
                &self,
                _dois: &[String],
            ) -> BoxedFuture<'_, Result<Map<String, String>, CurationError>> {
                Box::pin(async { Ok(Map::new()) })
            }
        }
    };
}
repository_answering!(NotRecorded, false);
repository_answering!(AlreadyRecorded, true);

/// The `P703` statement `row` would produce, with repository `repo` deciding
/// whether the occurrence is already there.
fn occurrence_for<R: CurationKnowledgeRepository>(row: &Row<'_>, repo: &R) -> Option<String> {
    let mut item = Fixture::bare_item();
    item.mass = Some(180.16);
    item.formula = Some("C2H6O".to_owned());
    block_on(occurrence_statement(
        row,
        &item,
        repo,
        &Mutex::new(OccurrenceAskCache::default()),
    ))
    .unwrap_or_else(|e| panic!("the repository answered: {e}"))
}

#[test]
fn an_occurrence_wikidata_does_not_have_is_offered() {
    let fixture = Fixture::new();
    let row = fixture.row_with_doi(Some("Q16521"), None, None);

    assert_eq!(
        occurrence_for(&row, &NotRecorded).as_deref(),
        Some("Q1503|P703|Q16521")
    );
}

#[test]
fn an_occurrence_wikidata_already_has_is_not_offered_again() {
    let fixture = Fixture::new();
    let row = fixture.row_with_doi(Some("Q16521"), None, None);

    assert_eq!(
        occurrence_for(&row, &AlreadyRecorded),
        None,
        "re-adding it would be a no-op edit"
    );
}

#[test]
fn an_occurrence_is_qualified_when_a_reference_resolved() {
    let fixture = Fixture::new();
    let row = fixture.row_with_doi(Some("Q16521"), Some("Q1234"), Some("10.1/x"));

    assert_eq!(
        occurrence_for(&row, &NotRecorded).as_deref(),
        Some("Q1503|P703|Q16521|S248|Q1234")
    );
}

#[test]
fn an_occurrence_waits_for_its_reference_item() {
    // A DOI with no item yet: the reference's own statements go in the
    // dependency block, and P703 is submitted on the next pass.
    let fixture = Fixture::new();
    let row = fixture.row_with_doi(Some("Q16521"), None, Some("10.1/x"));

    assert_eq!(occurrence_for(&row, &NotRecorded), None);
}

#[test]
fn an_unresolved_taxon_still_gets_the_placeholder_statement() {
    // Nothing to ask Wikidata about, and the dependency block will create the
    // taxon, so the statement is written now and filled in on the next pass.
    let fixture = Fixture::new();
    let row = fixture.row_with_doi(None, None, None);

    assert_eq!(
        occurrence_for(&row, &NotRecorded).as_deref(),
        Some("Q1503|P703|[NEW_TAXON_QID]")
    );
}
