// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Turning one corpus row into a curation result.
//!
//! A row resolves to one of two outcomes, and they are computed separately
//! because they share almost nothing: [`update_existing`] writes the statements
//! that fill gaps in an item Wikidata already has, [`create_new`] writes the
//! statements that make the item. Both consume the same converted structure and
//! the same resolved dependencies.
#![allow(clippy::future_not_send)]

use super::occurrence::Resolved;
use super::occurrence_cache::{
    OccurrenceAskCache, compound_has_taxon_cached, compound_has_taxon_with_ref_cached,
};
use super::{
    CurationError, CurationInputRow, CurationResultRow, CurationStatus, DependencyResolution,
    MassResolution, QS_REF_INFERRED_FROM_SMILES, WD_CHEMICAL_COMPOUND_QID,
    WD_STEREOISOMER_GROUP_QID, WD_TYPE_CHEMICAL_ENTITY_QID, convert_smiles,
    curation_note_dependencies_pending, curation_note_existing_complete,
    curation_note_existing_updates, curation_note_new_compound, curation_pending_reference,
    curation_pending_taxon, escape_qs_string, fetch_reference_quickstatements, has_isomeric_smiles,
    has_undefined_stereo, normalize_doi, normalize_taxon_lookup, qs_canonical_smiles_statement,
    qs_inchi_statement, qs_inchikey_statement, qs_isomeric_smiles_statement,
    qs_statement_with_refs, resolve_exact_mass,
};
use crate::features::curation::repositories::CurationKnowledgeRepository;
use crate::features::curation::services::helpers::{
    extract_formula_from_inchi, normalize_formula_for_wikidata, qs_mass_statement,
};
use crate::i18n::Locale;
use lotus_curation::WikidataCompound;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Curation for one row, with its failure reported as a result row.
///
/// The pipeline reports errors in-band because a batch is curated one row at a
/// time and a single unreadable structure must not discard the rest.
pub async fn curate_single_row(
    locale: Locale,
    input: CurationInputRow,
    repository: Arc<dyn CurationKnowledgeRepository>,
    prefetched_taxa: Arc<HashMap<String, String>>,
    prefetched_references: Arc<HashMap<String, String>>,
    occurrence_ask_cache: Arc<Mutex<OccurrenceAskCache>>,
) -> CurationResultRow {
    enrich_and_generate(
        locale,
        &input,
        repository.as_ref(),
        prefetched_taxa.as_ref(),
        prefetched_references.as_ref(),
        occurrence_ask_cache.as_ref(),
    )
    .await
    .unwrap_or_else(|err| CurationResultRow {
        input,
        canonical_smiles: None,
        inchikey: None,
        inchi: None,
        formula: None,
        exact_mass: None,
        mass_warning: None,
        wikidata_qid: None,
        status: CurationStatus::Error,
        note: err.to_string(),
        dependency_blocks: Vec::new(),
        quickstatements: Vec::new(),
    })
}

/// Everything both outcomes need from the row, resolved once.
struct Converted {
    canonical_smiles: String,
    isomeric_smiles: String,
    inchikey: String,
    inchi: String,
    /// The formula implied by the `InChI`, which is how a structure that names no
    /// formula still contributes one.
    formula: Option<String>,
}

async fn enrich_and_generate(
    locale: Locale,
    input: &CurationInputRow,
    repository: &dyn CurationKnowledgeRepository,
    prefetched_taxa: &HashMap<String, String>,
    prefetched_references: &HashMap<String, String>,
    occurrence_ask_cache: &Mutex<OccurrenceAskCache>,
) -> Result<CurationResultRow, CurationError> {
    let structure = convert_structure(&input.smiles).await?;
    let normalized_doi = input.doi.as_deref().and_then(normalize_doi);
    let existing = repository
        .fetch_compound_by_inchikey(&structure.inchikey)
        .await?;

    let dependencies = resolve_row_dependencies(
        locale,
        input,
        normalized_doi.as_deref(),
        repository,
        prefetched_taxa,
        prefetched_references,
    )
    .await?;

    let context = Row {
        locale,
        input,
        structure: &structure,
        doi: normalized_doi.as_deref(),
        dependencies,
    };

    match existing {
        Some(existing) => {
            update_existing(&context, &existing, repository, occurrence_ask_cache).await
        }
        None => create_new(&context).await,
    }
}

/// The inputs both outcomes share, borrowed for the length of one.
struct Row<'a> {
    locale: Locale,
    input: &'a CurationInputRow,
    structure: &'a Converted,
    /// The row's normalised `DOI`, if it named one.
    doi: Option<&'a str>,
    /// `None` when the row names no taxon, so it has no dependencies at all.
    dependencies: Option<DependencyResolution>,
}

impl Row<'_> {
    /// What the row resolved to, for the `P703` statement.
    fn resolved(&self) -> Resolved<'_> {
        Resolved {
            taxon_qid: self
                .dependencies
                .as_ref()
                .and_then(|deps| deps.taxon_qid.as_deref()),
            reference_qid: self
                .dependencies
                .as_ref()
                .and_then(|deps| deps.reference_qid.as_deref()),
            row_has_doi: self.doi.is_some(),
        }
    }

    /// The statements that must be submitted before the row's own.
    fn dependency_blocks(&self) -> Vec<String> {
        self.dependencies
            .as_ref()
            .map(|deps| deps.dependency_blocks.clone())
            .unwrap_or_default()
    }

    /// The status and note for a row whose dependencies are still outstanding.
    fn pending_outcome(&self) -> (CurationStatus, String) {
        let pending = self
            .dependencies
            .as_ref()
            .is_some_and(|deps| !deps.pending_messages.is_empty());
        let notes = self
            .dependencies
            .as_ref()
            .map(|deps| deps.pending_messages.join("\n"))
            .unwrap_or_default();
        if pending {
            (
                CurationStatus::PendingDependencies,
                format!(
                    "{}\n{notes}",
                    curation_note_dependencies_pending(self.locale)
                ),
            )
        } else {
            (
                CurationStatus::NewCompound,
                curation_note_new_compound(self.locale).into(),
            )
        }
    }
}

async fn convert_structure(smiles: &str) -> Result<Converted, CurationError> {
    let converted = convert_smiles(smiles).await?;
    Ok(Converted {
        formula: extract_formula_from_inchi(&converted.inchi)
            .map(|formula| normalize_formula_for_wikidata(&formula)),
        canonical_smiles: converted.canonical_smiles,
        isomeric_smiles: converted.isomeric_smiles,
        inchikey: converted.inchikey,
        inchi: converted.inchi,
    })
}

/// The statements that fill gaps in an item Wikidata already has.
async fn update_existing(
    row: &Row<'_>,
    existing: &WikidataCompound,
    repository: &dyn CurationKnowledgeRepository,
    occurrence_ask_cache: &Mutex<OccurrenceAskCache>,
) -> Result<CurationResultRow, CurationError> {
    let structure = row.structure;
    // Wikidata's own mass is authoritative, so the toolkit is only asked when the
    // item has none.
    let mass_resolution = if existing.mass.is_none() {
        resolve_exact_mass(&row.input.smiles, &structure.canonical_smiles).await
    } else {
        MassResolution::default()
    };

    let (mut lines, mut changes) = missing_statements(row, existing, mass_resolution.exact_mass);
    let mut status = CurationStatus::ExistingComplete;
    let mut note = curation_note_existing_complete(row.locale).into();

    if let Some(statement) =
        occurrence_statement(row, existing, repository, occurrence_ask_cache).await?
    {
        lines.push(statement);
        changes += 1;
    }

    let pending = pending_messages(row);
    if !pending.is_empty() {
        status = CurationStatus::PendingDependencies;
        note = format!(
            "{}\n{}",
            curation_note_dependencies_pending(row.locale),
            pending.join("\n")
        );
    } else if changes > 0 {
        status = CurationStatus::ExistingNeedsUpdates;
        note = curation_note_existing_updates(row.locale).into();
    }

    Ok(CurationResultRow {
        input: row.input.clone(),
        canonical_smiles: Some(structure.canonical_smiles.clone()),
        inchikey: Some(structure.inchikey.clone()),
        inchi: Some(structure.inchi.clone()),
        formula: existing
            .formula
            .clone()
            .or_else(|| structure.formula.clone()),
        exact_mass: existing.mass.or(mass_resolution.exact_mass),
        mass_warning: if existing.mass.is_some() {
            None
        } else {
            mass_resolution.warning
        },
        wikidata_qid: Some(existing.qid.clone()),
        status,
        note,
        dependency_blocks: row.dependency_blocks(),
        quickstatements: lines,
    })
}

/// Each statement Wikidata is missing, and how many that is.
///
/// The order is the order a curator reads them in: identity first (name,
/// structures, formula), then mass, then stereochemistry.
fn missing_statements(
    row: &Row<'_>,
    existing: &WikidataCompound,
    exact_mass: Option<f64>,
) -> (Vec<String>, usize) {
    let structure = row.structure;
    let mut lines = Vec::new();

    if existing.canonical_smiles.is_none() {
        lines.push(qs_canonical_smiles_statement(
            &existing.qid,
            &structure.canonical_smiles,
            has_isomeric_smiles(&structure.isomeric_smiles),
        ));
    }
    if existing.inchi.is_none() {
        lines.push(qs_inchi_statement(&existing.qid, &structure.inchi));
    }
    if existing.formula.is_none()
        && let Some(formula) = structure.formula.as_deref()
    {
        lines.push(qs_statement_with_refs(
            &existing.qid,
            "P274",
            formula,
            &[QS_REF_INFERRED_FROM_SMILES],
        ));
    }
    if existing.mass.is_none()
        && let Some(mass) = exact_mass
    {
        lines.push(qs_mass_statement(&existing.qid, mass));
    }
    if has_isomeric_smiles(&structure.isomeric_smiles) && existing.isomeric_smiles.is_none() {
        lines.push(qs_isomeric_smiles_statement(
            &existing.qid,
            &structure.isomeric_smiles,
        ));
    }

    let changes = lines.len();
    (lines, changes)
}

/// The `P703` statement, if Wikidata does not already have it.
///
/// Asks the repository whether the occurrence is recorded, except when the row
/// named no taxon: there is nothing to ask about, and the placeholder statement
/// is what the dependency block's taxon will satisfy.
async fn occurrence_statement(
    row: &Row<'_>,
    existing: &WikidataCompound,
    repository: &dyn CurationKnowledgeRepository,
    occurrence_ask_cache: &Mutex<OccurrenceAskCache>,
) -> Result<Option<String>, CurationError> {
    let resolved = row.resolved();
    let Some(statement) = resolved.statement_for(&existing.qid) else {
        return Ok(None);
    };

    let recorded = match (resolved.taxon_qid, resolved.reference_qid) {
        (Some(taxon), Some(reference)) => {
            compound_has_taxon_with_ref_cached(
                repository,
                occurrence_ask_cache,
                &existing.qid,
                taxon,
                reference,
            )
            .await?
        }
        (Some(taxon), None) => {
            compound_has_taxon_cached(repository, occurrence_ask_cache, &existing.qid, taxon)
                .await?
        }
        // No taxon: the dependency block creates it, so the statement is added
        // now and its placeholder is filled in on the next pass.
        (None, _) => false,
    };

    Ok((!recorded).then_some(statement))
}

fn pending_messages<'a>(row: &'a Row<'_>) -> &'a [String] {
    row.dependencies
        .as_ref()
        .map_or(&[][..], |deps| deps.pending_messages.as_slice())
}

/// The statements that make an item Wikidata does not have.
async fn create_new(row: &Row<'_>) -> Result<CurationResultRow, CurationError> {
    let structure = row.structure;
    let mass_resolution = resolve_exact_mass(&row.input.smiles, &structure.canonical_smiles).await;

    let mut lines = vec!["CREATE".into()];
    lines.push(format!(
        "LAST|Len|\"{}\"",
        escape_qs_string(&row.input.name)
    ));
    lines.push("LAST|Den|\"chemical compound\"".into());
    // A structure the toolkit cannot read is not "no undefined stereo", so the
    // flag stays off rather than claiming the compound is fully specified.
    if has_undefined_stereo(&row.input.smiles)
        .await
        .unwrap_or(false)
    {
        lines.push(format!("LAST|P31|{WD_STEREOISOMER_GROUP_QID}"));
    } else {
        lines.push(format!("LAST|P31|{WD_TYPE_CHEMICAL_ENTITY_QID}"));
    }
    lines.push(format!("LAST|P279|{WD_CHEMICAL_COMPOUND_QID}"));
    lines.push(qs_inchikey_statement("LAST", &structure.inchikey));
    lines.push(qs_canonical_smiles_statement(
        "LAST",
        &structure.canonical_smiles,
        has_isomeric_smiles(&structure.isomeric_smiles),
    ));
    if has_isomeric_smiles(&structure.isomeric_smiles) {
        lines.push(qs_isomeric_smiles_statement(
            "LAST",
            &structure.isomeric_smiles,
        ));
    }
    lines.push(qs_inchi_statement("LAST", &structure.inchi));
    if let Some(formula) = structure.formula.as_deref() {
        lines.push(qs_statement_with_refs(
            "LAST",
            "P274",
            formula,
            &[QS_REF_INFERRED_FROM_SMILES],
        ));
    }
    if let Some(mass) = mass_resolution.exact_mass {
        lines.push(qs_mass_statement("LAST", mass));
    }
    if let Some(statement) = row.resolved().statement_for_new() {
        lines.push(statement);
    }

    let (status, note) = row.pending_outcome();
    Ok(CurationResultRow {
        input: row.input.clone(),
        canonical_smiles: Some(structure.canonical_smiles.clone()),
        inchikey: Some(structure.inchikey.clone()),
        inchi: Some(structure.inchi.clone()),
        formula: structure.formula.clone(),
        exact_mass: mass_resolution.exact_mass,
        mass_warning: mass_resolution.warning,
        wikidata_qid: None,
        status,
        note,
        dependency_blocks: row.dependency_blocks(),
        quickstatements: lines,
    })
}

async fn resolve_row_dependencies(
    locale: Locale,
    input: &CurationInputRow,
    normalized_doi: Option<&str>,
    repository: &dyn CurationKnowledgeRepository,
    prefetched_taxa: &HashMap<String, String>,
    prefetched_references: &HashMap<String, String>,
) -> Result<Option<DependencyResolution>, CurationError> {
    let Some(taxon_name) = input.taxon.as_deref() else {
        return Ok(None);
    };

    let prefetched_taxon_qid = normalize_taxon_lookup(taxon_name)
        .and_then(|lookup| prefetched_taxa.get(&lookup))
        .map(String::as_str);
    let (taxon_qid_opt, taxon_new_qs) = repository
        .resolve_or_create_taxon(taxon_name, prefetched_taxon_qid)
        .await?;
    let (ref_qid_opt, ref_new_qs) = match normalized_doi {
        Some(doi) => {
            let prefetched_ref_qid = prefetched_references.get(doi).map(String::as_str);
            resolve_or_create_reference(repository, doi, prefetched_ref_qid).await?
        }
        None => (None, Vec::new()),
    };

    let mut resolution = DependencyResolution {
        taxon_qid: taxon_qid_opt,
        reference_qid: ref_qid_opt,
        ..DependencyResolution::default()
    };

    if !taxon_new_qs.is_empty() {
        resolution.dependency_blocks.push(taxon_new_qs.join("\n"));
    }
    if !ref_new_qs.is_empty() {
        resolution.dependency_blocks.push(ref_new_qs.join("\n"));
    }

    if resolution.taxon_qid.is_none() {
        resolution
            .pending_messages
            .push(curation_pending_taxon(locale, taxon_name));
    }
    if let Some(doi) = normalized_doi.filter(|_| resolution.reference_qid.is_none()) {
        resolution
            .pending_messages
            .push(curation_pending_reference(locale, doi));
    }

    Ok(Some(resolution))
}

/// Resolve a row's reference, or produce the statements that would create one.
///
/// Returns `None` for the item and the dependency block that creates it, because
/// the row's own statements cannot name a reference that does not exist.
async fn resolve_or_create_reference(
    repository: &dyn CurationKnowledgeRepository,
    doi: &str,
    pre_resolved_qid: Option<&str>,
) -> Result<(Option<String>, Vec<String>), CurationError> {
    if let Some(qid) = pre_resolved_qid {
        return Ok((Some(qid.into()), Vec::new()));
    }
    if let Some(qid) = repository.resolve_reference_qid(doi).await? {
        return Ok((Some(qid), Vec::new()));
    }

    let mut lines = vec![REFERENCE_STEP_HEADER.to_owned()];
    lines.extend(
        fetch_reference_quickstatements(doi)
            .await
            .unwrap_or_default(),
    );
    Ok((None, lines))
}

/// Heads the block that creates a missing reference, so a curator can see where
/// it begins in the dependency output.
const REFERENCE_STEP_HEADER: &str = "## -- Step: create missing reference --";

#[cfg(test)]
mod tests {
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
        assert!(pending_messages(&row).is_empty());
        assert!(row.dependency_blocks().is_empty());
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
}
