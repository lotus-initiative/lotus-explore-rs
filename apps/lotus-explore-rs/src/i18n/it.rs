// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Italian translation table.

use crate::i18n::TextKey;

// One flat match arm per `TextKey` variant; splitting the table into helper
// fns would hurt readability more than the line count helps it.
#[allow(
    clippy::too_many_lines,
    reason = "one flat table of translations; splitting it would mean a key is not findable by name"
)]
pub const fn it_t(key: TextKey) -> &'static str {
    match key {
        TextKey::Share => "Condividi",
        TextKey::ShareableLink => "Link condivisibile",
        TextKey::TsvFileUpload => "File TSV",
        TextKey::Copy => "Copia",
        TextKey::Copied => "Copiato!",
        TextKey::CopyToClipboard => "Copia negli appunti",
        TextKey::Notice => "Nota",
        TextKey::Error => "Errore",
        TextKey::DismissError => "Chiudi errore",
        TextKey::Language => "Lingua",
        TextKey::PageTitle => "Esploratore LOTUS",
        TextKey::DarkModeToggle => "Attiva/disattiva tema chiaro/scuro",
        TextKey::DarkMode => "Scuro",
        TextKey::LightMode => "Chiaro",
        TextKey::SkipToResults => "Vai al contenuto principale",
        TextKey::PageSubtitle => {
            "Esplora dati aperti collegati: entità chimiche, organismi biologici e letteratura scientifica."
        }
        TextKey::LandingTitle => "Benvenuti nell'esploratore LOTUS",

        TextKey::OpenSearch => "Apri la ricerca",

        TextKey::PageNotFound => "Pagina non trovata",
        TextKey::PageNotFoundDescription => "La pagina richiesta non esiste.",
        TextKey::ReturnHome => "Torna alla home page",
        TextKey::ResolvedTaxon => "Taxon risolto",
        TextKey::DuplicateRowsHint => "Perché alcuni composti compaiono più di una volta?",
        TextKey::DuplicateRowsExplain => {
            "Ogni riga è un composto così come un riferimento lo riporta: un composto trovato da tre articoli occupa tre righe che differiscono solo per il riferimento. Le occorrenze sono distinte, i composti no. Il numero più piccolo accanto al conteggio delle righe indica quanti composti distinti contiene il risultato."
        }
        TextKey::QueryHash => "Hash della query",
        TextKey::ResultHash => "Hash del risultato",
        TextKey::CopyTaxonQid => "Copia QID del taxon",
        TextKey::CopyFullQueryHash => "Copia hash completo della query (SHA-256)",
        TextKey::CopyFullResultHash => "Copia hash completo del risultato (SHA-256)",
        TextKey::CopyShareableLink => "Copia link condivisibile",
        TextKey::Unique => "Uniche",
        TextKey::LoadingTitle => "Interrogazione di Wikidata tramite QLever...",
        TextKey::LoadingHint => "Un elevato numero di risultati può richiedere alcuni secondi.",
        TextKey::LoadingResolvingStructure => "Risoluzione della struttura...",
        TextKey::LoadingResolvingReference => "Risoluzione del riferimento...",
        TextKey::LoadingResolvingTaxon => "Risoluzione del taxon...",
        TextKey::LoadingFetchingResults => "Recupero risultati...",
        TextKey::LoadingRowsSoFar => "finora",
        TextKey::LoadingProcessingResults => "Elaborazione dei conteggi dei risultati...",
        TextKey::LoadingRendering => "Rendering della tabella...",
        TextKey::Retry => "Riprova",
        TextKey::ErrorHintValidation => "Controlla l'input e riprova.",
        TextKey::ErrorHintConfiguration => {
            "In questo ambiente manca la configurazione di servizio richiesta."
        }
        TextKey::ErrorHintNetwork => "Problema di rete rilevato. Riprova può riuscire.",
        TextKey::ErrorHintRateLimit => {
            "Limite di richieste raggiunto sul servizio upstream. Attendi circa un minuto e riprova."
        }
        TextKey::ErrorHintQueryTooExpensive => {
            "Questa ricerca è troppo ampia: il servizio si è interrotto dopo 30 secondi. Restringila con un taxon, un intervallo di massa, un anno o una formula, poi riesegui la ricerca."
        }
        TextKey::ErrorHintBadRequest => {
            "Il server ha rifiutato la richiesta. Controlla i parametri di ricerca."
        }
        TextKey::ErrorHintParse => {
            "Impossibile interpretare la risposta. Riprova o affina la query."
        }
        TextKey::ErrorHintTruncated => {
            "Questo insieme di risultati è stato interrotto: le righe mostrate non sono la risposta completa. Restringi la ricerca per completarla."
        }
        TextKey::ErrorHintUnknown => "Errore inatteso. Riprova.",
        TextKey::WelcomeLeadA => {
            "Questa applicazione dimostra la potenza dei dati aperti collegati, mettendo in relazione entità chimiche, organismi biologici e letteratura scientifica. "
        }

        TextKey::WelcomeLeadB => {
            "Il modello di dati collega composti, taxa e riferimenti provenienti da "
        }

        TextKey::WelcomeLeadC => ", pubblicati come dati aperti collegati su ",
        TextKey::WelcomeLeadD => " e interrogati tramite SPARQL da ",

        TextKey::WelcomeLeadE => ".",
        TextKey::ExampleQueryExecute => "Esegui",
        TextKey::ExampleQueryTaxon | TextKey::DownloadCsvLabel => "Scarica CSV",
        TextKey::ExampleQueryStructure | TextKey::DownloadJsonLabel => "Scarica JSON",
        TextKey::ExampleQueryAdvanced | TextKey::DownloadRdfLabel => "Scarica RDF",
        TextKey::ExampleApiUrls => "Esempi di URL API",
        TextKey::SearchExamples => "Esempi di ricerca",

        TextKey::LabelLanguagePolicy => {
            "Le etichette preferiscono 'mul' e ricorrono a 'en' come fallback per mantenere confrontabili i risultati."
        }
        TextKey::AdvancedFilters => "Filtri avanzati",
        TextKey::Taxon | TextKey::TaxonCol => "Taxon",
        TextKey::TaxonField => "Nome scientifico o comune del taxon",
        TextKey::TaxonPlaceholder => "un nome, un QID o * per tutto",
        TextKey::TaxonNomenclature => "Cerca anche con altri nomi",
        TextKey::TaxonNomenclatureAccepted => "nome accettato e i suoi sinonimi",
        TextKey::TaxonNomenclatureBasionym => {
            "basionimo (il nome sotto cui è stato descritto originariamente)"
        }
        TextKey::TaxonNomenclatureProtonym => {
            "combinazione originale (il nome alla prima pubblicazione)"
        }
        TextKey::TaxonNomenclatureReplacement => "nome sostitutivo (nomen novum) e nome sostituito",

        TextKey::Examples => "Esempi",
        TextKey::ExampleSets => "Imposta il campo su",
        TextKey::StructureSmilesOrMol => "Struttura, nome del composto o InChIKey",
        TextKey::StructurePlaceholder => "SMILES, un Molfile, un nome o una chiave InChI",
        TextKey::Exact => "Esatto",
        TextKey::Substructure => "Sottostruttura",
        TextKey::Similarity => "Somiglianza",
        TextKey::StructureSearchMode => "Modalità di ricerca per struttura",
        TextKey::ReferenceField => "QID o DOI del riferimento",

        TextKey::EditCopyDaylightSmiles => "Modifica -> Copia come SMILES Daylight",
        TextKey::CopyExtendedSmilesMol => "Copia come SMILES esteso / MOL V3000",

        TextKey::FormulaFilter => "Filtro formula",
        TextKey::ExactFormula => "Formula esatta",
        TextKey::MinCount => "min",
        TextKey::MaxCount => "max",
        TextKey::MinCountAria => "conteggio minimo",
        TextKey::MaxCountAria => "conteggio massimo",
        TextKey::ElementRequirement => "vincolo",
        TextKey::ElementStateAllowed => "consentito",
        TextKey::ElementStateRequired => "richiesto",
        TextKey::ElementStateExcluded => "escluso",
        TextKey::Search => "Cerca",
        TextKey::Searching => "Ricerca...",
        TextKey::MolecularMass => "Massa molecolare (Da)",
        TextKey::Min => "Min",
        TextKey::Max => "Max",
        TextKey::PublicationYear => "Anno di pubblicazione",
        TextKey::YearFrom => "Da",
        TextKey::YearTo => "A",
        TextKey::RunSearch => "Avvia ricerca",
        TextKey::ResetSearchFilters => "Reimposta i filtri",
        TextKey::KetcherSummary => "Editor di strutture (Ketcher)",
        TextKey::KetcherHintA => "Devi disegnare o cercare una struttura? Apri la scheda ",
        TextKey::KetcherHintB => " e poi copia con ",
        TextKey::KetcherHintC => " (oppure ",
        TextKey::KetcherHintD => ") e usalo nel campo struttura della scheda Ricerca.",
        TextKey::KetcherIframeTitle => "Editor di strutture Ketcher",
        TextKey::KetcherClickToLoad => "Clicca per caricare l'editor di strutture Ketcher.",
        TextKey::KetcherNotBundled => {
            "L'editor di strutture non è incluso in questa build. Esegui `cargo run -p lotus-web-assets --bin fetch-assets` e ricompila."
        }
        TextKey::KetcherPreparing => {
            "Preparazione dell'editor di strutture (solo al primo avvio)..."
        }
        TextKey::DatasetStatistics => "Statistiche del dataset",
        TextKey::DownloadResults => "Scarica i risultati",
        TextKey::PreparingDownload => "Preparazione download...",
        TextKey::StartingCsvDownload => "Avvio download CSV...",
        TextKey::PreparingJsonDownload => "Preparazione download JSON...",
        TextKey::PreparingRdfDownload => "Preparazione download RDF...",
        TextKey::DownloadCsvTitle => "Scarica i risultati in CSV",
        TextKey::DownloadJsonTitle => "Scarica i risultati in JSON",
        TextKey::DownloadRdfTitle => "Scarica i risultati in RDF (Turtle)",
        TextKey::DownloadMetadataTitle => "Scarica metadati Schema.org (JSON-LD)",
        TextKey::DownloadMetadataLabel => "Scarica metadati della query",
        TextKey::OpenInQlever => "Apri in QLever",
        TextKey::OpenInQleverTitle => "Apri questa query nell'interfaccia web di QLever",
        TextKey::OpenInEndpoint => "Apri nell'endpoint",
        TextKey::OpenInEndpointTitle => {
            "Apri questa query nell'interfaccia web dell'endpoint SPARQL"
        }
        TextKey::NoResults => "Nessun risultato. Prova ad ampliare la ricerca.",
        TextKey::StageTaxonSearch => "risoluzione del taxon",
        TextKey::StageResultsQuery => "recupero risultati",
        TextKey::DisplayCappedHint => {
            "Per sicurezza di memoria su questo dispositivo vengono mostrate solo le prime righe. I conteggi restano esatti."
        }
        TextKey::FilterColumn => "Filtra",
        TextKey::FilterTextPlaceholder => "Filtra…",
        TextKey::FilterShowing => "Visualizzate",
        TextKey::FilterOf => "di",
        TextKey::FilterNoMatches => "Nessuna riga corrisponde a questi filtri.",
        TextKey::ClearFilters => "Azzera i filtri",
        TextKey::Structure => "Struttura",
        TextKey::Compound => "Composto",
        TextKey::Mass => "Massa",
        TextKey::Formula => "Formula",
        TextKey::Reference => "Riferimento",
        TextKey::Year => "Anno",
        TextKey::FooterData => "Dati",
        TextKey::FooterCitation => "Citazione",
        TextKey::FooterCode => "Codice",
        TextKey::FooterArchive => "Archivio",
        TextKey::FooterPrograms => "Programmi",
        TextKey::FooterLicense => "Licenza",
        TextKey::FooterForData => " per i dati ",
        TextKey::FooterForCode => " per il codice",
        TextKey::TableTriplesAria => "Triple composto-taxon-riferimento",
        TextKey::OpenFullSizeDepiction => "Apri la rappresentazione a grandezza naturale",
        TextKey::OpenInWikidata => "Apri in Wikidata",
        TextKey::OpenInScholia => "Apri in Scholia",
        TextKey::OpenInCompoundScholia => "Apri il composto in Scholia",
        TextKey::OpenInTaxonScholia => "Apri il taxon in Scholia",
        TextKey::OpenInReferenceScholia => "Apri il riferimento in Scholia",
        TextKey::OpenDoi => "Apri DOI",
        TextKey::Statement => "Dichiarazione",
        TextKey::SparqlQuery => "Query SPARQL",
        TextKey::CopySparqlQuery => "Copia query SPARQL",
        TextKey::StaleResults => {
            "I criteri di ricerca sono cambiati. Esegui di nuovo la ricerca per vedere i risultati."
        }
    }
}
