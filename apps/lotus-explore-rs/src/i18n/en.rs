// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! English translation table.

use crate::i18n::TextKey;

// One flat match arm per `TextKey` variant; splitting the table into helper
// fns would hurt readability more than the line count helps it.
#[allow(clippy::too_many_lines)]
pub const fn en_t(key: TextKey) -> &'static str {
    match key {
        TextKey::Share => "Share",
        TextKey::ShareableLink => "Shareable link",
        TextKey::TsvFileUpload => "TSV file",
        TextKey::Copy => "Copy",
        TextKey::Copied => "Copied!",
        TextKey::CopyToClipboard => "Copy to clipboard",
        TextKey::Notice => "Notice",
        TextKey::Error => "Error",
        TextKey::DismissError => "Dismiss error",
        TextKey::Language => "Language",
        TextKey::PageTitle => "LOTUS Explorer",
        TextKey::DarkModeToggle => "Toggle dark/light mode",
        TextKey::DarkMode => "Dark",
        TextKey::LightMode => "Light",
        TextKey::SkipToResults => "Skip to main content",
        TextKey::PageSubtitle => {
            "Explore linked open data: chemical entities, biological organisms, and scientific literature."
        }
        TextKey::LandingTitle => "Welcome to LOTUS Explorer",
        TextKey::OpenSearch => "Open search",

        TextKey::PageNotFound => "Page not found",
        TextKey::PageNotFoundDescription => "The page you requested does not exist.",
        TextKey::ReturnHome => "Return to the home page",
        TextKey::ResolvedTaxon => "Resolved taxon",
        TextKey::QueryHash => "Query hash",
        TextKey::ResultHash => "Result hash",
        TextKey::CopyTaxonQid => "Copy taxon QID",
        TextKey::CopyFullQueryHash => "Copy full query hash (SHA-256)",
        TextKey::CopyFullResultHash => "Copy full result hash (SHA-256)",
        TextKey::CopyShareableLink => "Copy shareable link",
        TextKey::Unique => "Unique",
        TextKey::LoadingTitle => "Querying Wikidata via QLever...",
        TextKey::LoadingHint => "Large result sets may take several seconds.",
        TextKey::LoadingResolvingTaxon => "Resolving taxon...",
        TextKey::LoadingFetchingResults => "Fetching results...",
        TextKey::LoadingProcessingResults => "Processing result counts...",
        TextKey::LoadingRendering => "Rendering table...",
        TextKey::Retry => "Retry",
        TextKey::ErrorHintValidation => "Please adjust your query input and try again.",
        TextKey::ErrorHintConfiguration => {
            "This environment is missing required service configuration."
        }
        TextKey::ErrorHintNetwork => "Network issue detected. Retry may succeed.",
        TextKey::ErrorHintRateLimit => {
            "Rate limit reached on the upstream service. Please wait about a minute and retry."
        }
        TextKey::ErrorHintBadRequest => {
            "The server rejected the request. Check your search parameters."
        }
        TextKey::ErrorHintParse => "Response parsing failed. Retry or refine query.",
        TextKey::ErrorHintUnknown => "Unexpected error. Retry may help.",
        TextKey::WelcomeLeadA => {
            "This app demonstrates the power of linked open data by connecting chemical entities to biological organisms and scientific literature. "
        }
        TextKey::WelcomeLeadB => {
            "The data model links compounds, taxa, and references, sourced from the "
        }
        TextKey::WelcomeLeadC => ", published as linked data on ",
        TextKey::WelcomeLeadD => ", and queried via SPARQL through ",
        TextKey::WelcomeLeadE => ".",
        TextKey::ExampleQueryExecute => "Execute",
        TextKey::ExampleQueryTaxon | TextKey::DownloadCsvLabel => "Download CSV",
        TextKey::ExampleQueryStructure | TextKey::DownloadJsonLabel => "Download JSON",
        TextKey::ExampleQueryAdvanced | TextKey::DownloadRdfLabel => "Download RDF",
        TextKey::ExampleApiUrls => "API URL examples",

        TextKey::SearchExamples => "Search examples",

        TextKey::LabelLanguagePolicy => {
            "Labels prefer 'mul' and fall back to 'en' so results remain comparable."
        }
        TextKey::Taxon | TextKey::TaxonCol => "Taxon",
        TextKey::TaxonPlaceholder => "Gentiana lutea - Q34317 - *",
        TextKey::StructureSmilesOrMol => "SMILES or Molfile",
        TextKey::StructurePlaceholder => "c1ccccc1   - or paste a Molfile (V2000 / V3000) block",
        TextKey::Substructure => "Substructure",
        TextKey::Similarity => "Similarity",
        TextKey::StructureSearchMode => "Structure search mode",
        TextKey::EditCopyDaylightSmiles => "Edit -> Copy as Daylight SMILES",
        TextKey::CopyExtendedSmilesMol => "Copy as Extended SMILES / MOL V3000",
        TextKey::FormulaFilter => "Formula filter",
        TextKey::ExactFormula => "Exact formula",
        TextKey::MinCount => "min",
        TextKey::MaxCount => "max",
        TextKey::MinCountAria => "minimum count",
        TextKey::MaxCountAria => "maximum count",
        TextKey::ElementRequirement => "requirement",
        TextKey::ElementStateAllowed => "allowed",
        TextKey::ElementStateRequired => "required",
        TextKey::ElementStateExcluded => "excluded",
        TextKey::Search => "Search",
        TextKey::Searching => "Searching...",
        TextKey::MolecularMass => "Molecular Mass (Da)",
        TextKey::Min => "Min",
        TextKey::Max => "Max",
        TextKey::PublicationYear => "Publication Year",
        TextKey::YearFrom => "From",
        TextKey::YearTo => "To",
        TextKey::RunSearch => "Run search",
        TextKey::KetcherSummary => "Structure editor (Ketcher)",
        TextKey::KetcherHintA => "Need to draw or look up a structure? Open the ",
        TextKey::KetcherHintB => " tab, then copy with ",
        TextKey::KetcherHintC => " (or ",
        TextKey::KetcherHintD => ") and use it in the Search structure field.",
        TextKey::KetcherIframeTitle => "Ketcher structure editor",
        TextKey::KetcherClickToLoad => "Click to load the Ketcher structure editor.",
        TextKey::KetcherNotBundled => {
            "The structure editor was not included in this build. Run `cargo run -p lotus-web-assets --bin fetch-assets` and rebuild."
        }
        TextKey::KindNoteSmiles => "  Sent as a single-line SPARQL literal.",
        TextKey::KindNoteMol2000 => "  Forwarded verbatim to SACHEM scoredSubstructureSearch.",
        TextKey::KindNoteMol3000 => {
            "  Forwarded verbatim to SACHEM scoredSubstructureSearch (CTAB v3000)."
        }
        TextKey::DatasetStatistics => "Dataset statistics",
        TextKey::DownloadResults => "Download results",
        TextKey::PreparingDownload => "Preparing download...",
        TextKey::StartingCsvDownload => "Starting CSV download...",
        TextKey::PreparingJsonDownload => "Preparing JSON download...",
        TextKey::PreparingRdfDownload => "Preparing RDF download...",
        TextKey::DownloadCsvTitle => "Download results as CSV",
        TextKey::DownloadJsonTitle => "Download results as JSON",
        TextKey::DownloadRdfTitle => "Download results as RDF (Turtle)",
        TextKey::DownloadMetadataTitle => "Download Schema.org metadata (JSON-LD)",
        TextKey::DownloadMetadataLabel => "Download metadata",
        TextKey::OpenInQlever => "Open in QLever",
        TextKey::OpenInQleverTitle => "Open this query in the QLever web interface",
        TextKey::OpenInEndpoint => "Open in endpoint",
        TextKey::OpenInEndpointTitle => "Open this query in the SPARQL endpoint web interface",
        TextKey::NoResults => "No results. Try broadening your search.",
        TextKey::StageTaxonSearch => "taxon lookup",
        TextKey::StageResultsQuery => "results fetch",
        TextKey::DisplayCappedHint => {
            "Displaying the first rows only for memory safety on this device. Counts remain exact."
        }
        TextKey::Structure => "Structure",
        TextKey::Compound => "Compound",
        TextKey::Mass => "Mass",
        TextKey::Formula => "Formula",
        TextKey::Reference => "Reference",
        TextKey::Year => "Year",
        TextKey::FooterData => "Data",
        TextKey::FooterCitation => "Citation",
        TextKey::FooterCode => "Code",
        TextKey::FooterArchive => "Archive",
        TextKey::FooterPrograms => "Programs",
        TextKey::FooterLicense => "License",
        TextKey::FooterForData => " for data ",
        TextKey::FooterForCode => " for code",
        TextKey::TableTriplesAria => "Compound-taxon-reference triples",
        TextKey::OpenFullSizeDepiction => "Open full-size depiction",
        TextKey::OpenInWikidata => "Open in Wikidata",
        TextKey::OpenInScholia => "Open in Scholia",
        TextKey::OpenInCompoundScholia => "Open compound in Scholia",
        TextKey::OpenInTaxonScholia => "Open taxon in Scholia",
        TextKey::OpenInReferenceScholia => "Open reference in Scholia",
        TextKey::OpenDoi => "Open DOI",
        TextKey::Statement => "Statement",
        TextKey::SparqlQuery => "SPARQL Query",
        TextKey::CopySparqlQuery => "Copy SPARQL query",
    }
}
