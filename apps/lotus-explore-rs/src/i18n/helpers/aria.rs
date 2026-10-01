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
