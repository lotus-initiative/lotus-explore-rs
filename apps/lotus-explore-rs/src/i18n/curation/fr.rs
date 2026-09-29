// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub(super) const fn heading_add_one_row() -> &'static str {
    "Ajouter une ligne"
}

pub(super) const fn heading_tsv_import() -> &'static str {
    "Import TSV"
}

pub(super) const fn heading_queued_rows() -> &'static str {
    "Lignes en file"
}

pub(super) const fn heading_results() -> &'static str {
    "Résultats de curation"
}

pub(super) const fn heading_quickstatements() -> &'static str {
    "QuickStatements (copier-coller)"
}

pub(super) const fn heading_quickstatements_dependencies() -> &'static str {
    "QuickStatements — prérequis"
}

pub(super) const fn placeholder_molecule_name() -> &'static str {
    "Nom de molécule"
}

pub(super) const fn placeholder_taxon_optional() -> &'static str {
    "Taxon (optionnel)"
}

pub(super) const fn placeholder_doi_optional() -> &'static str {
    "DOI (optionnel)"
}

pub(super) const fn button_add_row() -> &'static str {
    "Ajouter"
}

pub(super) const fn button_load_example_rows() -> &'static str {
    "Charger des exemples"
}

pub(super) const fn button_append_tsv_rows() -> &'static str {
    "Ajouter les lignes TSV"
}

pub(super) const fn button_generate_quickstatements() -> &'static str {
    "Générer les QuickStatements"
}

pub(super) const fn button_generating() -> &'static str {
    "Génération..."
}

pub(super) const fn button_remove() -> &'static str {
    "Retirer"
}

pub(super) const fn col_name() -> &'static str {
    "Nom"
}

pub(super) const fn col_action() -> &'static str {
    "Action"
}

pub(super) const fn col_original_smiles() -> &'static str {
    "SMILES original"
}

pub(super) const fn col_canonical_smiles() -> &'static str {
    "SMILES canonique"
}

pub(super) const fn col_exact_mass() -> &'static str {
    "Masse exacte"
}

pub(super) const fn col_status() -> &'static str {
    "Statut"
}

pub(super) const fn label_new_item() -> &'static str {
    "nouvel élément"
}

pub(super) const fn hint_expected_tsv_headers() -> &'static str {
    "En-têtes attendus : name, smiles, taxon (ou organism), doi"
}

pub(super) const fn hint_scroll_curation_results() -> &'static str {
    "Astuce : faites glisser horizontalement pour voir toutes les colonnes."
}

pub(super) fn msg_name_smiles_required() -> String {
    "Le nom et le SMILES sont requis pour ajouter une ligne.".to_string()
}

pub(super) fn msg_duplicate_row_skipped() -> String {
    "Ligne en double ignorée (même structure/taxon/référence).".to_string()
}

pub(super) fn msg_no_valid_tsv_rows() -> String {
    "Aucune ligne valide trouvée dans l'entrée TSV.".to_string()
}

pub(super) fn msg_tsv_missing_column(column: &str) -> String {
    format!("La colonne obligatoire '{column}' est absente de l'entrée TSV.")
}

pub(super) fn msg_tsv_import_complete(added: usize, skipped: usize) -> String {
    format!(
        "Import TSV terminé : {added} ligne(s) unique(s) ajoutée(s), {skipped} doublon(s) ignoré(s)."
    )
}

pub(super) fn msg_examples_loaded(added: usize, skipped: usize) -> String {
    format!(
        "Exemples chargés : {added} ligne(s) unique(s) ajoutée(s), {skipped} doublon(s) ignoré(s)."
    )
}

pub(super) fn msg_add_row_before_generate() -> String {
    "Ajoutez au moins une ligne avant de générer les QuickStatements.".to_string()
}

pub(super) fn msg_running_checks() -> String {
    "Exécution des vérifications de curation avec RDKit.js et Wikidata...".to_string()
}

pub(super) fn msg_done_review_copy() -> String {
    "Terminé. Vérifiez les lignes générées et copiez le bloc QuickStatements.".to_string()
}

pub(super) fn msg_curation_failed(detail: &str) -> String {
    format!("Échec de la curation : {detail}")
}

pub(super) const fn msg_curation_rate_limited() -> &'static str {
    "Limite de débit atteinte sur un service de métadonnées amont (HTTP 429). Attendez environ 60 s puis réessayez."
}

pub(super) fn msg_prerequisites_pending(count: usize) -> String {
    format!(
        "{count} ligne(s) attend(ent) encore des entités préalables. Exécutez les prérequis, créez-les ou fusionnez-les dans Wikidata, puis lancez la seconde passe."
    )
}

pub(super) const fn msg_two_step_hint() -> &'static str {
    "Flux en deux étapes : exécutez d'abord les prérequis, créez/fusionnez ces éléments dans Wikidata, puis cliquez à nouveau sur Générer les QuickStatements pour que le bloc principal utilise directement les QID résolus."
}

pub(super) const fn button_second_pass() -> &'static str {
    "J'ai créé les éléments manquants ; terminons"
}

pub(super) const fn msg_second_pass_running() -> &'static str {
    "Exécution d'une seconde passe sur les lignes qui dépendent d'éléments manquants..."
}

pub(super) const fn msg_second_pass_done() -> &'static str {
    "Seconde passe terminée. Les QuickStatements principaux utilisent désormais les QID résolus lorsque disponibles."
}

pub(super) fn msg_second_pass_still_pending_count(count: usize) -> String {
    format!(
        "{count} élément(s) prérequis sont toujours introuvables. Créez-les ou fusionnez-les, puis réessayez dans environ 30 à 120 secondes."
    )
}

pub(super) const fn curation_badge_prerequisite_pending() -> &'static str {
    "Prérequis en attente"
}

pub(super) const fn curation_badge_mass_missing() -> &'static str {
    "Masse manquante"
}

pub(super) const fn curation_badge_second_pass_required() -> &'static str {
    "Seconde passe requise"
}

pub(super) const fn curation_mass_warning_title() -> &'static str {
    "La masse exacte n'a pas pu être déterminée à partir des points de terminaison des descripteurs"
}

pub(super) const fn msg_delay_advice() -> &'static str {
    "Conseil : Wikidata et les points d'accès de requête peuvent mettre 30 à 120 secondes à exposer les nouveaux éléments."
}

pub(super) const fn curation_qs_dev_label() -> &'static str {
    "Ouvrir QS-Dev"
}

pub(super) const fn curation_qs_dev_prereq_hint() -> &'static str {
    "Ouvrez QS-Dev, collez le bloc de prérequis, exécutez-le, créez ou fusionnez les nouveaux éléments dans Wikidata, attendez un instant, puis revenez ici pour la seconde passe."
}

pub(super) const fn curation_qs_dev_main_hint() -> &'static str {
    "Ouvrez QS-Dev, collez le bloc principal, vérifiez les commandes, puis exécutez-les."
}

pub(super) const fn curation_note_existing_complete() -> &'static str {
    "L'entrée existe déjà dans Wikidata et aucun champ manquant n'a été détecté."
}

pub(super) const fn curation_note_existing_updates() -> &'static str {
    "Entrée Wikidata existante trouvée : QuickStatements de mise à jour générés."
}

pub(super) const fn curation_note_new_compound() -> &'static str {
    "Aucune entrée Wikidata n'a été trouvée via InChIKey ; les QuickStatements de création ont été générés."
}

pub(super) const fn curation_note_dependencies_pending() -> &'static str {
    "Les entités préalables ne sont pas encore résolues."
}

pub(super) fn curation_pending_taxon(taxon: &str) -> String {
    format!("Le taxon '{taxon}' n'a pas encore été trouvé dans Wikidata.")
}

pub(super) fn curation_pending_reference(doi: &str) -> String {
    format!("La référence pour le DOI '{doi}' n'est pas encore disponible.")
}

pub(super) const fn view_switch_aria() -> &'static str {
    "Sélecteur de section"
}

pub(super) const fn view_label_explorer() -> &'static str {
    "Recherche"
}

pub(super) const fn view_label_curation_explorer() -> &'static str {
    "Curation"
}

pub(super) const fn view_label_draw() -> &'static str {
    "Éditeur de structure"
}

pub(super) fn curation_status_label(status_key: &str) -> &'static str {
    match status_key {
        "existing_complete" => "déjà complet",
        "existing_updates" => "élément existant, mises à jour générées",
        "new_compound" => "nouvel élément, création générée",
        "pending_dependencies" => "en attente des entités préalables",
        "error" => "erreur",
        _ => "statut",
    }
}
