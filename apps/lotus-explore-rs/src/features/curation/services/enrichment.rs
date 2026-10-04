// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Turning one corpus row into a curation result.
//!
//! A row resolves to one of two outcomes, and they are computed separately
//! because they share almost nothing: [`update_existing`] writes the statements
//! that fill gaps in an item Wikidata already has, [`create_new`] writes the
//! statements that make the item. Both consume the same converted structure and
//! the same resolved dependencies.
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]

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
pub(super) struct Converted {
    canonical_smiles: String,
    isomeric_smiles: String,
    pub(super) inchikey: String,
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

pub(super) async fn convert_structure(smiles: &str) -> Result<Converted, CurationError> {
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
#[path = "enrichment/tests.rs"]
mod tests;
