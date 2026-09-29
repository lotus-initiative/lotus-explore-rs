// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Secondary i18n helpers: the messages that describe an outcome rather than
//! label a control. Split from `i18n/mod.rs`, which holds the locale itself and
//! the control labels.

use super::{CountNoun, Locale};

mod aria;
mod errors;

pub use aria::*;
pub use errors::*;

pub fn threshold_label(locale: Locale, value: f64) -> String {
    match locale {
        Locale::En => format!("Threshold: {value:.2}"),
        Locale::Fr => format!("Seuil: {value:.2}"),
        Locale::De => format!("Grenzwert: {value:.2}"),
        Locale::It => format!("Soglia: {value:.2}"),
    }
}

use std::fmt::Write;

fn group_digits(mut value: usize, sep: char) -> String {
    if value < 1000 {
        return value.to_string();
    }

    let mut groups: Vec<usize> = Vec::new();
    while value >= 1000 {
        groups.push(value % 1000);
        value /= 1000;
    }

    let mut out = value.to_string();
    for group in groups.iter().rev() {
        out.push(sep);
        let _ = write!(out, "{group:03}");
    }
    out
}

pub fn format_count(locale: Locale, value: usize) -> String {
    let sep = match locale {
        Locale::En => ',',
        Locale::Fr => ' ',
        Locale::De | Locale::It => '.',
    };
    group_digits(value, sep)
}

pub const fn count_label(locale: Locale, noun: CountNoun, count: usize) -> &'static str {
    match (locale, noun, count == 1) {
        (Locale::En, CountNoun::Compound, true) => "Compound",
        (Locale::En, CountNoun::Compound, false) => "Compounds",
        (Locale::Fr, CountNoun::Compound, true) => "Composé",
        (Locale::Fr, CountNoun::Compound, false) => "Composés",
        (Locale::De, CountNoun::Compound, true) => "Verbindung",
        (Locale::De, CountNoun::Compound, false) => "Verbindungen",
        (Locale::It, CountNoun::Compound, true) => "Composto",
        (Locale::It, CountNoun::Compound, false) => "Composti",
        (Locale::En | Locale::Fr | Locale::De | Locale::It, CountNoun::Taxon, true) => "Taxon",
        (Locale::En | Locale::Fr | Locale::De | Locale::It, CountNoun::Taxon, false) => "Taxa",
        (Locale::En, CountNoun::Reference, true) => "Reference",
        (Locale::En, CountNoun::Reference, false) => "References",
        (Locale::Fr, CountNoun::Reference, true) => "Référence",
        (Locale::Fr, CountNoun::Reference, false) => "Références",
        (Locale::De, CountNoun::Reference, true) => "Referenz",
        (Locale::De, CountNoun::Reference, false) => "Referenzen",
        (Locale::It, CountNoun::Reference, true) => "Riferimento",
        (Locale::It, CountNoun::Reference, false) => "Riferimenti",
        (Locale::En, CountNoun::Entry, true) => "Entry",
        (Locale::En, CountNoun::Entry, false) => "Entries",
        (Locale::Fr, CountNoun::Entry, true) => "Entrée",
        (Locale::Fr, CountNoun::Entry, false) => "Entrées",
        (Locale::De, CountNoun::Entry, true) => "Eintrag",
        (Locale::De, CountNoun::Entry, false) => "Einträge",
        (Locale::It, CountNoun::Entry, true) => "Voce",
        (Locale::It, CountNoun::Entry, false) => "Voci",
    }
}
