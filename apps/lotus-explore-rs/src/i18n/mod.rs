// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Minimal i18n helpers for user-facing labels and status text.

mod curation;
pub use curation::*;

pub mod faq;
pub use faq::*;

mod de;
mod en;
mod fr;
mod it;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    Fr,
    De,
    It,
}

impl Locale {
    fn from_lang_tag(lang_tag: &str) -> Option<Self> {
        // Extract the language subtag: "fr-CA" → "fr", "de_DE" → "de", "en-US" → "en"
        let lang = lang_tag
            .trim()
            .split(['-', '_'])
            .next()?
            .trim()
            .to_ascii_lowercase();
        match lang.as_str() {
            "fr" => Some(Self::Fr),
            "de" => Some(Self::De),
            "it" => Some(Self::It),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    pub fn detect(lang_hint: &str) -> Self {
        if let Some(locale) = Self::from_lang_tag(lang_hint) {
            return locale;
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(win) = web_sys::window() {
                let win_js = wasm_bindgen::JsValue::from(win);
                if let Ok(nav) =
                    js_sys::Reflect::get(&win_js, &wasm_bindgen::JsValue::from_str("navigator"))
                    && let Ok(lang) =
                        js_sys::Reflect::get(&nav, &wasm_bindgen::JsValue::from_str("language"))
                    && let Some(code) = lang.as_string()
                    && let Some(locale) = Self::from_lang_tag(&code)
                {
                    return locale;
                }
            }
        }

        Self::En
    }

    pub const fn lang_code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
            Self::De => "de",
            Self::It => "it",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountNoun {
    Compound,
    Taxon,
    Reference,
    Entry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKey {
    // Generic/meta
    Share,
    ShareableLink,
    TsvFileUpload,
    Copy,
    Copied,
    CopyToClipboard,
    Notice,
    Error,
    DismissError,
    Language,
    // Header
    PageTitle,
    DarkModeToggle,
    DarkMode,
    LightMode,
    SkipToResults,
    PageSubtitle,
    LandingTitle,
    OpenSearch,
    PageNotFound,
    PageNotFoundDescription,
    ReturnHome,
    ResolvedTaxon,

    QueryHash,
    ResultHash,
    CopyTaxonQid,
    CopyFullQueryHash,
    CopyFullResultHash,
    CopyShareableLink,
    Unique,
    // Loading/welcome
    LoadingTitle,
    LoadingHint,
    LoadingResolvingTaxon,
    LoadingResolvingStructure,
    LoadingResolvingReference,
    LoadingFetchingResults,
    LoadingProcessingResults,
    LoadingRendering,
    Retry,
    ErrorHintValidation,
    ErrorHintConfiguration,
    ErrorHintNetwork,
    ErrorHintRateLimit,
    ErrorHintBadRequest,
    ErrorHintParse,
    ErrorHintUnknown,
    WelcomeLeadA,
    WelcomeLeadB,
    WelcomeLeadC,
    WelcomeLeadD,
    WelcomeLeadE,
    ExampleQueryExecute,
    ExampleApiUrls,
    SearchExamples,
    ExampleQueryTaxon,
    ExampleQueryStructure,
    ExampleQueryAdvanced,
    LabelLanguagePolicy,
    // Search panel
    /// Heading of the collapsed sub-filter disclosure in each entity group.
    AdvancedFilters,
    /// The green heading of the taxon filter group.
    Taxon,
    /// The black label on the taxon input itself. A different key from
    /// [`Taxon`] because the field now accepts two kinds of name and says so,
    /// while the group it belongs to is about the organism either way.
    TaxonField,
    TaxonPlaceholder,
    TaxonNomenclature,
    TaxonNomenclatureAccepted,
    TaxonNomenclatureBasionym,
    TaxonNomenclatureProtonym,
    TaxonNomenclatureReplacement,
    Examples,
    ExampleSets,
    StructureSmilesOrMol,
    StructurePlaceholder,
    /// The compound the input resolved to, and nothing else.
    Exact,
    Substructure,
    Similarity,
    StructureSearchMode,
    ReferenceField,
    EditCopyDaylightSmiles,
    CopyExtendedSmilesMol,
    FormulaFilter,
    ExactFormula,
    MinCount,
    MaxCount,
    MinCountAria,
    MaxCountAria,
    ElementRequirement,
    ElementStateAllowed,
    ElementStateRequired,
    ElementStateExcluded,
    Search,
    Searching,
    MolecularMass,
    Min,
    Max,
    PublicationYear,
    YearFrom,
    YearTo,
    RunSearch,
    KetcherSummary,
    KetcherHintA,
    KetcherHintB,
    KetcherHintC,
    KetcherHintD,
    KetcherIframeTitle,
    KetcherClickToLoad,
    KetcherNotBundled,
    KetcherPreparing,
    // Error stage labels (used in transport error messages)
    StageTaxonSearch,
    StageResultsQuery,
    // Table/export
    DatasetStatistics,
    DownloadResults,
    PreparingDownload,
    StartingCsvDownload,
    PreparingJsonDownload,
    PreparingRdfDownload,
    DownloadCsvTitle,
    DownloadCsvLabel,
    DownloadJsonTitle,
    DownloadJsonLabel,
    DownloadRdfTitle,
    DownloadRdfLabel,
    DownloadMetadataTitle,
    DownloadMetadataLabel,
    OpenInQlever,
    OpenInQleverTitle,
    OpenInEndpoint,
    OpenInEndpointTitle,
    NoResults,
    DisplayCappedHint,
    // Column filters
    FilterColumn,
    FilterTextPlaceholder,
    FilterShowing,
    FilterOf,
    FilterNoMatches,
    ClearFilters,
    // Columns
    Structure,
    Compound,
    Mass,
    Formula,
    TaxonCol,
    Reference,
    Year,
    // Footer
    FooterData,
    FooterCitation,
    FooterCode,
    FooterArchive,
    FooterPrograms,
    FooterLicense,
    FooterForData,
    FooterForCode,
    TableTriplesAria,
    OpenFullSizeDepiction,
    OpenInWikidata,
    OpenInScholia,
    OpenInCompoundScholia,
    OpenInTaxonScholia,
    OpenInReferenceScholia,
    OpenDoi,
    Statement,
    SparqlQuery,
    CopySparqlQuery,
    /// Shown when the form was edited but the search was not re-run.
    StaleResults,
}

/// Resolve a [`TextKey`] for the given [`Locale`].
/// Delegates to the per-locale submodule functions so each translation table
/// lives in its own file and can be edited independently.
pub fn t(locale: Locale, key: TextKey) -> &'static str {
    match locale {
        Locale::En => en::en_t(key),
        Locale::Fr => fr::fr_t(key),
        Locale::De => de::de_t(key),
        Locale::It => it::it_t(key),
    }
}

mod helpers;

pub use helpers::*;

#[cfg(test)]
mod tests {
    use super::{Locale, TextKey, msg_tsv_missing_column, t};

    #[test]
    fn landing_copy_uses_chemical_entities_in_every_locale() {
        let expected = [
            (Locale::En, "chemical entities"),
            (Locale::Fr, "entités chimiques"),
            (Locale::De, "chemische Entitäten"),
            (Locale::It, "entità chimiche"),
        ];
        for (locale, phrase) in expected {
            assert!(t(locale, TextKey::PageSubtitle).contains(phrase));
            assert!(t(locale, TextKey::WelcomeLeadA).contains(phrase));
        }
    }

    #[test]
    fn tsv_missing_column_messages_are_localized() {
        for locale in [Locale::En, Locale::Fr, Locale::De, Locale::It] {
            let message = msg_tsv_missing_column(locale, "name");
            // `contains` already implies non-empty, so this asserts the column
            // name survived substitution in every locale, which is the thing that
            // can actually break.
            assert!(
                message.contains("name"),
                "{locale:?} dropped the column name: {message:?}"
            );
        }
    }
}
