// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! German translation table.

use crate::i18n::TextKey;

// One flat match arm per `TextKey` variant; splitting the table into helper
// fns would hurt readability more than the line count helps it.
#[allow(
    clippy::too_many_lines,
    reason = "one flat table of translations; splitting it would mean a key is not findable by name"
)]
pub const fn de_t(key: TextKey) -> &'static str {
    match key {
        TextKey::Share => "Teilen",
        TextKey::ShareableLink => "Teilbarer Link",
        TextKey::TsvFileUpload => "TSV- / CSV-Datei",
        TextKey::Copy => "Kopieren",
        TextKey::Copied => "Kopiert!",
        TextKey::CopyToClipboard => "In die Zwischenablage kopieren",
        TextKey::Notice => "Hinweis",
        TextKey::Error => "Fehler",
        TextKey::DismissError => "Fehler schließen",
        TextKey::Language => "Sprache",
        TextKey::PageTitle => "LOTUS Explorer",
        TextKey::DarkModeToggle => "Thema hell/dunkel umschalten",
        TextKey::DarkMode => "Dunkel",
        TextKey::LightMode => "Hell",
        TextKey::SkipToResults => "Zum Hauptinhalt springen",
        TextKey::PageSubtitle => {
            "Finden Sie heraus, welche Verbindungen aus einem Organismus beschrieben wurden und in welchen Artikeln, so wie die LOTUS-Initiative es in Wikidata erfasst hat."
        }

        TextKey::PageNotFound => "Seite nicht gefunden",
        TextKey::PageNotFoundDescription => "Die angeforderte Seite existiert nicht.",
        TextKey::ReturnHome => "Zurück zur Startseite",
        TextKey::ResolvedTaxon => "Gesuchtes Taxon",
        TextKey::LinkedNamesIncluded => {
            "Einträge unter einem verknüpften Namen, etwa einem Synonym oder Basionym, sind eingeschlossen."
        }
        TextKey::DuplicateRowsHint => "Warum stehen manche Verbindungen mehrfach in der Liste?",
        TextKey::DuplicateRowsExplain => {
            "Jede Zeile ist ein Verbindung-Taxon-Referenz-Eintrag aus Wikidata. Eine in mehreren Taxa oder Artikeln berichtete Verbindung belegt mehrere Zeilen, und dasselbe Verbindung-Taxon-Referenz-Tripel kann mehrfach vorkommen, wenn mehrere Aussagen oder Synonyme zutreffen. Die kleinere Zahl neben der Eintragszahl gibt die verschiedenen Verbindung-Taxon-Referenz-Tripel an; verschiedene Verbindungen werden separat gezählt."
        }
        TextKey::QueryHash => "Abfrage-Hash",
        TextKey::ResultHash => "Ergebnis-Hash",
        TextKey::CopyTaxonQid => "Taxon-QID kopieren",
        TextKey::CopyFullQueryHash => "Vollständigen Abfrage-Hash kopieren (SHA-256)",
        TextKey::CopyFullResultHash => "Vollständigen Ergebnis-Hash kopieren (SHA-256)",
        TextKey::CopyShareableLink => "Freigabelink kopieren",
        TextKey::Unique => "eindeutige",
        TextKey::LoadingTitle => "Wikidata wird über QLever abgefragt...",
        TextKey::LoadingHint => "Große Ergebnismengen können einige Sekunden dauern.",
        TextKey::LoadingResolvingStructure => "Struktur wird aufgelöst...",
        TextKey::LoadingResolvingReference => "Referenz wird aufgelöst...",
        TextKey::LoadingResolvingTaxon => "Taxon wird aufgelöst...",
        TextKey::LoadingFetchingResults => "Ergebnisse werden geladen...",
        TextKey::LoadingRowsSoFar => "bisher erhalten",
        TextKey::LoadingProcessingResults => "Ergebnisanzahlen werden verarbeitet...",
        TextKey::LoadingRendering => "Tabelle wird gerendert...",
        TextKey::Retry => "Erneut versuchen",
        TextKey::ErrorHintValidation => "Bitte Eingaben prüfen, dann erneut versuchen.",
        TextKey::ErrorHintConfiguration => {
            "Für diese Umgebung fehlt die erforderliche Dienstkonfiguration."
        }
        TextKey::ErrorHintNetwork => "Netzwerkproblem erkannt. Ein erneuter Versuch kann helfen.",
        TextKey::ErrorHintRateLimit => {
            "Ratenlimit beim vorgelagerten Dienst erreicht. Bitte etwa eine Minute warten und erneut versuchen."
        }
        TextKey::ErrorHintQueryTooExpensive => {
            "Diese Suche ist zu breit: Der Abfragedienst hat sie an seinem Zeitlimit abgebrochen. Grenzen Sie sie mit Taxon, Massenbereich, Jahr oder Formel ein und führen Sie sie erneut aus."
        }
        TextKey::ErrorHintBadRequest => {
            "Der Server hat die Anfrage abgelehnt. Bitte Suchparameter prüfen."
        }
        TextKey::ErrorHintParse => {
            "Antwort konnte nicht verarbeitet werden. Erneut versuchen oder Abfrage verfeinern."
        }
        TextKey::ErrorHintTruncated => {
            "Diese Ergebnismenge wurde abgebrochen; die angezeigten Zeilen sind nicht die vollständige Antwort. Suche einschränken, um sie abzuschließen."
        }
        TextKey::ErrorHintUnknown => "Unerwarteter Fehler. Ein erneuter Versuch kann helfen.",

        TextKey::ExampleQueryExecute => "Ausführen",
        TextKey::ExampleQueryTaxon | TextKey::DownloadCsvLabel => "CSV herunterladen",
        TextKey::ExampleQueryStructure | TextKey::DownloadJsonLabel => "JSON herunterladen",
        TextKey::ExampleQueryAdvanced | TextKey::DownloadRdfLabel => "RDF herunterladen",
        TextKey::ExampleApiUrls => "Beispiele für API-URLs",

        TextKey::SearchExamples => "Suchbeispiele",

        TextKey::AdvancedFilters => "Erweiterte Filter",
        TextKey::Taxon | TextKey::TaxonCol => "Taxon",
        TextKey::TaxonField => "Wissenschaftlicher oder Trivialname des Taxons",
        TextKey::TaxonPlaceholder => "ein Name, eine QID oder * für alles",
        TextKey::TaxonNomenclature => "Auch unter anderen Namen suchen",
        TextKey::TaxonNomenclatureAccepted => "akzeptierter Name und seine Synonyme",
        TextKey::TaxonNomenclatureBasionym => "Basionym (der Name der Erstbeschreibung)",
        TextKey::TaxonNomenclatureProtonym => "Originalkombination (der Name bei Erstpublikation)",
        TextKey::TaxonNomenclatureReplacement => "Ersatzname (nomen novum) und der ersetzte Name",

        TextKey::Examples => "Beispiele",
        TextKey::ExampleSets => "Feld auf",
        TextKey::StructureSmilesOrMol => "Struktur, Verbindungsname oder InChIKey",
        TextKey::StructurePlaceholder => "SMILES, ein Molfile, ein Name oder ein InChIKey",
        TextKey::Exact => "Exakt (eine eingefügte Struktur kann auch ihre Stereoisomere treffen)",
        TextKey::Substructure => "Substruktur",
        TextKey::Similarity => "Ähnlichkeit",
        TextKey::StructureSearchMode => "Struktursuche",
        TextKey::ReferenceField => "Referenz-QID oder DOI",
        TextKey::EditCopyDaylightSmiles => "Bearbeiten -> Als Daylight SMILES kopieren",
        TextKey::CopyExtendedSmilesMol => "Als erweiterte SMILES / MOL V3000 kopieren",
        TextKey::FormulaFilter => "Formelfilter",
        TextKey::ExactFormula => "Exakte Formel",
        TextKey::MinCount => "min",
        TextKey::MaxCount => "max",
        TextKey::MinCountAria => "Mindestanzahl",
        TextKey::MaxCountAria => "Maximalanzahl",
        TextKey::ElementRequirement => "Bedingung",
        TextKey::ElementStateAllowed => "erlaubt",
        TextKey::ElementStateRequired => "erforderlich",
        TextKey::ElementStateExcluded => "ausgeschlossen",
        TextKey::Search => "Suchen",
        TextKey::Searching => "Suche...",
        TextKey::MolecularMass => "Molekulare Masse (Da)",
        TextKey::Min => "Min",
        TextKey::Max => "Max",
        TextKey::PublicationYear => "Publikationsjahr",
        TextKey::YearFrom => "Von",
        TextKey::YearTo => "Bis",
        TextKey::RunSearch => "Suche starten",
        TextKey::ResetSearchFilters => "Alle Filter zurücksetzen",
        TextKey::KetcherSummary => "Struktureditor (Ketcher)",
        TextKey::KetcherHintA => {
            "Möchten Sie eine Struktur zeichnen oder nachschlagen? Öffnen Sie den Tab "
        }

        TextKey::KetcherHintB => " und kopieren Sie sie dann mit ",

        TextKey::KetcherHintC => " (oder ",
        TextKey::KetcherHintD => ") und verwenden Sie sie im Strukturfeld der Registerkarte Suche.",

        TextKey::KetcherIframeTitle => "Ketcher-Struktureditor",
        TextKey::KetcherClickToLoad => "Klicken Sie, um den Ketcher-Struktureditor zu laden.",
        TextKey::KetcherNotBundled => {
            "Der Struktureditor war in diesem Build nicht enthalten. Führen Sie `cargo run -p lotus-web-assets --bin fetch-assets` aus und bauen Sie neu."
        }
        TextKey::KetcherPreparing => "Struktureditor wird vorbereitet (nur beim ersten Start)...",
        TextKey::DatasetStatistics => "Datensatz-Statistiken",
        TextKey::DownloadResults => "Ergebnisse herunterladen",
        TextKey::PreparingDownload => "Download wird vorbereitet...",
        TextKey::DownloadOnlyHint => {
            "Für diesen Download-Link wurden noch keine Ergebnisse abgerufen. Suche starten, um den Export zu erzeugen."
        }
        TextKey::StartingCsvDownload => "CSV-Download wird gestartet...",
        TextKey::PreparingJsonDownload => "JSON-Download wird vorbereitet...",
        TextKey::PreparingRdfDownload => "RDF-Download wird vorbereitet...",
        TextKey::DownloadCsvTitle => "Ergebnisse als CSV herunterladen",
        TextKey::DownloadJsonTitle => "Ergebnisse als JSON herunterladen",
        TextKey::DownloadRdfTitle => "Ergebnisse als RDF (Turtle) herunterladen",
        TextKey::DownloadMetadataTitle => "Schema.org-Metadaten herunterladen (JSON-LD)",
        TextKey::DownloadMetadataLabel => "Abfrage-Metadaten herunterladen",
        TextKey::OpenInQlever => "In QLever öffnen",
        TextKey::OpenInQleverTitle => "Diese Abfrage in der QLever-Weboberfläche öffnen",
        TextKey::OpenInEndpoint => "Im Endpoint öffnen",
        TextKey::OpenInEndpointTitle => "Diese Abfrage in der SPARQL-Endpoint-Weboberfläche öffnen",
        TextKey::NoResults => "Keine Ergebnisse. Bitte erweitern Sie die Suche.",
        TextKey::StageTaxonSearch => "Taxon-Auflösung",
        TextKey::StageStructureSearch => "Strukturauflösung",
        TextKey::StageResultsQuery => "Ergebnisabruf",
        TextKey::DisplayCappedHint => {
            "Aus Speichergründen werden auf diesem Gerät nur die ersten Zeilen angezeigt. Die Gesamtzahlen bleiben exakt."
        }
        TextKey::FilterColumn => "Filtern",
        TextKey::FilterTextPlaceholder => "Filtern…",
        TextKey::FilterShowing => "Angezeigt",
        TextKey::FilterOf => "von",
        TextKey::FilterNoMatches => "Keine Zeile entspricht diesen Filtern.",
        TextKey::ClearFilters => "Filter zurücksetzen",
        TextKey::Structure => "Struktur",
        TextKey::Compound => "Verbindung",
        TextKey::Mass => "Masse",
        TextKey::Formula => "Formel",
        TextKey::Reference => "Referenz",
        TextKey::Year => "Jahr",
        TextKey::FooterData => "Daten",
        TextKey::FooterCitation => "Zitat",
        TextKey::FooterCode => "Code",
        TextKey::FooterArchive => "Archiv",
        TextKey::FooterPrograms => "Programme",
        TextKey::FooterLicense => "Lizenz",
        TextKey::FooterForData => " für Wikidata-Daten ",
        TextKey::FooterForCode => " für Code",
        TextKey::TableTriplesAria => "Verbindung-Taxon-Referenz-Tripel",
        TextKey::OpenFullSizeDepiction => "Darstellung in voller Größe öffnen",
        TextKey::OpenInWikidata => "In Wikidata öffnen",
        TextKey::OpenInScholia => "In Scholia öffnen",
        TextKey::OpenInCompoundScholia => "Verbindung in Scholia öffnen",
        TextKey::OpenInTaxonScholia => "Taxon in Scholia öffnen",
        TextKey::OpenInReferenceScholia => "Referenz in Scholia öffnen",
        TextKey::OpenDoi => "DOI öffnen",
        TextKey::Statement => "Aussage · Melden",
        TextKey::SparqlQuery => "SPARQL-Abfrage",
        TextKey::CopySparqlQuery => "SPARQL-Abfrage kopieren",
        TextKey::ShowFailedQuery => {
            "Diese Suche wurde nicht abgeschlossen — verwendete Abfrage anzeigen"
        }
        TextKey::FailedQueryReconstructed => {
            "Aus Ihren Suchkriterien neu erstellt, also nah an der gesendeten Abfrage statt einer wortgetreuen Kopie. Passen Sie sie an und starten Sie erneut."
        }
        TextKey::StaleResults => {
            "Ihre Suchkriterien haben sich geändert. Führen Sie die Suche erneut aus, um Ergebnisse zu sehen."
        }
        TextKey::ViewInNetwork => "Im Netzwerk ansehen",
        TextKey::NetworkStarterHeading => "LOTUS-Wissensgraph erkunden",
        TextKey::NetworkStarterSubtitle => {
            "Öffnen Sie direkt eine interaktive Verbindung–Taxon–Referenz-Konstellation oder starten Sie unten eine Schnellsuche."
        }
        TextKey::NetworkQuickSearchLabel => "Schnellsuche nach Entität",
        TextKey::NetworkQuickSearchPlaceholder => "Taxon, Verbindung-QID/SMILES oder DOI/QID…",
        TextKey::NetworkQuickSearchSubmit => "Graph erkunden",
        TextKey::NetworkQuickSearchKindLabel => "Entitätstyp",
        TextKey::NetworkStarterTitleBetaSitosterol => "β-Sitosterol",
        TextKey::NetworkStarterDescBetaSitosterol => {
            "Verbreitetes Phytosterol (Q121802) über dokumentierte Pflanzentaxa"
        }
        TextKey::NetworkStarterTitleGentianaGenus => "Gentiana (Gattung)",
        TextKey::NetworkStarterDescGentianaGenus => {
            "Alpen-Enziane: Secoiridoide, Xanthone und Taxa"
        }
        TextKey::NetworkStarterTitleGentianaLutea => "Gentiana-lutea-Literatur",
        TextKey::NetworkStarterDescGentianaLutea => {
            "Gelber Enzian (Q158572): Verbindungen verknüpft mit Primärartikeln"
        }
        TextKey::NetworkStarterTitleIndole => "Indol-Substruktur",
        TextKey::NetworkStarterDescIndole => "Verbindungen mit Indolring über biologische Taxa",
        TextKey::OpenAdvancedSearch => "Erweiterte Suche & Filter öffnen",
        TextKey::PreviousSearch => "Vorherige Suche",
        TextKey::TableAndFilters => "Tabelle / Filter bearbeiten",
        TextKey::GraphProjectionLabel => "Graph-Ansicht",
        TextKey::ProjectionAuto => "Auto",
        TextKey::ProjectionCompoundTaxon => "Verbindungen ↔ Taxa",
        TextKey::ProjectionCompoundReference => "Verbindungen ↔ Referenzen",
        TextKey::ProjectionTaxonReference => "Taxa ↔ Referenzen",
        TextKey::ProjectionTripartite => "Alle 3 Typen",
        TextKey::GraphNodeSearchLabel => "Entität im Graph finden",
        TextKey::GraphNodeSearchPlaceholder => "Knoten finden (Taste /)…",
        TextKey::GraphZoomIn => "Zoom +",
        TextKey::GraphZoomOut => "Zoom −",
        TextKey::GraphResetView => "Ansicht zurücksetzen",
        TextKey::GraphPauseMotion => "Bewegung pausieren",
        TextKey::GraphResumeMotion => "Bewegung fortsetzen",
        TextKey::GraphDownloadPng => "PNG herunterladen",
        TextKey::GraphNodeLimitLabel => "Knotenlimit",
        TextKey::NetworkWorkspaceRegionAria => "Interaktiver LOTUS-Netzwerkbereich",
        TextKey::NetworkControlsNavAria => "Netzwerkgraph-Steuerung",
        TextKey::NetworkInspectorRegionAria => "Entitätsinspektor und verknüpfte Nachbarn",
        TextKey::GraphCanvasAriaPrefix => "Interaktives LOTUS Verbindung–Taxon–Referenz-Netzwerk",
        TextKey::GraphCanvasHint => {
            "Ziehen zum Drehen/Verschieben · Scrollen zum Zoomen · Knoten anklicken · Tasten: + − 0 Esc"
        }
        TextKey::GraphLegendAria => "Legende der Graphentitäten",
        TextKey::GraphBackToOverview => "Zurück zur Übersicht",
        TextKey::GraphEgoBadgePrefix => "Fokussierte Nachbarschaft:",
        TextKey::GraphCappedNoticePrefix => "Zeige Top",
        TextKey::GraphCappedNoticeSuffix => {
            "verknüpfte Knoten — nutzen Sie die Suche, um jede Entität zu fokussieren."
        }
        TextKey::GraphNoOccurrenceEdges => {
            "Diese Ergebnisse enthalten keine Taxon- oder Referenz-Verknüpfungen für eine Netzwerkansicht."
        }
        TextKey::GraphCenterNotInRows => {
            "Die fokussierte Entität wird von den aktuellen Tabellenfiltern ausgeschlossen."
        }
        TextKey::GraphWebglUnavailable => {
            "WebGL2 ist hier nicht verfügbar; nutzen Sie den Inspektor für Top-Hubs und Nachbarn."
        }
        TextKey::GraphPngSaved => "PNG-Schnappschuss gespeichert:",
        TextKey::GraphContextLost => "WebGL2-Kontext pausiert; wird wiederhergestellt…",
        TextKey::TopConnectedEntities => "Meistverknüpfte Entitäten",
        TextKey::FocusNeighborhood => "Nachbarschaft fokussieren",
        TextKey::FilterTableToEntity => "Tabelle darauf filtern",
        TextKey::ExploreInLotus => "In LOTUS erkunden",
        TextKey::LinkedEntities => "Verknüpfte Entitäten",
        TextKey::CloseInspector => "Inspektor schließen",
        TextKey::NodeDegreeLabel => "Links",
        TextKey::NodeOccurrencesLabel => "Vorkommen",
        TextKey::GraphCorroborationRefs => "Refs",
        TextKey::GraphCorroborationTaxa => "Taxa",
        TextKey::GraphCorroborationCompounds => "Verbindungen",
        TextKey::GraphSmilesLabel => "SMILES",
        TextKey::CopySmiles => "SMILES kopieren",
        TextKey::GraphActiveFilters => "Tabellenfilter aktiv",
        TextKey::GraphMode3d => "3D",
        TextKey::GraphMode2d => "2D",
        TextKey::GraphViewModeAria => "Graph-Dimensionsmodus",
    }
}
