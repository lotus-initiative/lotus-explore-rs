// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! French translation table.

use crate::i18n::TextKey;

// One flat match arm per `TextKey` variant; splitting the table into helper
// fns would hurt readability more than the line count helps it.
#[allow(
    clippy::too_many_lines,
    reason = "one flat table of translations; splitting it would mean a key is not findable by name"
)]
pub const fn fr_t(key: TextKey) -> &'static str {
    match key {
        TextKey::Share => "Partager",
        TextKey::ShareableLink => "Lien partageable",
        TextKey::TsvFileUpload => "Fichier TSV",
        TextKey::Copy => "Copier",
        TextKey::Copied => "Copié\u{202f}!",
        TextKey::CopyToClipboard => "Copier dans le presse-papiers",
        TextKey::Notice => "Note",
        TextKey::Error => "Erreur",
        TextKey::DismissError => "Fermer l'erreur",
        TextKey::Language => "Langue",
        TextKey::PageTitle => "Explorateur LOTUS",
        TextKey::DarkModeToggle => "Basculer thème clair/sombre",
        TextKey::DarkMode => "Sombre",
        TextKey::LightMode => "Clair",
        TextKey::SkipToResults => "Passer au contenu principal",
        TextKey::PageSubtitle => {
            "Explorez des données ouvertes liées : entités chimiques, organismes biologiques et littérature scientifique."
        }
        TextKey::LandingTitle => "Bienvenue dans l'explorateur LOTUS",

        TextKey::OpenSearch => "Ouvrir la recherche",

        TextKey::PageNotFound => "Page introuvable",
        TextKey::PageNotFoundDescription => "La page demandée n'existe pas.",
        TextKey::ReturnHome => "Retour à l'accueil",
        TextKey::ResolvedTaxon => "Taxon résolu",
        TextKey::QueryHash => "Hash de la requête",
        TextKey::ResultHash => "Hash du résultat",
        TextKey::CopyTaxonQid => "Copier le QID du taxon",
        TextKey::CopyFullQueryHash => "Copier le hash complet de la requête (SHA-256)",
        TextKey::CopyFullResultHash => "Copier le hash complet du résultat (SHA-256)",
        TextKey::CopyShareableLink => "Copier le lien à partager",
        TextKey::Unique => "uniques",
        TextKey::LoadingTitle => "Interrogation de Wikidata via QLever...",
        TextKey::LoadingHint => "Les grands jeux de résultats peuvent prendre du temps.",
        TextKey::LoadingResolvingStructure => "Résolution de la structure...",
        TextKey::LoadingResolvingTaxon => "Résolution du taxon...",
        TextKey::LoadingFetchingResults => "Récupération des résultats...",
        TextKey::LoadingProcessingResults => "Traitement des comptages de résultats...",
        TextKey::LoadingRendering => "Rendu du tableau...",
        TextKey::Retry => "Réessayer",
        TextKey::ErrorHintValidation => "Veuillez ajuster la saisie puis réessayer.",
        TextKey::ErrorHintConfiguration => {
            "Cette instance ne dispose pas de la configuration de service requise."
        }
        TextKey::ErrorHintNetwork => "Problème réseau détecté. Réessayer peut aider.",
        TextKey::ErrorHintRateLimit => {
            "Limite de débit atteinte sur le service amont. Attendez environ une minute puis réessayez."
        }
        TextKey::ErrorHintBadRequest => {
            "Le serveur a rejeté la requête. Vérifiez les paramètres de recherche."
        }
        TextKey::ErrorHintParse => {
            "Échec de lecture de la réponse. Réessayez ou affinez la requête."
        }
        TextKey::ErrorHintUnknown => "Erreur inattendue. Réessayer peut aider.",
        TextKey::WelcomeLeadA => {
            "Cette application démontre la puissance des données ouvertes liées en reliant des entités chimiques à des organismes biologiques et à la littérature scientifique. "
        }

        TextKey::WelcomeLeadB => {
            "Le modèle de données relie les composés, les taxa et les références, qui proviennent de "
        }

        TextKey::WelcomeLeadC => ", publiées en tant que données ouvertes liées sur ",
        TextKey::WelcomeLeadD => " et interrogées via SPARQL par ",
        TextKey::WelcomeLeadE => ".",
        TextKey::ExampleQueryExecute => "Exécuter",
        TextKey::ExampleQueryTaxon | TextKey::DownloadCsvLabel => "Télécharger CSV",
        TextKey::ExampleQueryStructure | TextKey::DownloadJsonLabel => "Télécharger JSON",
        TextKey::ExampleQueryAdvanced | TextKey::DownloadRdfLabel => "Télécharger RDF",
        TextKey::ExampleApiUrls => "Exemples d'URL d'API",

        TextKey::SearchExamples => "Exemples de recherche",

        TextKey::LabelLanguagePolicy => {
            "Les libellés privilégient 'mul' et utilisent 'en' en repli afin de garantir des résultats comparables."
        }
        TextKey::AdvancedFilters => "Filtres avancés",
        TextKey::Taxon | TextKey::TaxonCol => "Taxon",
        TextKey::TaxonField => "Nom scientifique ou nom commun du taxon",
        TextKey::TaxonPlaceholder => "un nom, un QID, ou * pour tout",
        TextKey::TaxonNomenclature => "Chercher aussi sous d’autres noms",
        TextKey::TaxonNomenclatureAccepted => "nom accepté et ses synonymes",
        TextKey::TaxonNomenclatureBasionym => "basionyme (nom de description initiale)",
        TextKey::TaxonNomenclatureProtonym => "combinaison originale (nom tel que publié)",
        TextKey::TaxonNomenclatureReplacement => {
            "nom de remplacement (nomen novum) et nom remplacé"
        }

        TextKey::Examples => "Exemples",
        TextKey::ExampleSets => "Remplir le champ avec",
        TextKey::StructureSmilesOrMol => "Structure, nom de composé ou InChIKey",
        TextKey::StructurePlaceholder => "SMILES, un Molfile, un nom ou une clé InChI",
        TextKey::Exact => "Exact",
        TextKey::Substructure => "Sous-structure",
        TextKey::Similarity => "Similarité",
        TextKey::StructureSearchMode => "Recherche par structure",

        TextKey::EditCopyDaylightSmiles => "Édition -> Copier en tant que SMILES Daylight",
        TextKey::CopyExtendedSmilesMol => "Copier au format SMILES étendu / MOL V3000",

        TextKey::FormulaFilter => "Filtre de formule",
        TextKey::ExactFormula | TextKey::Formula => "Formule exacte",
        TextKey::MinCount => "min",
        TextKey::MaxCount => "max",
        TextKey::MinCountAria => "compte minimum",
        TextKey::MaxCountAria => "compte maximum",
        TextKey::ElementRequirement => "contrainte",
        TextKey::ElementStateAllowed => "autorisé",
        TextKey::ElementStateRequired => "requis",
        TextKey::ElementStateExcluded => "exclu",
        TextKey::Search => "Rechercher",
        TextKey::Searching => "Recherche...",
        TextKey::MolecularMass => "Masse moléculaire (Da)",
        TextKey::Min => "Min",
        TextKey::Max => "Max",
        TextKey::PublicationYear => "Année de publication",
        TextKey::YearFrom => "De",
        TextKey::YearTo => "À",
        TextKey::RunSearch => "Lancer la recherche",
        TextKey::KetcherSummary => "Éditeur de structure (Ketcher)",
        TextKey::KetcherHintA => {
            "Besoin de dessiner une structure ou d'en trouver une ? Ouvrez l'onglet "
        }

        TextKey::KetcherHintB => ", puis copiez avec ",
        TextKey::KetcherHintC => " (ou ",
        TextKey::KetcherHintD => {
            ") puis utilisez-la dans le champ structure de l'onglet Recherche."
        }
        TextKey::KetcherIframeTitle => "Éditeur de structure Ketcher",
        TextKey::KetcherClickToLoad => "Cliquez pour charger l'éditeur de structure Ketcher.",
        TextKey::KetcherNotBundled => {
            "L'éditeur de structure n'a pas été inclus dans cette version. Lancez `cargo run -p lotus-web-assets --bin fetch-assets` puis recompilez."
        }
        TextKey::KetcherPreparing => {
            "Préparation de l'éditeur de structure (premier lancement uniquement)..."
        }
        TextKey::DatasetStatistics => "Statistiques du jeu de données",
        TextKey::DownloadResults => "Télécharger les résultats",
        TextKey::PreparingDownload => "Préparation du téléchargement...",
        TextKey::StartingCsvDownload => "Démarrage du téléchargement CSV...",
        TextKey::PreparingJsonDownload => "Préparation du téléchargement JSON...",
        TextKey::PreparingRdfDownload => "Préparation du téléchargement RDF...",
        TextKey::DownloadCsvTitle => "Télécharger les résultats en CSV",
        TextKey::DownloadJsonTitle => "Télécharger les résultats en JSON",
        TextKey::DownloadRdfTitle => "Télécharger les résultats en RDF (Turtle)",
        TextKey::DownloadMetadataTitle => {
            "Télécharger les métadonnées de la requête (JSON-LD Schema.org)"
        }
        TextKey::DownloadMetadataLabel => "Télécharger les métadonnées de la requête",
        TextKey::OpenInQlever => "Ouvrir dans QLever",
        TextKey::OpenInQleverTitle => "Ouvrir cette requête dans l'interface web de QLever",
        TextKey::OpenInEndpoint => "Ouvrir dans l'endpoint",
        TextKey::OpenInEndpointTitle => {
            "Ouvrir cette requête dans l'interface web de l'endpoint SPARQL"
        }
        TextKey::NoResults => "Aucun résultat. Essayez une recherche plus large.",
        TextKey::StageTaxonSearch => "résolution du taxon",
        TextKey::StageResultsQuery => "récupération des résultats",
        TextKey::DisplayCappedHint => {
            "Affichage des premières lignes uniquement pour préserver la mémoire de l'appareil. Les totaux restent exacts."
        }
        TextKey::FilterColumn => "Filtrer",
        TextKey::FilterTextPlaceholder => "Filtrer…",
        TextKey::FilterShowing => "Affichage de",
        TextKey::FilterOf => "sur",
        TextKey::FilterNoMatches => "Aucune ligne ne correspond à ces filtres.",
        TextKey::ClearFilters => "Effacer les filtres",
        TextKey::Structure => "Structure",
        TextKey::Compound => "Composé",
        TextKey::Mass => "Masse",
        TextKey::Reference => "Référence",
        TextKey::Year => "Année",
        TextKey::FooterData => "Données",
        TextKey::FooterCitation => "Citation",
        TextKey::FooterCode => "Code",
        TextKey::FooterArchive => "Archive",
        TextKey::FooterPrograms => "Programmes",
        TextKey::FooterLicense => "Licence",
        TextKey::FooterForData => " pour les données ",
        TextKey::FooterForCode => " pour le code",
        TextKey::TableTriplesAria => "Triplets composé-taxon-référence",
        TextKey::OpenFullSizeDepiction => "Ouvrir la représentation en taille complète",
        TextKey::OpenInWikidata => "Ouvrir dans Wikidata",
        TextKey::OpenInScholia => "Ouvrir dans Scholia",
        TextKey::OpenInCompoundScholia => "Ouvrir le composé dans Scholia",
        TextKey::OpenInTaxonScholia => "Ouvrir le taxon dans Scholia",
        TextKey::OpenInReferenceScholia => "Ouvrir la référence dans Scholia",
        TextKey::OpenDoi => "Ouvrir DOI",
        TextKey::Statement => "Déclaration",
        TextKey::SparqlQuery => "Requête SPARQL",
        TextKey::CopySparqlQuery => "Copier la requête SPARQL",
        TextKey::StaleResults => {
            "Vos critères de recherche ont changé. Relancez la recherche pour voir les résultats correspondants."
        }
    }
}
