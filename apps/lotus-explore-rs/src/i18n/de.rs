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
        TextKey::TsvFileUpload => "TSV-Datei",
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
            "Erkunden Sie verknüpfte offene Daten: chemische Entitäten, biologische Organismen und wissenschaftliche Literatur."
        }
        TextKey::LandingTitle => "Willkommen bei LOTUS Explorer",
        TextKey::OpenSearch => "Suche öffnen",

        TextKey::PageNotFound => "Seite nicht gefunden",
        TextKey::PageNotFoundDescription => "Die angeforderte Seite existiert nicht.",
        TextKey::ReturnHome => "Zurück zur Startseite",
        TextKey::ResolvedTaxon => "Aufgelöstes Taxon",
        TextKey::QueryHash => "Abfrage-Hash",
        TextKey::ResultHash => "Ergebnis-Hash",
        TextKey::CopyTaxonQid => "Taxon-QID kopieren",
        TextKey::CopyFullQueryHash => "Vollständigen Abfrage-Hash kopieren (SHA-256)",
        TextKey::CopyFullResultHash => "Vollständigen Ergebnis-Hash kopieren (SHA-256)",
        TextKey::CopyShareableLink => "Freigabelink kopieren",
        TextKey::Unique => "eindeutige",
        TextKey::LoadingTitle => "Wikidata wird über QLever abgefragt...",
        TextKey::LoadingHint => "Große Ergebnismengen können einige Sekunden dauern.",
        TextKey::LoadingResolvingTaxon => "Taxon wird aufgelöst...",
        TextKey::LoadingFetchingResults => "Ergebnisse werden geladen...",
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
        TextKey::ErrorHintBadRequest => {
            "Der Server hat die Anfrage abgelehnt. Bitte Suchparameter prüfen."
        }
        TextKey::ErrorHintParse => {
            "Antwort konnte nicht verarbeitet werden. Erneut versuchen oder Abfrage verfeinern."
        }
        TextKey::ErrorHintUnknown => "Unerwarteter Fehler. Ein erneuter Versuch kann helfen.",
        TextKey::WelcomeLeadA => {
            "Diese Anwendung veranschaulicht die Leistungsfähigkeit verknüpfter offener Daten, indem sie chemische Entitäten mit biologischen Organismen und wissenschaftlicher Literatur verknüpft. "
        }

        TextKey::WelcomeLeadB => {
            "Das Datenmodell verknüpft Verbindungen, Taxa und Referenzen, die aus der "
        }

        TextKey::WelcomeLeadC => ", veröffentlicht als verknüpfte offene Daten auf ",
        TextKey::WelcomeLeadD => " und über SPARQL abgefragt durch ",

        TextKey::WelcomeLeadE => ".",
        TextKey::ExampleQueryExecute => "Ausführen",
        TextKey::ExampleQueryTaxon | TextKey::DownloadCsvLabel => "CSV herunterladen",
        TextKey::ExampleQueryStructure | TextKey::DownloadJsonLabel => "JSON herunterladen",
        TextKey::ExampleQueryAdvanced | TextKey::DownloadRdfLabel => "RDF herunterladen",
        TextKey::ExampleApiUrls => "Beispiele für API-URLs",

        TextKey::SearchExamples => "Suchbeispiele",

        TextKey::LabelLanguagePolicy => {
            "Beschriftungen bevorzugen 'mul' und verwenden 'en' als Fallback, damit Ergebnisse vergleichbar bleiben."
        }
        TextKey::Taxon | TextKey::TaxonCol => "Taxon",
        TextKey::TaxonPlaceholder => "ein Name, eine QID oder * für alles",
        TextKey::TaxonNomenclature => "Auch unter anderen Namen suchen",
        TextKey::TaxonNomenclatureAccepted => "akzeptierter Name und seine Synonyme",
        TextKey::TaxonNomenclatureBasionym => "Basionym (der Name der Erstbeschreibung)",
        TextKey::TaxonNomenclatureProtonym => "Originalkombination (der Name bei Erstpublikation)",
        TextKey::TaxonNomenclatureReplacement => "Ersatzname (nomen novum) und der ersetzte Name",

        TextKey::Examples => "Beispiele",
        TextKey::ExampleSets => "Feld auf",
        TextKey::StructureSmilesOrMol => "SMILES oder Molfile",
        TextKey::StructurePlaceholder => "SMILES oder ein Molfile (V2000 / V3000)",
        TextKey::Substructure => "Substruktur",
        TextKey::Similarity => "Ähnlichkeit",
        TextKey::StructureSearchMode => "Struktursuche",
        TextKey::EditCopyDaylightSmiles => "Bearbeiten -> Als Daylight SMILES kopieren",
        TextKey::CopyExtendedSmilesMol => "Als erweiterte SMILES / MOL V3000 kopieren",
        TextKey::FormulaFilter => "Formelfilter",
        TextKey::ExactFormula => "Summenformel",
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
        TextKey::KindNoteSmiles => "  Wird als einzeiliges SPARQL-Literal gesendet.",
        TextKey::KindNoteMol2000 => {
            "  Wird unverändert an SACHEM scoredSubstructureSearch weitergegeben."
        }
        TextKey::KindNoteMol3000 => {
            "  Wird unverändert an SACHEM scoredSubstructureSearch weitergegeben (CTAB v3000)."
        }
        TextKey::DatasetStatistics => "Datensatz-Statistiken",
        TextKey::DownloadResults => "Ergebnisse herunterladen",
        TextKey::PreparingDownload => "Download wird vorbereitet...",
        TextKey::StartingCsvDownload => "CSV-Download wird gestartet...",
        TextKey::PreparingJsonDownload => "JSON-Download wird vorbereitet...",
        TextKey::PreparingRdfDownload => "RDF-Download wird vorbereitet...",
        TextKey::DownloadCsvTitle => "Ergebnisse als CSV herunterladen",
        TextKey::DownloadJsonTitle => "Ergebnisse als JSON herunterladen",
        TextKey::DownloadRdfTitle => "Ergebnisse als RDF (Turtle) herunterladen",
        TextKey::DownloadMetadataTitle => "Schema.org-Metadaten herunterladen (JSON-LD)",
        TextKey::DownloadMetadataLabel => "Metadaten herunterladen",
        TextKey::OpenInQlever => "In QLever öffnen",
        TextKey::OpenInQleverTitle => "Diese Abfrage in der QLever-Weboberfläche öffnen",
        TextKey::OpenInEndpoint => "In Endpoint öffnen",
        TextKey::OpenInEndpointTitle => "Diese Abfrage in der SPARQL-Endpoint-Weboberfläche öffnen",
        TextKey::NoResults => "Keine Ergebnisse. Bitte erweitern Sie die Suche.",
        TextKey::StageTaxonSearch => "Taxon-Auflösung",
        TextKey::StageResultsQuery => "Ergebnisabruf",
        TextKey::DisplayCappedHint => {
            "Aus Speichergründen werden auf diesem Gerät nur die ersten Zeilen angezeigt. Die Gesamtzahlen bleiben exakt."
        }
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
        TextKey::FooterForData => " für Daten ",
        TextKey::FooterForCode => " für Code",
        TextKey::TableTriplesAria => "Verbindung-Taxon-Referenz-Tripel",
        TextKey::OpenFullSizeDepiction => "Darstellung in voller Größe öffnen",
        TextKey::OpenInWikidata => "In Wikidata öffnen",
        TextKey::OpenInScholia => "In Scholia öffnen",
        TextKey::OpenInCompoundScholia => "Verbindung in Scholia öffnen",
        TextKey::OpenInTaxonScholia => "Taxon in Scholia öffnen",
        TextKey::OpenInReferenceScholia => "Referenz in Scholia öffnen",
        TextKey::OpenDoi => "DOI öffnen",
        TextKey::Statement => "Aussage",
        TextKey::SparqlQuery => "SPARQL-Abfrage",
        TextKey::CopySparqlQuery => "SPARQL-Abfrage kopieren",
        TextKey::StaleResults => {
            "Ihre Suchkriterien haben sich geändert. Führen Sie die Suche erneut aus, um Ergebnisse zu sehen."
        }
    }
}
