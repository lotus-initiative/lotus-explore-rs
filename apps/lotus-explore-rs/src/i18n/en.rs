// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! English translation table.

use crate::i18n::TextKey;

// One flat match arm per `TextKey` variant; splitting the table into helper
// fns would hurt readability more than the line count helps it.
#[allow(
    clippy::too_many_lines,
    reason = "one flat table of translations; splitting it would mean a key is not findable by name"
)]
pub const fn en_t(key: TextKey) -> &'static str {
    match key {
        TextKey::Share => "Share",
        TextKey::ShareableLink => "Shareable link",
        TextKey::TsvFileUpload => "TSV / CSV file",
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
            "Find which compounds are reported from an organism, in which articles, as recorded in Wikidata by the LOTUS initiative."
        }

        TextKey::PageNotFound => "Page not found",
        TextKey::PageNotFoundDescription => "The page you requested does not exist.",
        TextKey::ReturnHome => "Return to the home page",
        TextKey::ResolvedTaxon => "Searched taxon",
        TextKey::LinkedNamesIncluded => {
            "Records filed under a linked name, such as a synonym or a basionym, are included."
        }
        TextKey::DuplicateRowsHint => "Why are some compounds listed more than once?",
        TextKey::DuplicateRowsExplain => {
            "Each row is one compound as reported by one reference, so a compound found by three papers occupies three rows that differ only in the reference. The occurrences are distinct; the compounds are not. The smaller number beside the row count is how many distinct compounds are in the result."
        }
        TextKey::QueryHash => "Query hash",
        TextKey::ResultHash => "Result hash",
        TextKey::CopyTaxonQid => "Copy taxon QID",
        TextKey::CopyFullQueryHash => "Copy full query hash (SHA-256)",
        TextKey::CopyFullResultHash => "Copy full result hash (SHA-256)",
        TextKey::CopyShareableLink => "Copy shareable link",
        TextKey::Unique => "Unique",
        TextKey::LoadingTitle => "Querying Wikidata via QLever...",
        TextKey::LoadingHint => "Large result sets may take several seconds.",
        TextKey::LoadingResolvingStructure => "Resolving structure...",
        TextKey::LoadingResolvingReference => "Resolving reference...",
        TextKey::LoadingResolvingTaxon => "Resolving taxon...",
        TextKey::LoadingFetchingResults => "Fetching results...",
        TextKey::LoadingRowsSoFar => "so far",
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
        TextKey::ErrorHintQueryTooExpensive => {
            "This search is too broad: the query service stopped it at its time limit. Narrow it with a taxon, a mass range, a year or a formula, then run it again."
        }
        TextKey::ErrorHintBadRequest => {
            "The server rejected the request. Check your search parameters."
        }
        TextKey::ErrorHintParse => "Response parsing failed. Retry or refine query.",
        TextKey::ErrorHintTruncated => {
            "This result set was cut short, so the rows shown are not the whole answer. Narrow the search to finish."
        }
        TextKey::ErrorHintUnknown => "Unexpected error. Retry may help.",
        TextKey::ExampleQueryExecute => "Execute",
        TextKey::ExampleQueryTaxon | TextKey::DownloadCsvLabel => "Download CSV",
        TextKey::ExampleQueryStructure | TextKey::DownloadJsonLabel => "Download JSON",
        TextKey::ExampleQueryAdvanced | TextKey::DownloadRdfLabel => "Download RDF",
        TextKey::ExampleApiUrls => "API URL examples",

        TextKey::SearchExamples => "Search examples",

        TextKey::AdvancedFilters => "Advanced filters",
        TextKey::Taxon | TextKey::TaxonCol => "Taxon",
        TextKey::TaxonField => "Taxon scientific or common name",
        TextKey::TaxonPlaceholder => "a name, a QID, or * for everything",
        TextKey::TaxonNomenclature => "Also search other names",
        TextKey::TaxonNomenclatureAccepted => "accepted name and its synonyms",
        TextKey::TaxonNomenclatureBasionym => "basionym (name it was first described under)",
        TextKey::TaxonNomenclatureProtonym => "original combination (name as first published)",
        TextKey::TaxonNomenclatureReplacement => {
            "replacement name (nomen novum) and the name it replaced"
        }

        TextKey::Examples => "Examples",
        TextKey::ExampleSets => "Set the field to",
        TextKey::StructureSmilesOrMol => "Structure, compound name or InChIKey",
        TextKey::StructurePlaceholder => "SMILES, a Molfile, a name or an InChIKey",
        TextKey::Exact => "Exact (a pasted structure can also match its stereoisomers)",
        TextKey::Substructure => "Substructure",
        TextKey::Similarity => "Similarity",
        TextKey::StructureSearchMode => "Structure search mode",
        TextKey::ReferenceField => "Reference QID or DOI",
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
        TextKey::ResetSearchFilters => "Reset filters",
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
        TextKey::KetcherPreparing => "Preparing the structure editor (first launch only)...",
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
        TextKey::DownloadMetadataLabel => "Download query metadata",
        TextKey::OpenInQlever => "Open in QLever",
        TextKey::OpenInQleverTitle => "Open this query in the QLever web interface",
        TextKey::OpenInEndpoint => "Open in endpoint",
        TextKey::OpenInEndpointTitle => "Open this query in the SPARQL endpoint web interface",
        TextKey::NoResults => "No results. Try broadening your search.",
        TextKey::StageTaxonSearch => "taxon lookup",
        TextKey::StageStructureSearch => "structure lookup",
        TextKey::StageResultsQuery => "results fetch",
        TextKey::DisplayCappedHint => {
            "Displaying the first rows only for memory safety on this device. Counts remain exact."
        }
        TextKey::FilterColumn => "Filter",
        TextKey::FilterTextPlaceholder => "Filter…",
        TextKey::FilterShowing => "Showing",
        TextKey::FilterOf => "of",
        TextKey::FilterNoMatches => "No rows match these filters.",
        TextKey::ClearFilters => "Clear filters",
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
        TextKey::FooterForData => " for Wikidata data ",
        TextKey::FooterForCode => " for code",
        TextKey::TableTriplesAria => "Compound-taxon-reference triples",
        TextKey::OpenFullSizeDepiction => "Open full-size depiction",
        TextKey::OpenInWikidata => "Open in Wikidata",
        TextKey::OpenInScholia => "Open in Scholia",
        TextKey::OpenInCompoundScholia => "Open compound in Scholia",
        TextKey::OpenInTaxonScholia => "Open taxon in Scholia",
        TextKey::OpenInReferenceScholia => "Open reference in Scholia",
        TextKey::OpenDoi => "Open DOI",
        TextKey::Statement => "Statement · Report",
        TextKey::SparqlQuery => "SPARQL Query",
        TextKey::CopySparqlQuery => "Copy SPARQL query",
        TextKey::ShowFailedQuery => "This search did not finish — show the query it used",
        TextKey::FailedQueryReconstructed => {
            "Rebuilt from your search criteria, so it is close to what was sent rather than a byte-for-byte copy. Use it to adjust and re-run."
        }
        TextKey::StaleResults => {
            "Your search criteria changed. Run the search again to see results for them."
        }
        TextKey::ViewInNetwork => "View in Network",
        TextKey::NetworkStarterHeading => "Explore the LOTUS knowledge graph",
        TextKey::NetworkStarterSubtitle => {
            "Jump straight into an interactive compound–taxon–reference constellation, or run a quick entity lookup below."
        }
        TextKey::NetworkQuickSearchLabel => "Quick entity search",
        TextKey::NetworkQuickSearchPlaceholder => "Taxon, compound QID/SMILES, or DOI/QID…",
        TextKey::NetworkQuickSearchSubmit => "Explore graph",
        TextKey::NetworkQuickSearchKindLabel => "Entity type",
        TextKey::NetworkStarterTitleBetaSitosterol => "β-Sitosterol",
        TextKey::NetworkStarterDescBetaSitosterol => {
            "Ubiquitous phytosterol (Q121802) across reported plant taxa"
        }
        TextKey::NetworkStarterTitleGentianaGenus => "Gentiana (genus)",
        TextKey::NetworkStarterDescGentianaGenus => {
            "Alpine gentians: secoiridoids, xanthones, and taxa"
        }
        TextKey::NetworkStarterTitleGentianaLutea => "Gentiana lutea literature",
        TextKey::NetworkStarterDescGentianaLutea => {
            "Yellow gentian (Q158572) compounds linked to primary articles"
        }
        TextKey::NetworkStarterTitleIndole => "Indole substructure",
        TextKey::NetworkStarterDescIndole => {
            "Compounds bearing an indole ring across biological taxa"
        }
        TextKey::OpenAdvancedSearch => "Open advanced search & filters",
        TextKey::PreviousSearch => "Previous search",
        TextKey::TableAndFilters => "Table / Edit filters",
        TextKey::GraphProjectionLabel => "Projection",
        TextKey::ProjectionAuto => "Auto",
        TextKey::ProjectionCompoundTaxon => "Compounds ↔ Taxa",
        TextKey::ProjectionCompoundReference => "Compounds ↔ References",
        TextKey::ProjectionTaxonReference => "Taxa ↔ References",
        TextKey::ProjectionTripartite => "All three",
        TextKey::GraphNodeSearchLabel => "Find entity in graph",
        TextKey::GraphNodeSearchPlaceholder => "Find node (press /)…",
        TextKey::GraphZoomIn => "Zoom +",
        TextKey::GraphZoomOut => "Zoom −",
        TextKey::GraphResetView => "Reset view",
        TextKey::GraphPauseMotion => "Pause motion",
        TextKey::GraphResumeMotion => "Resume motion",
        TextKey::GraphDownloadPng => "Download PNG",
        TextKey::GraphNodeLimitLabel => "Node cap",
        TextKey::NetworkWorkspaceRegionAria => "Interactive LOTUS network workspace",
        TextKey::NetworkControlsNavAria => "Network graph controls",
        TextKey::NetworkInspectorRegionAria => "Entity inspector and linked neighbors",
        TextKey::GraphCanvasAriaPrefix => "Interactive LOTUS compound–taxon–reference network",
        TextKey::GraphCanvasHint => {
            "Drag to orbit or pan · Scroll to zoom · Click a node to inspect · Keys: + − 0 Esc"
        }
        TextKey::GraphLegendAria => "Graph entity legend",
        TextKey::GraphBackToOverview => "Back to overview",
        TextKey::GraphEgoBadgePrefix => "Focused neighborhood:",
        TextKey::GraphCappedNoticePrefix => "Showing top",
        TextKey::GraphCappedNoticeSuffix => {
            "connected nodes from filtered results — use Find entity to focus any item."
        }
        TextKey::GraphNoOccurrenceEdges => {
            "These results have no taxon or reference occurrence links to project as a network."
        }
        TextKey::GraphCenterNotInRows => {
            "The focused entity is excluded by the current table filters."
        }
        TextKey::GraphWebglUnavailable => {
            "WebGL2 is unavailable in this environment; browse top hubs and linked entities in the inspector."
        }
        TextKey::GraphPngSaved => "Saved PNG snapshot:",
        TextKey::GraphContextLost => "WebGL2 context paused; restoring…",
        TextKey::TopConnectedEntities => "Top connected entities",
        TextKey::FocusNeighborhood => "Focus neighborhood",
        TextKey::FilterTableToEntity => "Filter table to this",
        TextKey::ExploreInLotus => "Explore in LOTUS",
        TextKey::LinkedEntities => "Linked entities",
        TextKey::CloseInspector => "Close inspector",
        TextKey::NodeDegreeLabel => "Links",
        TextKey::NodeOccurrencesLabel => "Occurrences",
        TextKey::GraphCorroborationRefs => "refs",
        TextKey::GraphCorroborationTaxa => "taxa",
        TextKey::GraphCorroborationCompounds => "compounds",
        TextKey::GraphSmilesLabel => "SMILES",
        TextKey::CopySmiles => "Copy SMILES",
        TextKey::GraphActiveFilters => "Table filters active",
        TextKey::GraphMode3d => "3D",
        TextKey::GraphMode2d => "2D",
        TextKey::GraphViewModeAria => "Graph dimension mode",
    }
}
