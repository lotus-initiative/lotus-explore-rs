// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub(super) const fn heading_add_one_row() -> &'static str {
    "Aggiungi una riga"
}

pub(super) const fn heading_tsv_import() -> &'static str {
    "Import TSV"
}

pub(super) const fn heading_queued_rows() -> &'static str {
    "Righe in coda"
}

pub(super) const fn heading_results() -> &'static str {
    "Risultati della curazione"
}

pub(super) const fn heading_quickstatements() -> &'static str {
    "QuickStatements (copia-incolla)"
}

pub(super) const fn heading_quickstatements_dependencies() -> &'static str {
    "QuickStatements — prerequisiti"
}

pub(super) const fn placeholder_molecule_name() -> &'static str {
    "Nome molecola"
}

pub(super) const fn placeholder_taxon_optional() -> &'static str {
    "Taxon (opzionale)"
}

pub(super) const fn placeholder_doi_optional() -> &'static str {
    "DOI (opzionale)"
}

pub(super) const fn button_add_row() -> &'static str {
    "Aggiungi"
}

pub(super) const fn button_load_example_rows() -> &'static str {
    "Carica esempi"
}

pub(super) const fn button_append_tsv_rows() -> &'static str {
    "Aggiungi righe TSV"
}

pub(super) const fn button_generate_quickstatements() -> &'static str {
    "Genera QuickStatements"
}

pub(super) const fn button_generating() -> &'static str {
    "Generazione in corso..."
}

pub(super) const fn button_remove() -> &'static str {
    "Rimuovi"
}

pub(super) const fn col_name() -> &'static str {
    "Nome"
}

pub(super) const fn col_action() -> &'static str {
    "Azione"
}

pub(super) const fn col_original_smiles() -> &'static str {
    "SMILES originale"
}

pub(super) const fn col_canonical_smiles() -> &'static str {
    "SMILES canonico"
}

pub(super) const fn col_exact_mass() -> &'static str {
    "Massa esatta"
}

pub(super) const fn col_status() -> &'static str {
    "Stato"
}

pub(super) const fn label_new_item() -> &'static str {
    "nuova voce"
}

pub(super) const fn hint_expected_tsv_headers() -> &'static str {
    "Intestazioni attese: name, smiles, taxon (o organism), doi"
}

pub(super) const fn hint_scroll_curation_results() -> &'static str {
    "Suggerimento: scorri orizzontalmente per vedere tutte le colonne dei risultati."
}

pub(super) fn msg_name_smiles_required() -> String {
    "Nome e SMILES sono obbligatori per aggiungere una riga.".to_string()
}

pub(super) fn msg_duplicate_row_skipped() -> String {
    "Riga duplicata ignorata (stessa struttura/taxon/riferimento).".to_string()
}

pub(super) fn msg_no_valid_tsv_rows() -> String {
    "Nessuna riga valida trovata nell'input TSV.".to_string()
}

pub(super) fn msg_tsv_missing_column(column: &str) -> String {
    format!("Nel file TSV manca la colonna obbligatoria '{column}'.")
}

pub(super) fn msg_tsv_import_complete(added: usize, skipped: usize) -> String {
    format!("Import TSV completato: aggiunte {added} riga(e) uniche, saltati {skipped} duplicati.")
}

pub(super) fn msg_examples_loaded(added: usize, skipped: usize) -> String {
    format!("Esempi caricati: aggiunte {added} riga(e) uniche, saltati {skipped} duplicati.")
}

pub(super) fn msg_add_row_before_generate() -> String {
    "Aggiungi almeno una riga prima di generare i QuickStatements.".to_string()
}

pub(super) fn msg_running_checks() -> String {
    "Esecuzione dei controlli di curazione con RDKit.js e Wikidata...".to_string()
}

pub(super) fn msg_done_review_copy() -> String {
    "Fatto. Controlla le righe generate e copia il blocco QuickStatements.".to_string()
}

pub(super) fn msg_curation_failed(detail: &str) -> String {
    format!("Curation non riuscita: {detail}")
}

pub(super) const fn msg_curation_rate_limited() -> &'static str {
    "Limite di richieste raggiunto su un servizio di metadati upstream (HTTP 429). Attendi circa 60 secondi e riprova."
}

pub(super) fn msg_prerequisites_pending(count: usize) -> String {
    format!(
        "{count} riga(e) sono ancora in attesa di entità prerequisito. Esegui i prerequisiti, crea/unisci in Wikidata, poi avvia il secondo passaggio."
    )
}

pub(super) const fn msg_two_step_hint() -> &'static str {
    "Flusso in due fasi: esegui prima i prerequisiti, crea/unisci quegli elementi in Wikidata, poi fai di nuovo clic su Genera QuickStatements così il blocco principale usa direttamente i QID risolti."
}

pub(super) const fn button_second_pass() -> &'static str {
    "Ho creato gli elementi mancanti; completiamo il lavoro"
}

pub(super) const fn msg_second_pass_running() -> &'static str {
    "Esecuzione del secondo passaggio sulle righe che dipendono da elementi mancanti..."
}

pub(super) const fn msg_second_pass_done() -> &'static str {
    "Secondo passaggio completato. I QuickStatements principali usano ora i QID risolti quando disponibili."
}

pub(super) fn msg_second_pass_still_pending_count(count: usize) -> String {
    format!(
        "{count} elementi prerequisito non sono ancora stati trovati. Creali o uniscili e riprova tra circa 30-120 secondi."
    )
}

pub(super) const fn curation_badge_prerequisite_pending() -> &'static str {
    "Prerequisito in attesa"
}

pub(super) const fn curation_badge_mass_missing() -> &'static str {
    "Massa mancante"
}

pub(super) const fn curation_badge_second_pass_required() -> &'static str {
    "Secondo passaggio richiesto"
}

pub(super) const fn curation_mass_warning_title() -> &'static str {
    "La massa esatta non è stata determinata dagli endpoint dei descrittori"
}

pub(super) const fn msg_delay_advice() -> &'static str {
    "Suggerimento: Wikidata e gli endpoint di query possono richiedere 30-120 secondi per rendere visibili i nuovi elementi."
}

pub(super) const fn curation_qs_dev_label() -> &'static str {
    "Apri QS-Dev"
}

pub(super) const fn curation_qs_dev_prereq_hint() -> &'static str {
    "Apri QS-Dev, incolla il blocco dei prerequisiti, eseguilo, crea o unisci i nuovi elementi in Wikidata, attendi un momento e poi torna qui per il secondo passaggio."
}

pub(super) const fn curation_qs_dev_main_hint() -> &'static str {
    "Apri QS-Dev, incolla il blocco principale, controlla i comandi e poi eseguili."
}

pub(super) const fn curation_note_existing_complete() -> &'static str {
    "La voce esiste già su Wikidata e non sono stati rilevati campi mancanti."
}

pub(super) const fn curation_note_existing_updates() -> &'static str {
    "Trovata una voce Wikidata esistente: generati QuickStatements di aggiornamento."
}

pub(super) const fn curation_note_new_compound() -> &'static str {
    "Nessuna voce Wikidata trovata tramite InChIKey; generati QuickStatements di creazione."
}

pub(super) const fn curation_note_dependencies_pending() -> &'static str {
    "Le entità prerequisito non sono ancora risolte."
}

pub(super) fn curation_pending_taxon(taxon: &str) -> String {
    format!("Il taxon '{taxon}' non è ancora stato trovato in Wikidata.")
}

pub(super) fn curation_pending_reference(doi: &str) -> String {
    format!("Il riferimento per il DOI '{doi}' non è ancora disponibile.")
}

pub(super) const fn view_switch_aria() -> &'static str {
    "Selettore di sezione"
}

pub(super) const fn view_label_explorer() -> &'static str {
    "Ricerca"
}

pub(super) const fn view_label_curation_explorer() -> &'static str {
    "Curazione"
}

pub(super) const fn view_label_draw() -> &'static str {
    "Editor di struttura"
}

pub(super) fn curation_status_label(status_key: &str) -> &'static str {
    match status_key {
        "existing_complete" => "già completo",
        "existing_updates" => "voce esistente, aggiornamenti generati",
        "new_compound" => "nuova voce, creazione generata",
        "pending_dependencies" => "in attesa delle entità prerequisito",
        "not_checked" => "non verificato",
        "error" => "errore",
        _ => "stato",
    }
}
