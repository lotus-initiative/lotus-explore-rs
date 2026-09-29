// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub(super) const fn heading_add_one_row() -> &'static str {
    "Eine Zeile hinzufügen"
}

pub(super) const fn heading_tsv_import() -> &'static str {
    "TSV-Import"
}

pub(super) const fn heading_queued_rows() -> &'static str {
    "Wartende Zeilen"
}

pub(super) const fn heading_results() -> &'static str {
    "Curation-Ergebnisse"
}

pub(super) const fn heading_quickstatements() -> &'static str {
    "QuickStatements (kopieren/einfügen)"
}

pub(super) const fn heading_quickstatements_dependencies() -> &'static str {
    "QuickStatements — Voraussetzungen"
}

pub(super) const fn placeholder_molecule_name() -> &'static str {
    "Molekülname"
}

pub(super) const fn placeholder_taxon_optional() -> &'static str {
    "Taxon (optional)"
}

pub(super) const fn placeholder_doi_optional() -> &'static str {
    "DOI (optional)"
}

pub(super) const fn button_add_row() -> &'static str {
    "Zeile hinzufügen"
}

pub(super) const fn button_load_example_rows() -> &'static str {
    "Beispielzeilen laden"
}

pub(super) const fn button_append_tsv_rows() -> &'static str {
    "TSV-Zeilen anhängen"
}

pub(super) const fn button_generate_quickstatements() -> &'static str {
    "QuickStatements erzeugen"
}

pub(super) const fn button_generating() -> &'static str {
    "Wird erzeugt..."
}

pub(super) const fn button_remove() -> &'static str {
    "Entfernen"
}

pub(super) const fn col_name() -> &'static str {
    "Name"
}

pub(super) const fn col_action() -> &'static str {
    "Aktion"
}

pub(super) const fn col_original_smiles() -> &'static str {
    "Original-SMILES"
}

pub(super) const fn col_canonical_smiles() -> &'static str {
    "Kanonisches SMILES"
}

pub(super) const fn col_exact_mass() -> &'static str {
    "Exakte Masse"
}

pub(super) const fn col_status() -> &'static str {
    "Status"
}

pub(super) const fn label_new_item() -> &'static str {
    "neuer Eintrag"
}

pub(super) const fn hint_expected_tsv_headers() -> &'static str {
    "Erwartete Header: name, smiles, taxon (oder organism), doi"
}

pub(super) const fn hint_scroll_curation_results() -> &'static str {
    "Tipp: Wischen Sie horizontal, um alle Ergebnisspalten zu sehen."
}

pub(super) fn msg_name_smiles_required() -> String {
    "Name und SMILES sind erforderlich, um eine Zeile hinzuzufügen.".to_string()
}

pub(super) fn msg_duplicate_row_skipped() -> String {
    "Doppelte Zeile übersprungen (gleiche Struktur/Taxon/Referenz).".to_string()
}

pub(super) fn msg_no_valid_tsv_rows() -> String {
    "Keine gültigen Zeilen in der TSV-Eingabe gefunden.".to_string()
}

pub(super) fn msg_tsv_missing_column(column: &str) -> String {
    format!("Die erforderliche Spalte '{column}' fehlt in der TSV-Eingabe.")
}

pub(super) fn msg_tsv_import_complete(added: usize, skipped: usize) -> String {
    format!(
        "TSV-Import abgeschlossen: {added} eindeutige Zeile(n) hinzugefügt, {skipped} Duplikat(e) übersprungen."
    )
}

pub(super) fn msg_examples_loaded(added: usize, skipped: usize) -> String {
    format!(
        "Beispiele geladen: {added} eindeutige Zeile(n) hinzugefügt, {skipped} Duplikat(e) übersprungen."
    )
}

pub(super) fn msg_add_row_before_generate() -> String {
    "Fügen Sie mindestens eine Zeile hinzu, bevor Sie QuickStatements erzeugen.".to_string()
}

pub(super) fn msg_running_checks() -> String {
    "Curation-Prüfungen mit RDKit.js und Wikidata werden ausgeführt...".to_string()
}

pub(super) fn msg_done_review_copy() -> String {
    "Fertig. Prüfen Sie die erzeugten Zeilen und kopieren Sie den QuickStatements-Block."
        .to_string()
}

pub(super) fn msg_curation_failed(detail: &str) -> String {
    format!("Curation fehlgeschlagen: {detail}")
}

pub(super) const fn msg_curation_rate_limited() -> &'static str {
    "Ratenlimit bei einem vorgelagerten Metadatendienst erreicht (HTTP 429). Warten Sie etwa 60 Sekunden und versuchen Sie es erneut."
}

pub(super) fn msg_prerequisites_pending(count: usize) -> String {
    format!(
        "{count} Zeile(n) warten noch auf vorausgesetzte Entitäten. Führen Sie die Voraussetzungen aus, erstellen/vereinigen Sie sie in Wikidata und starten Sie dann den zweiten Durchlauf."
    )
}

pub(super) const fn msg_two_step_hint() -> &'static str {
    "Zweistufiger Ablauf: Führen Sie zuerst die Voraussetzungen aus, erstellen/vereinigen Sie diese Einträge in Wikidata und klicken Sie dann erneut auf QuickStatements erzeugen, damit der Hauptblock direkt aufgelöste QIDs verwendet."
}

pub(super) const fn button_second_pass() -> &'static str {
    "Ich habe die fehlenden Einträge erstellt; jetzt den Rest"
}

pub(super) const fn msg_second_pass_running() -> &'static str {
    "Zweiter Durchlauf für Zeilen mit fehlenden Abhängigkeiten wird ausgeführt..."
}

pub(super) const fn msg_second_pass_done() -> &'static str {
    "Zweiter Durchlauf abgeschlossen. Haupt-QuickStatements verwenden jetzt aufgelöste QIDs, sofern verfügbar."
}

pub(super) fn msg_second_pass_still_pending_count(count: usize) -> String {
    format!(
        "{count} vorausgesetzte Einträge fehlen weiterhin. Erstellen oder vereinigen Sie sie und versuchen Sie es in etwa 30-120 Sekunden erneut."
    )
}

pub(super) const fn curation_badge_prerequisite_pending() -> &'static str {
    "Voraussetzung ausstehend"
}

pub(super) const fn curation_badge_mass_missing() -> &'static str {
    "Masse fehlt"
}

pub(super) const fn curation_badge_second_pass_required() -> &'static str {
    "Zweiter Durchlauf erforderlich"
}

pub(super) const fn curation_mass_warning_title() -> &'static str {
    "Die exakte Masse konnte von den Endpunkten für Deskriptoren nicht ermittelt werden"
}

pub(super) const fn msg_delay_advice() -> &'static str {
    "Hinweis: Wikidata und Abfrage-Endpunkte benötigen oft 30-120 Sekunden, bis neu erstellte Einträge sichtbar sind."
}

pub(super) const fn curation_qs_dev_label() -> &'static str {
    "QS-Dev öffnen"
}

pub(super) const fn curation_qs_dev_prereq_hint() -> &'static str {
    "Öffnen Sie QS-Dev, fügen Sie den Voraussetzungenblock ein, führen Sie ihn aus, erstellen oder vereinigen Sie die neuen Einträge in Wikidata, warten Sie kurz und kehren Sie dann für den zweiten Durchlauf hierher zurück."
}

pub(super) const fn curation_qs_dev_main_hint() -> &'static str {
    "Öffnen Sie QS-Dev, fügen Sie den Hauptblock ein, prüfen Sie die Befehle und führen Sie sie dann aus."
}

pub(super) const fn curation_note_existing_complete() -> &'static str {
    "Der Eintrag existiert bereits in Wikidata und es wurden keine fehlenden Felder erkannt."
}

pub(super) const fn curation_note_existing_updates() -> &'static str {
    "Vorhandener Wikidata-Eintrag gefunden: Update-QuickStatements wurden erstellt."
}

pub(super) const fn curation_note_new_compound() -> &'static str {
    "Kein Wikidata-Eintrag über InChIKey gefunden; Erstellungs-QuickStatements wurden erzeugt."
}

pub(super) const fn curation_note_dependencies_pending() -> &'static str {
    "Vorausgesetzte Entitäten sind noch nicht aufgelöst."
}

pub(super) fn curation_pending_taxon(taxon: &str) -> String {
    format!("Taxon '{taxon}' wurde in Wikidata noch nicht gefunden.")
}

pub(super) fn curation_pending_reference(doi: &str) -> String {
    format!("Die Referenz für DOI '{doi}' ist noch nicht verfügbar.")
}

pub(super) const fn view_switch_aria() -> &'static str {
    "Bereichsauswahl"
}

pub(super) const fn view_label_explorer() -> &'static str {
    "Suche"
}

pub(super) const fn view_label_curation_explorer() -> &'static str {
    "Kuratierung"
}

pub(super) const fn view_label_draw() -> &'static str {
    "Struktureditor"
}

pub(super) fn curation_status_label(status_key: &str) -> &'static str {
    match status_key {
        "existing_complete" => "bereits vollständig",
        "existing_updates" => "vorhandener Eintrag, Updates erzeugt",
        "new_compound" => "neuer Eintrag, Erstellung erzeugt",
        "pending_dependencies" => "wartet auf vorausgesetzte Entitäten",
        "not_checked" => "nicht geprüft",
        "error" => "fehler",
        _ => "Status",
    }
}
