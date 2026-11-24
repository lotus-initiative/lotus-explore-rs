// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::Locale;

pub fn aria_search_inchikey(locale: Locale, ik: &str) -> String {
    match locale {
        Locale::En => format!("Search Wikidata for InChIKey {ik}"),
        Locale::Fr => format!("Rechercher dans Wikidata la clé InChIKey {ik}"),
        Locale::De => format!("InChIKey {ik} in Wikidata suchen"),
        Locale::It => format!("Cerca InChIKey {ik} in Wikidata"),
    }
}

pub fn aria_chemical_structure(locale: Locale, compound_name: &str) -> String {
    match locale {
        Locale::En => format!("Chemical structure of {compound_name}"),
        Locale::Fr => format!("Structure chimique de {compound_name}"),
        Locale::De => format!("Chemische Struktur von {compound_name}"),
        Locale::It => format!("Struttura chimica di {compound_name}"),
    }
}

pub fn aria_wikidata_statement(locale: Locale, stmt: &str) -> String {
    match locale {
        Locale::En => format!("Wikidata statement {stmt}"),
        Locale::Fr => format!("Déclaration Wikidata {stmt}"),
        Locale::De => format!("Wikidata-Aussage {stmt}"),
        Locale::It => format!("Dichiarazione Wikidata {stmt}"),
    }
}

pub fn aria_sort_toggle(locale: Locale, column: &str, next_descending: bool) -> String {
    match locale {
        Locale::En => {
            let dir = if next_descending {
                "descending"
            } else {
                "ascending"
            };
            format!("Sort by {column}, {dir}")
        }
        Locale::Fr => {
            let dir = if next_descending {
                "décroissant"
            } else {
                "croissant"
            };
            format!("Trier par {column}, ordre {dir}")
        }
        Locale::De => {
            let dir = if next_descending {
                "absteigend"
            } else {
                "aufsteigend"
            };
            format!("Nach {column} sortieren, {dir}")
        }
        Locale::It => {
            let dir = if next_descending {
                "decrescente"
            } else {
                "crescente"
            };
            format!("Ordina per {column}, ordine {dir}")
        }
    }
}

pub fn graph_two_type_note(
    locale: Locale,
    hidden_label: &str,
    hidden_count: usize,
    count_str: &str,
    hidden_unit: &str,
) -> String {
    match (locale, hidden_count == 1) {
        (Locale::En, true) => format!(
            "Showing 2 types because all results share 1 {hidden_label}; click {hidden_label} above or switch Graph view to show all 3."
        ),
        (Locale::En, false) => format!(
            "Showing 2 types for clarity ({count_str} {hidden_unit} weight the links); click {hidden_label} above or switch Graph view to show all 3."
        ),
        (Locale::Fr, true) => format!(
            "2 types affichés car tous les résultats partagent 1 {hidden_label} ; cliquez sur {hidden_label} ou changez Vue du graphe pour afficher les 3."
        ),
        (Locale::Fr, false) => format!(
            "2 types affichés par clarté ({count_str} {hidden_unit} pondèrent les liens) ; cliquez sur {hidden_label} ou changez Vue du graphe pour afficher les 3."
        ),
        (Locale::De, true) => format!(
            "2 Typen angezeigt, da alle Ergebnisse 1 {hidden_label} teilen; klicken Sie auf {hidden_label} oder wechseln Sie Graph-Ansicht für alle 3."
        ),
        (Locale::De, false) => format!(
            "2 Typen zur Übersicht angezeigt ({count_str} {hidden_unit} gewichten die Links); klicken Sie auf {hidden_label} oder wechseln Sie Graph-Ansicht für alle 3."
        ),
        (Locale::It, true) => format!(
            "Mostrati 2 tipi perché tutti i risultati condividono 1 {hidden_label}; clicca {hidden_label} o cambia Vista grafo per mostrare tutti e 3."
        ),
        (Locale::It, false) => format!(
            "Mostrati 2 tipi per chiarezza ({count_str} {hidden_unit} pesano i collegamenti); clicca {hidden_label} o cambia Vista grafo per mostrare tutti e 3."
        ),
    }
}

pub fn aria_graph_kind_toggle(
    locale: Locale,
    kind_label: &str,
    count_str: &str,
    active: bool,
    can_toggle: bool,
) -> String {
    match (locale, active, can_toggle) {
        (Locale::En, true, true) => format!("{kind_label} {count_str}: hide from graph"),
        (Locale::En, true, false) => {
            format!("{kind_label} {count_str}: shown (at least 2 types required)")
        }
        (Locale::En, false, true) => format!("{kind_label} {count_str}: show in graph"),
        (Locale::En, false, false) => format!("{kind_label} {count_str}: none in results"),
        (Locale::Fr, true, true) => format!("{kind_label} {count_str} : masquer du graphe"),
        (Locale::Fr, true, false) => {
            format!("{kind_label} {count_str} : affiché (au moins 2 types requis)")
        }
        (Locale::Fr, false, true) => format!("{kind_label} {count_str} : afficher dans le graphe"),
        (Locale::Fr, false, false) => {
            format!("{kind_label} {count_str} : aucun dans les résultats")
        }
        (Locale::De, true, true) => format!("{kind_label} {count_str}: im Graph ausblenden"),
        (Locale::De, true, false) => {
            format!("{kind_label} {count_str}: angezeigt (mindestens 2 Typen erforderlich)")
        }
        (Locale::De, false, true) => format!("{kind_label} {count_str}: im Graph einblenden"),
        (Locale::De, false, false) => {
            format!("{kind_label} {count_str}: keine in den Ergebnissen")
        }
        (Locale::It, true, true) => format!("{kind_label} {count_str}: nascondi dal grafo"),
        (Locale::It, true, false) => {
            format!("{kind_label} {count_str}: mostrato (richiesti almeno 2 tipi)")
        }
        (Locale::It, false, true) => format!("{kind_label} {count_str}: mostra nel grafo"),
        (Locale::It, false, false) => {
            format!("{kind_label} {count_str}: nessuno nei risultati")
        }
    }
}
