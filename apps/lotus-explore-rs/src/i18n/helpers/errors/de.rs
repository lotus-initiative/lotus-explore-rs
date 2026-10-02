// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub fn err_invalid_search_input() -> String {
    "Bitte geben Sie einen Taxonnamen / eine QID oder eine SMILES-Struktur ein.".to_string()
}

pub fn err_api_not_configured() -> String {
    "Die LOTUS-API ist nicht konfiguriert.".to_string()
}

pub fn err_taxon_too_long() -> String {
    "Die Taxon-Eingabe ist zu lang. Bitte unter 500 Zeichen bleiben.".to_string()
}

pub fn err_structure_too_long() -> String {
    "Die Struktur-Eingabe ist zu lang. Bitte den SMILES/Molfile-Text kürzen.".to_string()
}

pub fn err_mass_out_of_range() -> String {
    "Massenwerte müssen zwischen 0 und 10000 liegen.".to_string()
}

pub fn err_mass_range_invalid() -> String {
    "Die minimale Masse darf die maximale Masse nicht überschreiten.".to_string()
}

pub fn err_year_out_of_range() -> String {
    "Das Jahr liegt außerhalb des unterstützten Bereichs.".to_string()
}

pub fn err_year_range_invalid() -> String {
    "Das Startjahr darf nicht größer als das Endjahr sein.".to_string()
}

pub fn err_element_count_too_high() -> String {
    "Die Elementanzahl in der Formel ist zu hoch.".to_string()
}

pub fn err_similarity_threshold_invalid() -> String {
    "Der Ähnlichkeitsschwellenwert muss größer als 0 sein.".to_string()
}

pub fn err_unsupported_format(fmt: &str) -> String {
    format!("Nicht unterstütztes Format '{fmt}'. Verwenden Sie csv, json oder rdf.")
}

pub fn err_taxon_parse_failed(detail: &str) -> String {
    format!("Taxon-Parsing fehlgeschlagen: {detail}")
}

pub fn err_query_stage_failed(stage: &str, detail: &str) -> String {
    format!("Schritt {stage} fehlgeschlagen: {detail}")
}

pub fn err_taxon_not_found(taxon: &str) -> String {
    format!("Taxon '{taxon}' wurde in Wikidata nicht gefunden.")
}

pub fn warn_input_standardized(original: &str, normalized: &str) -> String {
    format!("Eingabe von '{original}' zu '{normalized}' standardisiert.")
}

/// The common-name notice. Says which property matched, because that is
/// the actionable part: P1843 rather than P225.
pub fn warn_taxon_common_name(name: &str, qid: &str) -> String {
    format!(
        "Die Suche nach einem Trivialnamen wird nicht empfohlen: „{name}“ wurde Taxon {qid} über den Trivialnamen (P1843) zugeordnet, nicht über den wissenschaftlichen Namen (P225)."
    )
}

pub fn warn_ambiguous_taxon(best_name: &str, best_qid: &str, names: &str) -> String {
    format!("Mehrdeutiger Taxonname; verwende {best_name} ({best_qid}). Kandidaten: {names}")
}

pub fn warn_wdqs_fallback() -> String {
    "Abfrage über Wikidata Query Service ausgeführt (Fallback von QLever).".to_string()
}

#[cfg(target_arch = "wasm32")]
pub fn error_hint_memory() -> &'static str {
    "Ergebnis ist zu groß für den verfügbaren Gerätspeicher."
}
