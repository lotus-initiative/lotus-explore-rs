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

pub fn err_compound_not_found(input: &str) -> String {
    format!("Composé « {input} » introuvable dans Wikidata.")
}

pub fn err_taxon_not_found(taxon: &str) -> String {
    format!("Taxon '{taxon}' introuvable dans Wikidata.")
}

/// Une référence introuvable dans Wikidata.
pub fn err_reference_not_found(input: &str) -> String {
    format!("Référence '{input}' introuvable dans Wikidata.")
}

/// Une référence qui n'est ni un QID ni un DOI.
pub fn err_reference_not_an_identifier(input: &str) -> String {
    format!(
        "Une référence doit être un QID Wikidata ou un DOI ; '{input}' n'est ni l'un ni l'autre."
    )
}

pub fn warn_input_standardized(original: &str, normalized: &str) -> String {
    format!("Entrée standardisée de '{original}' à '{normalized}'.")
}

/// The common-name notice. Says which property matched, because that is
/// the actionable part: P1843 rather than P225.
pub fn warn_taxon_common_name(name: &str, qid: &str) -> String {
    format!(
        "La recherche par nom commun est déconseillée : « {name} » a correspondu au taxon {qid} par son nom commun (P1843) plutôt que par son nom scientifique (P225)."
    )
}

/// The structure-resolved notice. Says which compound the name became,
/// because that is the actionable part: the search now runs against a
/// different structure than the one that was typed.
pub fn warn_compound_resolved(label: &str, qid: &str) -> String {
    format!("« {label} » a été résolu en composé {qid}, par InChIKey, libellé ou alias.")
}

pub fn warn_ambiguous_taxon(best_name: &str, best_qid: &str, names: &str) -> String {
    format!("Nom de taxon ambigu ; utilisation de {best_name} ({best_qid}). Candidats : {names}")
}

/// More than one compound matched, and the one that was used is named.
///
/// Split from `warn_ambiguous_taxon` rather than shared: the reader's next move is
/// the same, but naming a compound "an ambiguous taxon name" points them at the
/// field they did not type into.
pub fn warn_ambiguous_compound(best_name: &str, best_qid: &str, names: &str) -> String {
    format!("Composé ambigu ; utilisation de {best_name} ({best_qid}). Candidats : {names}")
}

/// La recherche ne nomme ni structure, ni taxon, ni référence : tout LOTUS est
/// parcouru. Dit comme un fait sur le parcours et non sur la réponse, car les
/// filtres posés en plus restreignent la réponse sans restreindre le parcours.
pub fn warn_wdqs_fallback() -> String {
    "Requête exécutée via Wikidata Query Service (repli depuis QLever).".to_string()
}

#[cfg(target_arch = "wasm32")]
pub fn error_hint_memory() -> &'static str {
    "Résultat trop volumineux pour la mémoire de l'appareil."
}
