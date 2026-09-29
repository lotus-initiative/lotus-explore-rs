// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub(super) const fn heading_add_one_row() -> &'static str {
    "Add one row"
}

pub(super) const fn heading_tsv_import() -> &'static str {
    "TSV import"
}

pub(super) const fn heading_queued_rows() -> &'static str {
    "Queued rows"
}

pub(super) const fn heading_results() -> &'static str {
    "Curation results"
}

pub(super) const fn heading_quickstatements() -> &'static str {
    "QuickStatements (copy-paste)"
}

pub(super) const fn heading_quickstatements_dependencies() -> &'static str {
    "QuickStatements — prerequisites"
}

pub(super) const fn placeholder_molecule_name() -> &'static str {
    "Molecule name"
}

pub(super) const fn placeholder_taxon_optional() -> &'static str {
    "Taxon (optional)"
}

pub(super) const fn placeholder_doi_optional() -> &'static str {
    "DOI (optional)"
}

pub(super) const fn button_add_row() -> &'static str {
    "Add row"
}

pub(super) const fn button_load_example_rows() -> &'static str {
    "Load example rows"
}

pub(super) const fn button_append_tsv_rows() -> &'static str {
    "Append TSV rows"
}

pub(super) const fn button_generate_quickstatements() -> &'static str {
    "Generate QuickStatements"
}

pub(super) const fn button_generating() -> &'static str {
    "Generating..."
}

pub(super) const fn button_remove() -> &'static str {
    "Remove"
}

pub(super) const fn col_name() -> &'static str {
    "Name"
}

pub(super) const fn col_action() -> &'static str {
    "Action"
}

pub(super) const fn col_original_smiles() -> &'static str {
    "Original SMILES"
}

pub(super) const fn col_canonical_smiles() -> &'static str {
    "Canonical SMILES"
}

pub(super) const fn col_exact_mass() -> &'static str {
    "Exact mass"
}

pub(super) const fn col_status() -> &'static str {
    "Status"
}

pub(super) const fn label_new_item() -> &'static str {
    "new item"
}

pub(super) const fn hint_expected_tsv_headers() -> &'static str {
    "Expected headers: name, smiles, taxon (or organism), doi"
}

pub(super) const fn hint_scroll_curation_results() -> &'static str {
    "Tip: Swipe horizontally to review every results column."
}

pub(super) fn msg_name_smiles_required() -> String {
    "Name and SMILES are required to add a row.".to_string()
}

pub(super) fn msg_duplicate_row_skipped() -> String {
    "Duplicate row skipped (same structure/taxon/reference).".to_string()
}

pub(super) fn msg_no_valid_tsv_rows() -> String {
    "No valid rows found in TSV input.".to_string()
}

pub(super) fn msg_tsv_missing_column(column: &str) -> String {
    format!("The required '{column}' column is missing from the TSV input.")
}

pub(super) fn msg_tsv_import_complete(added: usize, skipped: usize) -> String {
    format!("TSV import complete: added {added} unique row(s), skipped {skipped} duplicate(s).")
}

pub(super) fn msg_examples_loaded(added: usize, skipped: usize) -> String {
    format!("Examples loaded: added {added} unique row(s), skipped {skipped} duplicate(s).")
}

pub(super) fn msg_add_row_before_generate() -> String {
    "Add at least one row before generating QuickStatements.".to_string()
}

pub(super) fn msg_running_checks() -> String {
    "Running curation checks with RDKit.js and Wikidata...".to_string()
}

pub(super) fn msg_done_review_copy() -> String {
    "Done. Review generated rows and copy the QuickStatements block.".to_string()
}

pub(super) fn msg_curation_failed(detail: &str) -> String {
    format!("Curation failed: {detail}")
}

pub(super) const fn msg_curation_rate_limited() -> &'static str {
    "Rate limit reached on an upstream metadata service (HTTP 429). Wait about 60s and retry."
}

pub(super) fn msg_prerequisites_pending(count: usize) -> String {
    format!(
        "{count} row(s) still wait for prerequisite entities. Run prerequisites, create/merge them in Wikidata, then run second pass."
    )
}

pub(super) const fn msg_two_step_hint() -> &'static str {
    "Two-step workflow: run prerequisites first, create/merge those items in Wikidata, then click Generate QuickStatements again so the main block uses resolved QIDs directly."
}

pub(super) const fn button_second_pass() -> &'static str {
    "I created the missing items; let's finish"
}

pub(super) const fn msg_second_pass_running() -> &'static str {
    "Running a second pass on rows that depend on missing items..."
}

pub(super) const fn msg_second_pass_done() -> &'static str {
    "Second pass complete. Main QuickStatements now use resolved QIDs where available."
}

pub(super) fn msg_second_pass_still_pending_count(count: usize) -> String {
    format!(
        "{count} prerequisite item(s) are still missing. Create or merge them, then retry in about 30-120 seconds."
    )
}

pub(super) const fn curation_badge_prerequisite_pending() -> &'static str {
    "Prerequisite pending"
}

pub(super) const fn curation_badge_mass_missing() -> &'static str {
    "Mass missing"
}

pub(super) const fn curation_badge_second_pass_required() -> &'static str {
    "Second pass required"
}

pub(super) const fn curation_mass_warning_title() -> &'static str {
    "Exact mass could not be resolved from the descriptor endpoints"
}

pub(super) const fn msg_delay_advice() -> &'static str {
    "Tip: Wikidata and query endpoints may take 30-120 seconds to expose newly created items."
}

pub(super) const fn curation_qs_dev_label() -> &'static str {
    "Open QS-Dev"
}

pub(super) const fn curation_qs_dev_prereq_hint() -> &'static str {
    "Open QS-Dev, paste the prerequisites block, run it, create or merge the new items in Wikidata, wait briefly, then return here for the second pass."
}

pub(super) const fn curation_qs_dev_main_hint() -> &'static str {
    "Open QS-Dev, paste the main block, review the commands, then execute them."
}

pub(super) const fn curation_note_existing_complete() -> &'static str {
    "Entry already exists on Wikidata and no missing fields were detected."
}

pub(super) const fn curation_note_existing_updates() -> &'static str {
    "Existing Wikidata entry found: generated update QuickStatements."
}

pub(super) const fn curation_note_new_compound() -> &'static str {
    "No Wikidata entry was found by InChIKey; creation QuickStatements were generated."
}

pub(super) const fn curation_note_dependencies_pending() -> &'static str {
    "Prerequisite entities are still unresolved."
}

pub(super) fn curation_pending_taxon(taxon: &str) -> String {
    format!("Taxon '{taxon}' has not been found in Wikidata yet.")
}

pub(super) fn curation_pending_reference(doi: &str) -> String {
    format!("Reference for DOI '{doi}' is not available yet.")
}

pub(super) const fn view_switch_aria() -> &'static str {
    "Choose a section"
}

pub(super) const fn view_label_explorer() -> &'static str {
    "Search"
}

pub(super) const fn view_label_curation_explorer() -> &'static str {
    "Curation"
}

pub(super) const fn view_label_draw() -> &'static str {
    "Structure editor"
}

pub(super) fn curation_status_label(status_key: &str) -> &'static str {
    match status_key {
        "existing_complete" => "already complete",
        "existing_updates" => "existing item, updates generated",
        "new_compound" => "new item, creation generated",
        "pending_dependencies" => "waiting for prerequisite entities",
        "not_checked" => "not looked up",
        "error" => "error",
        _ => "status",
    }
}
