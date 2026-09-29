// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub fn err_invalid_search_input() -> String {
    "Veuillez saisir un nom de taxon / QID, ou une structure SMILES.".to_string()
}

pub fn err_api_not_configured() -> String {
    "L'API LOTUS n'est pas configurée.".to_string()
}

pub fn err_taxon_too_long() -> String {
    "La valeur du taxon est trop longue. Veuillez rester sous 500 caractères.".to_string()
}

pub fn err_structure_too_long() -> String {
    "La structure est trop longue. Raccourcissez le texte SMILES/Molfile.".to_string()
}

pub fn err_mass_out_of_range() -> String {
    "Les valeurs de masse doivent être comprises entre 0 et 10000.".to_string()
}

pub fn err_mass_range_invalid() -> String {
    "La masse minimale ne peut pas dépasser la masse maximale.".to_string()
}

pub fn err_year_out_of_range() -> String {
    "L'année est hors de la plage prise en charge.".to_string()
}

pub fn err_year_range_invalid() -> String {
    "L'année de début ne peut pas dépasser l'année de fin.".to_string()
}

pub fn err_element_count_too_high() -> String {
    "Les comptages d'éléments de la formule sont trop élevés.".to_string()
}

pub fn err_similarity_threshold_invalid() -> String {
    "Le seuil de similarité doit être supérieur à 0.".to_string()
}

pub fn err_unsupported_format(fmt: &str) -> String {
    format!("Format '{fmt}' non pris en charge. Utilisez csv, json ou rdf.")
}

pub fn err_taxon_parse_failed(detail: &str) -> String {
    format!("Échec de l'analyse du taxon : {detail}")
}

pub fn err_query_stage_failed(stage: &str, detail: &str) -> String {
    format!("Échec de l'étape {stage} : {detail}")
}

pub fn err_taxon_not_found(taxon: &str) -> String {
    format!("Taxon '{taxon}' introuvable dans Wikidata.")
}

pub fn warn_input_standardized(original: &str, normalized: &str) -> String {
    format!("Entrée standardisée de '{original}' à '{normalized}'.")
}

pub fn warn_ambiguous_taxon(best_name: &str, best_qid: &str, names: &str) -> String {
    format!("Nom de taxon ambigu; utilisation de {best_name} ({best_qid}). Candidats : {names}")
}

pub fn warn_wdqs_fallback() -> String {
    "Requête exécutée via Wikidata Query Service (repli depuis QLever).".to_string()
}

#[cfg(target_arch = "wasm32")]
pub fn error_hint_memory() -> &'static str {
    "Résultat trop volumineux pour la mémoire de l'appareil."
}
