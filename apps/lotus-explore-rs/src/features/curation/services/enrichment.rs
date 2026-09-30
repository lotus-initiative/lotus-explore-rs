// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![allow(clippy::future_not_send)]

use super::occurrence_cache::{
    OccurrenceAskCache, compound_has_taxon_cached, compound_has_taxon_with_ref_cached,
};
use super::{
    CurationError, CurationInputRow, CurationResultRow, CurationStatus, DependencyResolution,
    MassResolution, QS_REF_INFERRED_FROM_SMILES, WD_CHEMICAL_COMPOUND_QID, WD_OCCURS_IN_TAXON_PROP,
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
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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

// One corpus row pipelines through many dependent resolution steps
// (smiles convert, exact mass, taxon/reference lookups, quickstatements
// assembly); splitting it would force threading a dozen intermediates
// through private helpers, obscuring the ordering that matters.
#[allow(clippy::too_many_lines)]
// A large, genuinely two-armed `match` (`Some`/`None` differ in ~equal
// mass); `if let ... else` would re-indent 270 lines without aiding clarity.
#[allow(clippy::single_match_else)]
async fn enrich_and_generate(
    locale: Locale,
    input: &CurationInputRow,
    repository: &dyn CurationKnowledgeRepository,
    prefetched_taxa: &HashMap<String, String>,
    prefetched_references: &HashMap<String, String>,
    occurrence_ask_cache: &Mutex<OccurrenceAskCache>,
) -> Result<CurationResultRow, CurationError> {
    let converted = convert_smiles(&input.smiles).await?;
    let formula_from_inchi =
        extract_formula_from_inchi(&converted.inchi).map(|f| normalize_formula_for_wikidata(&f));
    let normalized_doi = input.doi.as_deref().and_then(normalize_doi);
    let wd_compound = repository
        .fetch_compound_by_inchikey(&converted.inchikey)
        .await?;

    let result = match wd_compound {
        Some(existing) => {
            let mass_resolution = if existing.mass.is_none() {
                resolve_exact_mass(&input.smiles, &converted.canonical_smiles).await
            } else {
                MassResolution {
                    exact_mass: None,
                    warning: None,
                }
            };
            let exact_mass = mass_resolution.exact_mass;

            let mut lines: Vec<String> = Vec::new();
            let mut changes = 0usize;
            let mut status = CurationStatus::ExistingComplete;
            let mut note = curation_note_existing_complete(locale).into();

            if existing.canonical_smiles.is_none() {
                lines.push(qs_canonical_smiles_statement(
                    &existing.qid,
                    &converted.canonical_smiles,
                    has_isomeric_smiles(&converted.isomeric_smiles),
                ));
                changes += 1;
            }
            if existing.inchi.is_none() {
                lines.push(qs_inchi_statement(&existing.qid, &converted.inchi));
                changes += 1;
            }
            if existing.formula.is_none() && formula_from_inchi.is_some() {
                let formula = formula_from_inchi.as_deref().unwrap_or_default();
                lines.push(qs_statement_with_refs(
                    &existing.qid,
                    "P274",
                    formula,
                    &[QS_REF_INFERRED_FROM_SMILES],
                ));
                changes += 1;
            }
            if existing.mass.is_none() && exact_mass.is_some() {
                lines.push(qs_mass_statement(
                    &existing.qid,
                    exact_mass.unwrap_or_default(),
                ));
                changes += 1;
            }
            if has_isomeric_smiles(&converted.isomeric_smiles) && existing.isomeric_smiles.is_none()
            {
                lines.push(qs_isomeric_smiles_statement(
                    &existing.qid,
                    &converted.isomeric_smiles,
                ));
                changes += 1;
            }

            let dependencies = resolve_row_dependencies(
                locale,
                input,
                normalized_doi.as_deref(),
                repository,
                prefetched_taxa,
                prefetched_references,
            )
            .await?;

            if let Some(deps) = dependencies.as_ref() {
                let (should_add, p703) = match (
                    deps.taxon_qid.as_deref(),
                    deps.reference_qid.as_deref(),
                    normalized_doi.as_deref(),
                ) {
                    (Some(tqid), Some(rqid), _) => {
                        let add = !compound_has_taxon_with_ref_cached(
                            repository,
                            occurrence_ask_cache,
                            &existing.qid,
                            tqid,
                            rqid,
                        )
                        .await?;
                        (
                            add,
                            format!(
                                "{}|{}|{}|S248|{}",
                                existing.qid, WD_OCCURS_IN_TAXON_PROP, tqid, rqid
                            ),
                        )
                    }
                    // Reference item does not exist yet: generate it in dependencies, then rerun.
                    (Some(_), None, Some(_)) => (false, String::new()),
                    (Some(tqid), None, None) => {
                        let add = !compound_has_taxon_cached(
                            repository,
                            occurrence_ask_cache,
                            &existing.qid,
                            tqid,
                        )
                        .await?;
                        (
                            add,
                            format!("{}|{}|{}", existing.qid, WD_OCCURS_IN_TAXON_PROP, tqid),
                        )
                    }
                    // No taxon: add placeholder for new taxon
                    (None, _, _) => (
                        true,
                        format!("LAST|{WD_OCCURS_IN_TAXON_PROP}|[NEW_TAXON_QID]"),
                    ),
                };

                if should_add {
                    lines.push(p703);
                    changes += 1;
                }

                if !deps.pending_messages.is_empty() {
                    status = CurationStatus::PendingDependencies;
                    note = format!(
                        "{}\n{}",
                        curation_note_dependencies_pending(locale),
                        deps.pending_messages.join("\n")
                    );
                }
            }

            if matches!(status, CurationStatus::ExistingComplete) {
                if changes == 0 {
                    note = curation_note_existing_complete(locale).into();
                } else {
                    status = CurationStatus::ExistingNeedsUpdates;
                    note = curation_note_existing_updates(locale).into();
                }
            }

            CurationResultRow {
                input: input.clone(),
                canonical_smiles: Some(converted.canonical_smiles),
                inchikey: Some(converted.inchikey),
                inchi: Some(converted.inchi),
                formula: existing.formula.or(formula_from_inchi),
                exact_mass: existing.mass.or(exact_mass),
                mass_warning: if existing.mass.is_some() {
                    None
                } else {
                    mass_resolution.warning
                },
                wikidata_qid: Some(existing.qid.clone()),
                status,
                note,
                dependency_blocks: dependencies
                    .map(|deps| deps.dependency_blocks)
                    .unwrap_or_default(),
                quickstatements: lines,
            }
        }
        None => {
            let mass_resolution =
                resolve_exact_mass(&input.smiles, &converted.canonical_smiles).await;
            let exact_mass = mass_resolution.exact_mass;

            let dependencies = resolve_row_dependencies(
                locale,
                input,
                normalized_doi.as_deref(),
                repository,
                prefetched_taxa,
                prefetched_references,
            )
            .await?;
            let mut lines = vec!["CREATE".into()];
            lines.push(format!("LAST|Len|\"{}\"", escape_qs_string(&input.name)));
            lines.push("LAST|Den|\"chemical compound\"".into());

            // A structure the toolkit cannot read is not "no undefined
            // stereo", so the flag stays off rather than claiming the compound
            // is fully specified.
            if has_undefined_stereo(&input.smiles).await.unwrap_or(false) {
                lines.push(format!("LAST|P31|{WD_STEREOISOMER_GROUP_QID}"));
            } else {
                lines.push(format!("LAST|P31|{WD_TYPE_CHEMICAL_ENTITY_QID}"));
            }
            lines.push(format!("LAST|P279|{WD_CHEMICAL_COMPOUND_QID}"));
            lines.push(qs_inchikey_statement("LAST", &converted.inchikey));
            lines.push(qs_canonical_smiles_statement(
                "LAST",
                &converted.canonical_smiles,
                has_isomeric_smiles(&converted.isomeric_smiles),
            ));
            if has_isomeric_smiles(&converted.isomeric_smiles) {
                lines.push(qs_isomeric_smiles_statement(
                    "LAST",
                    &converted.isomeric_smiles,
                ));
            }
            lines.push(qs_inchi_statement("LAST", &converted.inchi));
            if let Some(formula) = formula_from_inchi.as_deref() {
                lines.push(qs_statement_with_refs(
                    "LAST",
                    "P274",
                    formula,
                    &[QS_REF_INFERRED_FROM_SMILES],
                ));
            }
            if let Some(mass) = exact_mass {
                lines.push(qs_mass_statement("LAST", mass));
            }

            if let Some(deps) = dependencies.as_ref() {
                let p703 = match (
                    deps.taxon_qid.as_deref(),
                    deps.reference_qid.as_deref(),
                    normalized_doi.as_deref(),
                ) {
                    (Some(tqid), Some(rqid), _) => {
                        format!("LAST|{WD_OCCURS_IN_TAXON_PROP}|{tqid}|S248|{rqid}")
                    }
                    (Some(tqid), None, None) => {
                        format!("LAST|{WD_OCCURS_IN_TAXON_PROP}|{tqid}")
                    }
                    // Reference item does not exist yet: generate it in dependencies, then rerun.
                    (Some(_), None, Some(_)) => String::new(),
                    (None, _, _) => {
                        format!("LAST|{WD_OCCURS_IN_TAXON_PROP}|[NEW_TAXON_QID]")
                    }
                };
                if !p703.is_empty() {
                    lines.push(p703);
                }
            }

            let (status, note) = dependencies.as_ref().map_or_else(
                || {
                    (
                        CurationStatus::NewCompound,
                        curation_note_new_compound(locale).into(),
                    )
                },
                |deps| {
                    if deps.pending_messages.is_empty() {
                        (
                            CurationStatus::NewCompound,
                            curation_note_new_compound(locale).into(),
                        )
                    } else {
                        (
                            CurationStatus::PendingDependencies,
                            format!(
                                "{}\n{}",
                                curation_note_dependencies_pending(locale),
                                deps.pending_messages.join("\n")
                            ),
                        )
                    }
                },
            );

            CurationResultRow {
                input: input.clone(),
                canonical_smiles: Some(converted.canonical_smiles),
                inchikey: Some(converted.inchikey),
                inchi: Some(converted.inchi),
                formula: formula_from_inchi,
                exact_mass,
                mass_warning: mass_resolution.warning,
                wikidata_qid: None,
                status,
                note,
                dependency_blocks: dependencies
                    .map(|deps| deps.dependency_blocks)
                    .unwrap_or_default(),
                quickstatements: lines,
            }
        }
    };

    Ok(result)
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
    let (ref_qid_opt, ref_new_qs) = if let Some(doi) = normalized_doi {
        let prefetched_ref_qid = prefetched_references.get(doi).map(String::as_str);
        resolve_or_create_reference(repository, doi, prefetched_ref_qid).await?
    } else {
        (None, Vec::new())
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
    if normalized_doi.is_some() && resolution.reference_qid.is_none() {
        resolution.pending_messages.push(curation_pending_reference(
            locale,
            normalized_doi.unwrap_or_default(),
        ));
    }

    Ok(Some(resolution))
}

/// Fetch pre-generated `QuickStatements` from citation.js (WASM only).
/// Uses the __lotusCitation JS bridge which fetches CSL JSON from doi.org and calls
async fn resolve_or_create_reference(
    repository: &dyn CurationKnowledgeRepository,
    doi: &str,
    pre_resolved_qid: Option<&str>,
) -> Result<(Option<String>, Vec<String>), CurationError> {
    if let Some(qid) = pre_resolved_qid {
        return Ok((Some(qid.into()), Vec::new()));
    }

    // Check if reference already exists in Wikidata
    if let Some(qid) = repository.resolve_reference_qid(doi).await? {
        return Ok((Some(qid), Vec::new()));
    }

    // Try to fetch QuickStatements from doi.org CSL JSON via JS bridge
    let qs_lines = fetch_reference_quickstatements(doi)
        .await
        .unwrap_or_default();

    let qs_lines = if qs_lines.is_empty() {
        vec!["## -- Step: create missing reference --".into()]
    } else {
        let mut lines = vec!["## -- Step: create missing reference --".into()];
        lines.extend(qs_lines);
        lines
    };

    Ok((None, qs_lines))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    #[test]
    fn reference_step_header_always_present() {
        // When QS lines are empty, only the header should be present.
        let qs_lines: Vec<String> = Vec::new();
        let result = if qs_lines.is_empty() {
            vec!["## -- Step: create missing reference --".to_string()]
        } else {
            let mut lines = vec!["## -- Step: create missing reference --".to_string()];
            lines.extend(qs_lines);
            lines
        };
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "## -- Step: create missing reference --");

        // When QS lines are present, the header is still the first line.
        let qs_lines: Vec<String> = vec!["CREATE".to_string(), "LAST|Len|\"Foo\"".to_string()];
        let result = if qs_lines.is_empty() {
            vec!["## -- Step: create missing reference --".to_string()]
        } else {
            let mut lines = vec!["## -- Step: create missing reference --".to_string()];
            lines.extend(qs_lines);
            lines
        };
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "## -- Step: create missing reference --");
        assert_eq!(result[1], "CREATE");
    }
}
