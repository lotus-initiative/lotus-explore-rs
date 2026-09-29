// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Typed error message keys for structured, i18n-aware error formatting.

use crate::i18n::Locale;

/// Typed keys for localized error messages and validation feedback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKey {
    // Validation errors
    InvalidSearchInput,
    TaxonTooLong,
    StructureTooLong,
    MassOutOfRange,
    MassRangeInvalid,
    YearOutOfRange,
    YearRangeInvalid,
    ElementCountTooHigh,

    // Taxon resolution errors
    TaxonNotFound,
    TaxonResolutionFailed,

    // Configuration errors
    ApiNotConfigured,

    // Format/parse errors
    UnsupportedFormat,
    TaxonParseFailed,

    // Query/processing errors
    QueryStageFailed,

    // Warnings
    InputStandardized,
    AmbiguousTaxon,

    // Platform-specific (wasm)
    #[cfg(target_arch = "wasm32")]
    WasmLargeQueryFallback,
    #[cfg(target_arch = "wasm32")]
    MemoryHint,
}

pub(crate) const ALL_ERROR_KEYS: &[ErrorKey] = &[
    ErrorKey::InvalidSearchInput,
    ErrorKey::TaxonTooLong,
    ErrorKey::StructureTooLong,
    ErrorKey::MassOutOfRange,
    ErrorKey::MassRangeInvalid,
    ErrorKey::YearOutOfRange,
    ErrorKey::YearRangeInvalid,
    ErrorKey::ElementCountTooHigh,
    ErrorKey::TaxonNotFound,
    ErrorKey::TaxonResolutionFailed,
    ErrorKey::ApiNotConfigured,
    ErrorKey::UnsupportedFormat,
    ErrorKey::TaxonParseFailed,
    ErrorKey::QueryStageFailed,
    ErrorKey::InputStandardized,
    ErrorKey::AmbiguousTaxon,
    #[cfg(target_arch = "wasm32")]
    ErrorKey::WasmLargeQueryFallback,
    #[cfg(target_arch = "wasm32")]
    ErrorKey::MemoryHint,
];

/// Resolve an error key to its localized message.
/// This function acts as the primary dispatcher for generic localized error text.
pub fn err(locale: Locale, key: ErrorKey) -> String {
    match locale {
        Locale::En => lookup_en(key),
        Locale::Fr => lookup_fr(key),
        Locale::De => lookup_de(key),
        Locale::It => lookup_it(key),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Locale-specific lookup implementations
// ─────────────────────────────────────────────────────────────────────────────

fn lookup_en(key: ErrorKey) -> String {
    match key {
        ErrorKey::InvalidSearchInput => {
            "Please enter a taxon name / QID, or a SMILES structure.".to_string()
        }
        ErrorKey::TaxonTooLong => {
            "Taxon input is too long. Please keep it under 500 characters.".to_string()
        }
        ErrorKey::StructureTooLong => {
            "Structure input is too long. Please shorten the SMILES/Molfile text.".to_string()
        }
        ErrorKey::MassOutOfRange => "Mass values must be between 0 and 10000.".to_string(),
        ErrorKey::MassRangeInvalid => "Mass minimum cannot exceed mass maximum.".to_string(),
        ErrorKey::YearOutOfRange => "Year is outside the supported range.".to_string(),
        ErrorKey::YearRangeInvalid => "Year from cannot exceed year to.".to_string(),
        ErrorKey::ElementCountTooHigh => "Formula element counts are too high.".to_string(),
        ErrorKey::TaxonNotFound => "Taxon not found.".to_string(),
        ErrorKey::TaxonResolutionFailed => "Taxon resolution failed.".to_string(),
        ErrorKey::ApiNotConfigured => "LOTUS API is not configured.".to_string(),
        ErrorKey::UnsupportedFormat => "Unsupported format.".to_string(),
        ErrorKey::TaxonParseFailed => "Taxon parse failed.".to_string(),
        ErrorKey::QueryStageFailed => "Query stage failed.".to_string(),
        ErrorKey::InputStandardized => "Input was standardized.".to_string(),
        ErrorKey::AmbiguousTaxon => "Ambiguous taxon name.".to_string(),
        #[cfg(target_arch = "wasm32")]
        ErrorKey::WasmLargeQueryFallback => "Large query fallback disabled.".to_string(),
        #[cfg(target_arch = "wasm32")]
        ErrorKey::MemoryHint => "Result too large for current device memory.".to_string(),
    }
}

fn lookup_fr(key: ErrorKey) -> String {
    match key {
        ErrorKey::InvalidSearchInput => {
            "Veuillez entrer un nom de taxon / QID, ou une structure SMILES.".to_string()
        }
        ErrorKey::TaxonTooLong => {
            "L'entrée du taxon est trop longue. Veuillez rester sous 500 caractères.".to_string()
        }
        ErrorKey::StructureTooLong => {
            "L'entrée de structure est trop longue. Veuillez raccourcir le texte SMILES/Molfile."
                .to_string()
        }
        ErrorKey::MassOutOfRange => {
            "Les valeurs de masse doivent être comprises entre 0 et 10000.".to_string()
        }
        ErrorKey::MassRangeInvalid => {
            "La masse minimale ne peut pas dépasser la masse maximale.".to_string()
        }
        ErrorKey::YearOutOfRange => {
            "L'année est en dehors de la plage prise en charge.".to_string()
        }
        ErrorKey::YearRangeInvalid => {
            "L'année de n'excessif ne peut pas dépasser l'année à.".to_string()
        }
        ErrorKey::ElementCountTooHigh => {
            "Les décomptes d'éléments de formule sont trop élevés.".to_string()
        }
        ErrorKey::TaxonNotFound => "Taxon non trouvé.".to_string(),
        ErrorKey::TaxonResolutionFailed => "La résolution du taxon a échoué.".to_string(),
        ErrorKey::ApiNotConfigured => "L'API LOTUS n'est pas configurée.".to_string(),
        ErrorKey::UnsupportedFormat => "Format non pris en charge.".to_string(),
        ErrorKey::TaxonParseFailed => "L'analyse du taxon a échoué.".to_string(),
        ErrorKey::QueryStageFailed => "L'étape de requête a échoué.".to_string(),
        ErrorKey::InputStandardized => "L'entrée a été normalisée.".to_string(),
        ErrorKey::AmbiguousTaxon => "Nom de taxon ambigu.".to_string(),
        #[cfg(target_arch = "wasm32")]
        ErrorKey::WasmLargeQueryFallback => {
            "Le secours à grande requête est désactivé.".to_string()
        }
        #[cfg(target_arch = "wasm32")]
        ErrorKey::MemoryHint => {
            "Résultat trop volumineux pour la mémoire actuelle de l'appareil.".to_string()
        }
    }
}

fn lookup_de(key: ErrorKey) -> String {
    match key {
        ErrorKey::InvalidSearchInput => {
            "Bitte geben Sie einen Taxonnamen / QID oder eine SMILES-Struktur ein.".to_string()
        }
        ErrorKey::TaxonTooLong => {
            "Taxoneingabe ist zu lang. Bitte halten Sie sich unter 500 Zeichen.".to_string()
        }
        ErrorKey::StructureTooLong => {
            "Struktureingabe ist zu lang. Bitte kürzen Sie den SMILES/Molfile-Text.".to_string()
        }
        ErrorKey::MassOutOfRange => "Massewerte müssen zwischen 0 und 10000 liegen.".to_string(),
        ErrorKey::MassRangeInvalid => {
            "Mindestmasse darf Maximalmasse nicht überschreiten.".to_string()
        }
        ErrorKey::YearOutOfRange => "Jahr liegt außerhalb des Unterstützungsbereichs.".to_string(),
        ErrorKey::YearRangeInvalid => "Anfangsjahr darf Endjahr nicht überschreiten.".to_string(),
        ErrorKey::ElementCountTooHigh => "Elementzahlen in der Formel sind zu hoch.".to_string(),
        ErrorKey::TaxonNotFound => "Taxon nicht gefunden.".to_string(),
        ErrorKey::TaxonResolutionFailed => "Taxonauflösung fehlgeschlagen.".to_string(),
        ErrorKey::ApiNotConfigured => "LOTUS-API ist nicht konfiguriert.".to_string(),
        ErrorKey::UnsupportedFormat => "Nicht unterstütztes Format.".to_string(),
        ErrorKey::TaxonParseFailed => "Taxonanalyse fehlgeschlagen.".to_string(),
        ErrorKey::QueryStageFailed => "Abfragestadium fehlgeschlagen.".to_string(),
        ErrorKey::InputStandardized => "Eingabe wurde standardisiert.".to_string(),
        ErrorKey::AmbiguousTaxon => "Mehrdeutiger Taxonname.".to_string(),
        #[cfg(target_arch = "wasm32")]
        ErrorKey::WasmLargeQueryFallback => "Große-Abfrage-Fallback ist deaktiviert.".to_string(),
        #[cfg(target_arch = "wasm32")]
        ErrorKey::MemoryHint => "Ergebnis zu groß für den aktuellen Gerätespeicher.".to_string(),
    }
}

fn lookup_it(key: ErrorKey) -> String {
    match key {
        ErrorKey::InvalidSearchInput => {
            "Inserisci un nome di taxon / QID o una struttura SMILES.".to_string()
        }
        ErrorKey::TaxonTooLong => {
            "L'ingresso del taxon è troppo lungo. Mantienilo sotto i 500 caratteri.".to_string()
        }
        ErrorKey::StructureTooLong => {
            "L'ingresso della struttura è troppo lungo. Abbrevia il testo SMILES/Molfile."
                .to_string()
        }
        ErrorKey::MassOutOfRange => {
            "I valori di massa devono essere compresi tra 0 e 10000.".to_string()
        }
        ErrorKey::MassRangeInvalid => {
            "La massa minima non può superare la massa massima.".to_string()
        }
        ErrorKey::YearOutOfRange => "L'anno è fuori dall'intervallo supportato.".to_string(),
        ErrorKey::YearRangeInvalid => "L'anno da non può superare l'anno a.".to_string(),
        ErrorKey::ElementCountTooHigh => {
            "I conteggi degli elementi della formula sono troppo alti.".to_string()
        }
        ErrorKey::TaxonNotFound => "Taxon non trovato.".to_string(),
        ErrorKey::TaxonResolutionFailed => "Risoluzione del taxon non riuscita.".to_string(),
        ErrorKey::ApiNotConfigured => "L'API LOTUS non è configurata.".to_string(),
        ErrorKey::UnsupportedFormat => "Formato non supportato.".to_string(),
        ErrorKey::TaxonParseFailed => "Analisi del taxon non riuscita.".to_string(),
        ErrorKey::QueryStageFailed => "Fase di query non riuscita.".to_string(),
        ErrorKey::InputStandardized => "L'ingresso è stato standardizzato.".to_string(),
        ErrorKey::AmbiguousTaxon => "Nome di taxon ambiguo.".to_string(),
        #[cfg(target_arch = "wasm32")]
        ErrorKey::WasmLargeQueryFallback => {
            "Il fallback di query di grandi dimensioni è disabilitato.".to_string()
        }
        #[cfg(target_arch = "wasm32")]
        ErrorKey::MemoryHint => {
            "Risultato troppo grande per la memoria del dispositivo corrente.".to_string()
        }
    }
}
