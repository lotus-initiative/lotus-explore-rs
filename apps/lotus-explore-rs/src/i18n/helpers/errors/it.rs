// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub fn err_invalid_search_input() -> String {
    "Inserisci un nome di taxon / QID oppure una struttura SMILES.".to_string()
}

pub fn err_api_not_configured() -> String {
    "L'API LOTUS non è configurata.".to_string()
}

pub fn err_taxon_too_long() -> String {
    "Il valore del taxon è troppo lungo. Mantienilo sotto i 500 caratteri.".to_string()
}

pub fn err_structure_too_long() -> String {
    "L'input della struttura è troppo lungo. Riduci il testo SMILES/Molfile.".to_string()
}

pub fn err_mass_out_of_range() -> String {
    "I valori di massa devono essere compresi tra 0 e 10000.".to_string()
}

pub fn err_mass_range_invalid() -> String {
    "La massa minima non può superare la massa massima.".to_string()
}

pub fn err_year_out_of_range() -> String {
    "L'anno è fuori dall'intervallo supportato.".to_string()
}

pub fn err_year_range_invalid() -> String {
    "L'anno iniziale non può superare l'anno finale.".to_string()
}

pub fn err_element_count_too_high() -> String {
    "I conteggi degli elementi della formula sono troppo alti.".to_string()
}

pub fn err_similarity_threshold_invalid() -> String {
    "La soglia di similarità deve essere maggiore di 0.".to_string()
}

pub fn err_unsupported_format(fmt: &str) -> String {
    format!("Formato '{fmt}' non supportato. Usa csv, json o rdf.")
}

pub fn err_taxon_parse_failed(detail: &str) -> String {
    format!("Parsing del taxon non riuscito: {detail}")
}

pub fn err_query_stage_failed(stage: &str, detail: &str) -> String {
    format!("Fase {stage} non riuscita: {detail}")
}

pub fn err_taxon_not_found(taxon: &str) -> String {
    format!("Taxon '{taxon}' non trovato in Wikidata.")
}

pub fn warn_input_standardized(original: &str, normalized: &str) -> String {
    format!("Input standardizzato da '{original}' a '{normalized}'.")
}

pub fn warn_ambiguous_taxon(best_name: &str, best_qid: &str, names: &str) -> String {
    format!("Nome taxon ambiguo; uso {best_name} ({best_qid}). Candidati: {names}")
}

pub fn warn_wdqs_fallback() -> String {
    "Query eseguita tramite Wikidata Query Service (fallback da QLever).".to_string()
}

#[cfg(target_arch = "wasm32")]
pub fn error_hint_memory() -> &'static str {
    "Risultato troppo grande per la memoria disponibile sul dispositivo."
}
