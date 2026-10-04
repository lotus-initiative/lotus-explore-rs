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

pub fn err_compound_not_found(input: &str) -> String {
    format!("Composto «{input}» non trovato in Wikidata.")
}

pub fn err_taxon_not_found(taxon: &str) -> String {
    format!("Taxon '{taxon}' non trovato in Wikidata.")
}

/// Un riferimento che Wikidata non conosce.
pub fn err_reference_not_found(input: &str) -> String {
    format!("Riferimento '{input}' non trovato in Wikidata.")
}

/// Un riferimento che non è né un QID né un DOI.
pub fn err_reference_not_an_identifier(input: &str) -> String {
    format!(
        "Un riferimento deve essere un QID di Wikidata o un DOI; '{input}' non è né l'uno né l'altro."
    )
}

pub fn warn_input_standardized(original: &str, normalized: &str) -> String {
    format!("Input standardizzato da '{original}' a '{normalized}'.")
}

/// The common-name notice. Says which property matched, because that is
/// the actionable part: P1843 rather than P225.
pub fn warn_taxon_common_name(name: &str, qid: &str) -> String {
    format!(
        "La ricerca per nome comune è sconsigliata: «{name}» è stato abbinato al taxon {qid} tramite il nome comune (P1843) anziché quello scientifico (P225)."
    )
}

/// The structure-resolved notice. Says which compound the name became,
/// because that is the actionable part: the search now runs against a
/// different structure than the one that was typed.
pub fn warn_compound_resolved(label: &str, qid: &str) -> String {
    format!("«{label}» è stato risolto nel composto {qid}, tramite InChIKey, etichetta o alias.")
}

pub fn warn_ambiguous_taxon(best_name: &str, best_qid: &str, names: &str) -> String {
    format!("Nome taxon ambiguo; uso {best_name} ({best_qid}). Candidati: {names}")
}

/// More than one compound matched, and the one that was used is named.
///
/// Split from `warn_ambiguous_taxon` rather than shared: the reader's next move is
/// the same, but naming a compound "an ambiguous taxon name" points them at the
/// field they did not type into.
pub fn warn_ambiguous_compound(best_name: &str, best_qid: &str, names: &str) -> String {
    format!("Composto ambiguo; uso {best_name} ({best_qid}). Candidati: {names}")
}

/// La ricerca non nomina né struttura né taxon né riferimento, quindi scandisce
/// tutto LOTUS. Detto come fatto sulla scansione e non sulla risposta, perché i
/// filtri impostati restringono la risposta senza restringere la scansione.
pub fn warn_wdqs_fallback() -> String {
    "Query eseguita tramite Wikidata Query Service (fallback da QLever).".to_string()
}

#[cfg(target_arch = "wasm32")]
pub fn error_hint_memory() -> &'static str {
    "Risultato troppo grande per la memoria disponibile sul dispositivo."
}
