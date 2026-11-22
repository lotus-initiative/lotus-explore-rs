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
        TextKey::TsvFileUpload => "Fichier TSV / CSV",
        TextKey::Copy => "Copier",
        TextKey::Copied => "Copié\u{202f}!",
        TextKey::CopyToClipboard => "Copier dans le presse-papiers",
        TextKey::Notice => "Note",
        TextKey::Error => "Erreur",
        TextKey::DismissError => "Fermer l'erreur",
        TextKey::Language => "Langue",
        TextKey::PageTitle => "Explorateur LOTUS",
        TextKey::DarkModeToggle => "Basculer le thème clair/sombre",
        TextKey::DarkMode => "Sombre",
        TextKey::LightMode => "Clair",
        TextKey::SkipToResults => "Passer au contenu principal",
        TextKey::PageSubtitle => {
            "Trouvez quels composés sont rapportés dans un organisme, et dans quels articles, d'après les données réunies dans Wikidata par l'initiative LOTUS."
        }

        TextKey::PageNotFound => "Page introuvable",
        TextKey::PageNotFoundDescription => "La page demandée n'existe pas.",
        TextKey::ReturnHome => "Retour à l'accueil",
        TextKey::ResolvedTaxon => "Taxon recherché",
        TextKey::LinkedNamesIncluded => {
            "Les enregistrements classés sous un nom lié, par exemple un synonyme ou un basionyme, sont inclus."
        }
        TextKey::DuplicateRowsHint => {
            "Pourquoi certains composés apparaissent-ils plusieurs fois ?"
        }
        TextKey::DuplicateRowsExplain => {
            "Chaque ligne est un composé tel qu'un document le rapporte : un composé trouvé par trois articles occupe trois lignes qui ne diffèrent que par la référence. Les occurrences sont distinctes, les composés non. Le nombre plus petit à côté du nombre de lignes est le nombre de composés distincts."
        }
        TextKey::QueryHash => "Hash de la requête",
        TextKey::ResultHash => "Hash du résultat",
        TextKey::CopyTaxonQid => "Copier le QID du taxon",
        TextKey::CopyFullQueryHash => "Copier le hash complet de la requête (SHA-256)",
        TextKey::CopyFullResultHash => "Copier le hash complet du résultat (SHA-256)",
        TextKey::CopyShareableLink => "Copier le lien à partager",
        TextKey::Unique => "Uniques",
        TextKey::LoadingTitle => "Interrogation de Wikidata via QLever...",
        TextKey::LoadingHint => "Les grands jeux de résultats peuvent prendre du temps.",
        TextKey::LoadingResolvingStructure => "Résolution de la structure...",
        TextKey::LoadingResolvingReference => "Résolution de la référence...",
        TextKey::LoadingResolvingTaxon => "Résolution du taxon...",
        TextKey::LoadingFetchingResults => "Récupération des résultats...",
        TextKey::LoadingRowsSoFar => "à ce jour",
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
        TextKey::ErrorHintQueryTooExpensive => {
            "Cette recherche est trop large : le service de requête l'a arrêtée à sa limite de temps. Réduisez-la avec un taxon, une plage de masse, une année ou une formule, puis relancez-la."
        }
        TextKey::ErrorHintBadRequest => {
            "Le serveur a rejeté la requête. Vérifiez les paramètres de recherche."
        }
        TextKey::ErrorHintParse => {
            "Échec de lecture de la réponse. Réessayez ou affinez la requête."
        }
        TextKey::ErrorHintTruncated => {
            "Ce jeu de résultats a été interrompu : les lignes affichées ne sont pas la réponse complète. Réduisez la recherche pour la terminer."
        }
        TextKey::ErrorHintUnknown => "Erreur inattendue. Réessayer peut aider.",

        TextKey::ExampleQueryExecute => "Exécuter",
        TextKey::ExampleQueryTaxon | TextKey::DownloadCsvLabel => "Télécharger CSV",
        TextKey::ExampleQueryStructure | TextKey::DownloadJsonLabel => "Télécharger JSON",
        TextKey::ExampleQueryAdvanced | TextKey::DownloadRdfLabel => "Télécharger RDF",
        TextKey::ExampleApiUrls => "Exemples d'URL d'API",

        TextKey::SearchExamples => "Exemples de recherche",

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
        TextKey::StructurePlaceholder => "SMILES, un Molfile, un nom ou un InChIKey",
        TextKey::Exact => {
            "Exact (une structure collée peut aussi correspondre à ses stéréo-isomères)"
        }
        TextKey::Substructure => "Sous-structure",
        TextKey::Similarity => "Similarité",
        TextKey::StructureSearchMode => "Recherche par structure",
        TextKey::ReferenceField => "QID ou DOI de référence",

        TextKey::EditCopyDaylightSmiles => "Édition -> Copier en tant que SMILES Daylight",
        TextKey::CopyExtendedSmilesMol => "Copier au format SMILES étendu / MOL V3000",

        TextKey::FormulaFilter => "Filtre de formule",
        TextKey::ExactFormula => "Formule exacte",
        TextKey::Formula => "Formule",
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
        TextKey::ResetSearchFilters => "Réinitialiser les filtres",
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
        TextKey::StageStructureSearch => "résolution de la structure",
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
        TextKey::FooterForData => " pour les données Wikidata ",
        TextKey::FooterForCode => " pour le code",
        TextKey::TableTriplesAria => "Triplets composé-taxon-référence",
        TextKey::OpenFullSizeDepiction => "Ouvrir la représentation en taille complète",
        TextKey::OpenInWikidata => "Ouvrir dans Wikidata",
        TextKey::OpenInScholia => "Ouvrir dans Scholia",
        TextKey::OpenInCompoundScholia => "Ouvrir le composé dans Scholia",
        TextKey::OpenInTaxonScholia => "Ouvrir le taxon dans Scholia",
        TextKey::OpenInReferenceScholia => "Ouvrir la référence dans Scholia",
        TextKey::OpenDoi => "Ouvrir DOI",
        TextKey::Statement => "Déclaration · Signaler",
        TextKey::SparqlQuery => "Requête SPARQL",
        TextKey::CopySparqlQuery => "Copier la requête SPARQL",
        TextKey::ShowFailedQuery => "Cette recherche n'a pas abouti — afficher la requête utilisée",
        TextKey::FailedQueryReconstructed => {
            "Reconstituée à partir de vos critères de recherche : proche de la requête envoyée plutôt qu'une copie exacte. Ajustez-la puis relancez."
        }
        TextKey::StaleResults => {
            "Vos critères de recherche ont changé. Relancez la recherche pour voir les résultats correspondants."
        }
        TextKey::ViewInNetwork => "Voir en réseau",
        TextKey::NetworkStarterHeading => "Explorer le graphe de connaissances LOTUS",
        TextKey::NetworkStarterSubtitle => {
            "Ouvrez une constellation composé–taxon–référence interactive, ou lancez une recherche rapide ci-dessous."
        }
        TextKey::NetworkQuickSearchLabel => "Recherche rapide d'entité",
        TextKey::NetworkQuickSearchPlaceholder => "Taxon, QID/SMILES de composé ou DOI/QID…",
        TextKey::NetworkQuickSearchSubmit => "Explorer le graphe",
        TextKey::NetworkQuickSearchKindLabel => "Type d'entité",
        TextKey::NetworkStarterTitleBetaSitosterol => "β-Sitostérol",
        TextKey::NetworkStarterDescBetaSitosterol => {
            "Phytostérol courant (Q121802) dans les taxons végétaux rapportés"
        }
        TextKey::NetworkStarterTitleGentianaGenus => "Gentiana (genre)",
        TextKey::NetworkStarterDescGentianaGenus => {
            "Gentianes alpines : séco-iridoïdes, xanthones et taxons"
        }
        TextKey::NetworkStarterTitleGentianaLutea => "Littérature sur Gentiana lutea",
        TextKey::NetworkStarterDescGentianaLutea => {
            "Composés de la gentiane jaune (Q158572) reliés aux articles sources"
        }
        TextKey::NetworkStarterTitleIndole => "Sous-structure indole",
        TextKey::NetworkStarterDescIndole => {
            "Composés portant un cycle indole à travers les taxons"
        }
        TextKey::OpenAdvancedSearch => "Ouvrir la recherche avancée et les filtres",
        TextKey::PreviousSearch => "Recherche précédente",
        TextKey::TableAndFilters => "Tableau / Modifier les filtres",
        TextKey::GraphProjectionLabel => "Vue du graphe",
        TextKey::ProjectionAuto => "Auto",
        TextKey::ProjectionCompoundTaxon => "Composés ↔ Taxons",
        TextKey::ProjectionCompoundReference => "Composés ↔ Références",
        TextKey::ProjectionTaxonReference => "Taxons ↔ Références",
        TextKey::ProjectionTripartite => "Les 3 types",
        TextKey::GraphNodeSearchLabel => "Chercher une entité dans le graphe",
        TextKey::GraphNodeSearchPlaceholder => "Trouver un nœud (touche /)…",
        TextKey::GraphZoomIn => "Zoom +",
        TextKey::GraphZoomOut => "Zoom −",
        TextKey::GraphResetView => "Réinitialiser la vue",
        TextKey::GraphPauseMotion => "Mettre en pause",
        TextKey::GraphResumeMotion => "Reprendre",
        TextKey::GraphDownloadPng => "Télécharger PNG",
        TextKey::GraphNodeLimitLabel => "Limite de nœuds",
        TextKey::NetworkWorkspaceRegionAria => "Espace réseau interactif LOTUS",
        TextKey::NetworkControlsNavAria => "Contrôles du graphe réseau",
        TextKey::NetworkInspectorRegionAria => "Inspecteur d'entité et voisins liés",
        TextKey::GraphCanvasAriaPrefix => "Réseau interactif composé–taxon–référence LOTUS",
        TextKey::GraphCanvasHint => {
            "Glisser pour pivoter/déplacer · Molette pour zoomer · Cliquer un nœud · Touches : + − 0 Esc"
        }
        TextKey::GraphLegendAria => "Légende des entités du graphe",
        TextKey::GraphBackToOverview => "Retour à la vue d'ensemble",
        TextKey::GraphEgoBadgePrefix => "Voisinage ciblé :",
        TextKey::GraphCappedNoticePrefix => "Affichage des",
        TextKey::GraphCappedNoticeSuffix => {
            "nœuds les plus connectés — utilisez la recherche pour cibler toute entité."
        }
        TextKey::GraphNoOccurrenceEdges => {
            "Ces résultats ne contiennent aucun lien d'occurrence taxon ou référence à projeter en réseau."
        }
        TextKey::GraphCenterNotInRows => {
            "L'entité ciblée est exclue par les filtres actuels du tableau."
        }
        TextKey::GraphWebglUnavailable => {
            "WebGL2 est indisponible ici ; parcourez les entités principales dans l'inspecteur."
        }
        TextKey::GraphPngSaved => "Capture PNG enregistrée :",
        TextKey::GraphContextLost => "Contexte WebGL2 interrompu ; restauration…",
        TextKey::TopConnectedEntities => "Entités les plus connectées",
        TextKey::FocusNeighborhood => "Cibler le voisinage",
        TextKey::FilterTableToEntity => "Filtrer le tableau sur ceci",
        TextKey::ExploreInLotus => "Explorer dans LOTUS",
        TextKey::LinkedEntities => "Entités liées",
        TextKey::CloseInspector => "Fermer l'inspecteur",
        TextKey::NodeDegreeLabel => "Liens",
        TextKey::NodeOccurrencesLabel => "Occurrences",
        TextKey::GraphCorroborationRefs => "réfs",
        TextKey::GraphCorroborationTaxa => "taxons",
        TextKey::GraphCorroborationCompounds => "composés",
        TextKey::GraphSmilesLabel => "SMILES",
        TextKey::CopySmiles => "Copier SMILES",
        TextKey::GraphActiveFilters => "Filtres du tableau actifs",
        TextKey::GraphMode3d => "3D",
        TextKey::GraphMode2d => "2D",
        TextKey::GraphViewModeAria => "Mode de dimension du graphe",
    }
}
