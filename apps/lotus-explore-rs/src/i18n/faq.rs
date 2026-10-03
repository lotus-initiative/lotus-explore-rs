// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The FAQ's questions and answers, in every locale.
//!
//! Its own module rather than four more `match` arms in `en.rs`/`fr.rs`/`de.rs`/
//! `it.rs`, for the same reason `i18n/curation` is its own directory: this is one
//! self-contained block of content that grows as a unit. Every question needs an
//! answer in every locale at once, so keeping them side by side in one file is what
//! makes a missing translation visible -- a gap is a `None` in one row rather than an
//! arm quietly absent from one of four matches in four other files.
//!
//! The answers are deliberately short. A FAQ answer that needs a paragraph is a
//! document pretending to be a FAQ, and the long-form explanations already live in
//! `docs/TAXON-SEARCH.md` and `docs/STRUCTURE-SEARCH.md`; each answer that can earns
//! a link to one instead of restating it.

use crate::i18n::Locale;

/// One question, and its answer in each locale.
///
/// `None` falls back to English rather than rendering nothing, so an untranslated
/// answer is a gap a reader can still get past instead of a blank panel.
#[derive(Clone, Copy)]
pub struct FaqEntry {
    /// Stable anchor, and the URL fragment a link to this question uses.
    ///
    /// Written out rather than derived from the question text so that re-wording a
    /// question does not break the links people have already shared.
    pub id: &'static str,
    /// The category heading this question sits under.
    pub category: FaqCategory,
    /// `(locale, question, answer)` for every locale that has one.
    pub translations: &'static [(Locale, &'static str, &'static str)],
}

impl FaqEntry {
    /// The question in `locale`, falling back to English.
    #[must_use]
    pub fn question(&self, locale: Locale) -> &'static str {
        self.pick(locale)
            .or_else(|| self.pick(Locale::En))
            .map_or("", |(question, _)| question)
    }

    /// The answer in `locale`, falling back to English.
    #[must_use]
    pub fn answer(&self, locale: Locale) -> &'static str {
        self.pick(locale)
            .or_else(|| self.pick(Locale::En))
            .map_or("", |(_, answer)| answer)
    }

    fn pick(&self, locale: Locale) -> Option<(&'static str, &'static str)> {
        self.translations
            .iter()
            .find(|(l, _, _)| *l == locale)
            .map(|(_, q, a)| (*q, *a))
    }
}

/// The groups the questions are filed under, in the order they appear.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaqCategory {
    About,
    Searching,
    Results,
    Export,
}

impl FaqCategory {
    /// Every category, in display order.
    pub const ALL: [Self; 4] = [Self::About, Self::Searching, Self::Results, Self::Export];

    /// Heading for this category in `locale`, falling back to English.
    #[must_use]
    pub fn label(self, locale: Locale) -> &'static str {
        let pick =
            |en: &'static str, fr: &'static str, de: &'static str, it: &'static str| match locale {
                Locale::En => en,
                Locale::Fr => fr,
                Locale::De => de,
                Locale::It => it,
            };
        match self {
            Self::About => pick(
                "About LOTUS",
                "À propos de LOTUS",
                "Über LOTUS",
                "Informazioni su LOTUS",
            ),
            Self::Searching => pick("Searching", "Recherche", "Suchen", "Ricerca"),
            Self::Results => pick(
                "Results and identifiers",
                "Résultats et identifiants",
                "Ergebnisse und Kennungen",
                "Risultati e identificatori",
            ),
            Self::Export => pick(
                "Export and data",
                "Export et données",
                "Export und Daten",
                "Esportazione e dati",
            ),
        }
    }
}

/// Every question, in display order.
pub const ENTRIES: &[FaqEntry] = &[
    FaqEntry {
        id: "what-is-lotus",
        category: FaqCategory::About,
        translations: &[
            (
                Locale::En,
                "What is LOTUS?",
                "LOTUS Explorer searches LOTUS, an open Linked Data graph of natural products, resolved from Wikidata.",
            ),
            (
                Locale::Fr,
                "Qu'est-ce que LOTUS ?",
                "LOTUS Explorer permet de rechercher dans LOTUS, un graphe de données ouvertes de produits naturels, résolu depuis Wikidata.",
            ),
            (
                Locale::De,
                "Was ist LOTUS?",
                "LOTUS Explorer durchsucht LOTUS, einen offenen Linked-Data-Graphen natürlicher Produkte, aufgelöst aus Wikidata.",
            ),
            (
                Locale::It,
                "Che cos'è LOTUS?",
                "LOTUS Explorer cerca in LOTUS, un grafo di dati linked open di prodotti naturali, risolto da Wikidata.",
            ),
        ],
    },
    FaqEntry {
        id: "does-anything-leave-my-browser",
        category: FaqCategory::About,
        translations: &[
            (
                Locale::En,
                "Does anything leave my browser?",
                "No. Searching, parsing and ranking all happen in this tab. Only the queries you send reach a SPARQL endpoint, and a download goes to whichever service issued the export URL.",
            ),
            (
                Locale::Fr,
                "Est-ce que des données quittent mon navigateur ?",
                "Non. La recherche, l'analyse et le tri se font dans cet onglet. Seules les requêtes envoyées atteignent un point d'accès SPARQL, et un téléchargement va vers le service qui a fourni l'URL d'export.",
            ),
            (
                Locale::De,
                "Verlässt etwas meinen Browser?",
                "Nein. Suche, Verarbeitung und Sortierung finden in diesem Tab statt. Nur die gesendeten Abfragen erreichen einen SPARQL-Endpunkt, und ein Download geht an den Dienst, der die Export-URL ausgestellt hat.",
            ),
            (
                Locale::It,
                "Qualcosa lascia il mio browser?",
                "No. Ricerca, analisi e ordinamento avvengono in questa scheda. Solo le query inviate raggiungono un endpoint SPARQL e un download va al servizio che ha emesso l'URL di esportazione.",
            ),
        ],
    },
    FaqEntry {
        id: "search-by-name",
        category: FaqCategory::Searching,
        translations: &[
            (
                Locale::En,
                "How do I search by name?",
                "Type a taxon name or a compound name in the search panel. A taxon is resolved to its Wikidata QID; a compound is matched on its label and synonyms.",
            ),
            (
                Locale::Fr,
                "Comment rechercher par nom ?",
                "Saisissez un nom de taxon ou de composé dans le panneau de recherche. Un taxon est résolu vers son QID Wikidata ; un composé est trouvé d'après son libellé et ses synonymes.",
            ),
            (
                Locale::De,
                "Wie suche ich nach Namen?",
                "Geben Sie einen Taxon- oder Verbindungsnamen in das Suchfeld ein. Ein Taxon wird auf seine Wikidata-QID aufgelöst; eine Verbindung wird über Bezeichnung und Synonyme abgeglichen.",
            ),
            (
                Locale::It,
                "Come si cerca per nome?",
                "Digita il nome di un taxon o di un composto nel pannello di ricerca. Un taxon viene risolto nel suo QID Wikidata; un composto viene abbinato per etichetta e sinonimi.",
            ),
        ],
    },
    FaqEntry {
        id: "search-by-structure",
        category: FaqCategory::Searching,
        translations: &[
            (
                Locale::En,
                "How do I search by chemical structure?",
                "Switch the search to structure mode and paste a SMILES string. It is used exactly as written: LOTUS never silently substitutes a similar structure for yours.",
            ),
            (
                Locale::Fr,
                "Comment rechercher par structure chimique ?",
                "Passez la recherche en mode structure et collez une chaîne SMILES. Elle est utilisée telle quelle : LOTUS ne remplace jamais silencieusement votre structure par une structure proche.",
            ),
            (
                Locale::De,
                "Wie suche ich nach chemischer Struktur?",
                "Wechseln Sie die Suche in den Strukturmodus und fügen Sie einen SMILES-String ein. Er wird genau so verwendet: LOTUS ersetzt nie stillschweigend Ihre Struktur durch eine ähnliche.",
            ),
            (
                Locale::It,
                "Come si cerca per struttura chimica?",
                "Passa la ricerca in modalità struttura e incolla una stringa SMILES. Viene usata esattamente come scritta: LOTUS non sostituisce mai silenziosamente la tua struttura con una simile.",
            ),
        ],
    },
    FaqEntry {
        id: "taxon-not-found",
        category: FaqCategory::Searching,
        translations: &[
            (
                Locale::En,
                "What does it mean when a taxon is not found?",
                "The name matched neither a scientific nor a common name in Wikidata. A bare QID is accepted directly and skips the lookup entirely.",
            ),
            (
                Locale::Fr,
                "Que signifie « taxon introuvable » ?",
                "Le nom ne correspondait à aucun nom scientifique ni commun dans Wikidata. Un simple QID est accepté directement et évite entièrement la recherche.",
            ),
            (
                Locale::De,
                "Was bedeutet „Taxon nicht gefunden“?",
                "Der Name traf weder einen wissenschaftlichen noch einen gebräuchlichen Namen in Wikidata. Eine reine QID wird direkt akzeptiert und überspringt die Suche ganz.",
            ),
            (
                Locale::It,
                "Cosa significa « taxon non trovato »?",
                "Il nome non corrispondeva né a un nome scientifico né a un nome comune in Wikidata. Un QID nudo viene accettato direttamente e salta del tutto la ricerca.",
            ),
        ],
    },
    FaqEntry {
        id: "what-is-a-qid",
        category: FaqCategory::Results,
        translations: &[
            (
                Locale::En,
                "What is a QID?",
                "A Wikidata identifier: `Q` followed by a number, naming one entity unambiguously. It is the join key between your query and everything else on the graph.",
            ),
            (
                Locale::Fr,
                "Qu'est-ce qu'un QID ?",
                "Un identifiant Wikidata : `Q` suivi d'un nombre, nommant une entité sans ambiguïté. C'est la clé de jointure entre votre requête et le reste du graphe.",
            ),
            (
                Locale::De,
                "Was ist eine QID?",
                "Ein Wikidata-Kennzeichen: ein `Q` gefolgt von einer Zahl, das eine Entität eindeutig benennt. Sie ist der Verknüpfungsschlüssel zwischen Ihrer Abfrage und dem Rest des Graphen.",
            ),
            (
                Locale::It,
                "Cos'è un QID?",
                "Un identificatore Wikidata: `Q` seguito da un numero, che nomina una singola entità senza ambiguità. È la chiave di join tra la tua query e il resto del grafo.",
            ),
        ],
    },
    FaqEntry {
        id: "how-many-rows",
        category: FaqCategory::Results,
        translations: &[
            (
                Locale::En,
                "How many rows can a search return?",
                "Every row that matches, not a sample. Counts come from the same result set the table shows, so the number in the header is the number of rows you can scroll through.",
            ),
            (
                Locale::Fr,
                "Combien de lignes une recherche peut-elle renvoyer ?",
                "Toutes les lignes correspondantes, pas un échantillon. Les compteurs viennent du même jeu de résultats que le tableau, donc le nombre affiché est celui que vous pouvez faire défiler.",
            ),
            (
                Locale::De,
                "Wie viele Zeilen kann eine Suche liefern?",
                "Alle passenden Zeilen, keine Stichprobe. Die Zähler stammen aus derselben Ergebnismenge wie die Tabelle, die Zahl in der Kopfzeile ist also die Zahl der Zeilen, die Sie durchscrollen können.",
            ),
            (
                Locale::It,
                "Quante righe può restituire una ricerca?",
                "Tutte le righe corrispondenti, non un campione. I conteggi derivano dallo stesso insieme di risultati della tabella, quindi il numero nell'intestazione è quello delle righe che puoi scorrere.",
            ),
        ],
    },
    FaqEntry {
        id: "filters-vs-query",
        category: FaqCategory::Results,
        translations: &[
            (
                Locale::En,
                "Do the filters change the download?",
                "No. Filters and sorting apply to the table on screen. A download exports the query's result set, because the export is produced by the service that ran the query, not by this tab.",
            ),
            (
                Locale::Fr,
                "Les filtres modifient-ils le téléchargement ?",
                "Non. Les filtres et le tri s'appliquent au tableau affiché. Un téléchargement exporte le jeu de résultats de la requête, car l'export est produit par le service qui a exécuté la requête, pas par cet onglet.",
            ),
            (
                Locale::De,
                "Verändern die Filter den Download?",
                "Nein. Filter und Sortierung gelten für die Tabelle auf dem Bildschirm. Ein Download exportiert die Ergebnismenge der Abfrage, weil der Export vom Dienst erzeugt wird, der die Abfrage ausgeführt hat, nicht von diesem Tab.",
            ),
            (
                Locale::It,
                "I filtri modificano il download?",
                "No. Filtri e ordinamento si applicano alla tabella a schermo. Un download esporta l'insieme di risultati della query, perché l'esportazione è prodotta dal servizio che ha eseguito la query, non da questa scheda.",
            ),
        ],
    },
    FaqEntry {
        id: "download-formats",
        category: FaqCategory::Export,
        translations: &[
            (
                Locale::En,
                "Which formats can I download?",
                "CSV, JSON and RDF/Turtle. CSV is the compact, lossless default for bulk work; RDF is a set of triples for loading into a triple store.",
            ),
            (
                Locale::Fr,
                "Quels formats puis-je télécharger ?",
                "CSV, JSON et RDF/Turtle. Le CSV est le format compact et sans perte par défaut ; le RDF est un ensemble de triplets à charger dans un magasin de triplets.",
            ),
            (
                Locale::De,
                "Welche Formate kann ich herunterladen?",
                "CSV, JSON und RDF/Turtle. CSV ist das kompakte, verlustfreie Format für Massenabzüge; RDF ist eine Tripelmenge zum Laden in einen Tripel-Speicher.",
            ),
            (
                Locale::It,
                "Quali formati posso scaricare?",
                "CSV, JSON e RDF/Turtle. Il CSV è il formato compatto e senza perdita per il lavoro di massa; l'RDF è un insieme di terne da caricare in un triple store.",
            ),
        ],
    },
    FaqEntry {
        id: "where-downloads-come-from",
        category: FaqCategory::Export,
        translations: &[
            (
                Locale::En,
                "Where does a download come from?",
                "The API produces the file and this tab hands you its URL, so a large export never passes through the browser's memory. If no API is configured, the query is sent to a public SPARQL endpoint instead.",
            ),
            (
                Locale::Fr,
                "D'où vient un téléchargement ?",
                "L'API produit le fichier et cet onglet vous en transmet l'URL, si bien qu'un gros export ne transite jamais par la mémoire du navigateur. Si aucune API n'est configurée, la requête est envoyée à un point d'accès SPARQL public.",
            ),
            (
                Locale::De,
                "Woher kommt ein Download?",
                "Die API erzeugt die Datei und dieser Tab übergibt Ihnen ihre URL, sodass ein großer Export nie durch den Browserspeicher läuft. Ist keine API konfiguriert, geht die Abfrage stattdessen an einen öffentlichen SPARQL-Endpunkt.",
            ),
            (
                Locale::It,
                "Da dove proviene un download?",
                "L'API produce il file e questa scheda ti passa l'URL, così un export di grandi dimensioni non attraversa mai la memoria del browser. Se nessuna API è configurata, la query viene inviata a un endpoint SPARQL pubblico.",
            ),
        ],
    },
    FaqEntry {
        id: "shareable-link",
        category: FaqCategory::Export,
        translations: &[
            (
                Locale::En,
                "Can I share a search?",
                "Yes. The shareable link carries the filters, and the two hash buttons give you the query and the result set as identifiers you can quote and compare.",
            ),
            (
                Locale::Fr,
                "Puis-je partager une recherche ?",
                "Oui. Le lien partageable contient les filtres, et les deux boutons de hachage donnent la requête et le jeu de résultats sous forme d'identifiants citables et comparables.",
            ),
            (
                Locale::De,
                "Kann ich eine Suche teilen?",
                "Ja. Der teilbare Link enthält die Filter, und die beiden Hash-Schaltflächen liefern die Abfrage und die Ergebnismenge als Kennungen, die Sie zitieren und vergleichen können.",
            ),
            (
                Locale::It,
                "Posso condividere una ricerca?",
                "Sì. Il link condivisibile contiene i filtri e i due pulsanti di hash restituiscono la query e l'insieme di risultati come identificatori che puoi citare e confrontare.",
            ),
        ],
    },
    FaqEntry {
        id: "tsv-upload",
        category: FaqCategory::Export,
        translations: &[
            (
                Locale::En,
                "Can I search my own list of names?",
                "Yes. A TSV file of names can be uploaded and resolved in one pass, rather than searching them one at a time.",
            ),
            (
                Locale::Fr,
                "Puis-je rechercher ma propre liste de noms ?",
                "Oui. Un fichier TSV de noms peut être téléversé et résolu en une seule passe, au lieu de les rechercher un par un.",
            ),
            (
                Locale::De,
                "Kann ich meine eigene Namensliste durchsuchen?",
                "Ja. Eine TSV-Datei mit Namen kann hochgeladen und in einem Durchgang aufgelöst werden, statt sie einzeln zu suchen.",
            ),
            (
                Locale::It,
                "Posso cercare il mio elenco di nomi?",
                "Sì. Un file TSV di nomi può essere caricato e risolto in una sola passata, invece di cercarli uno alla volta.",
            ),
        ],
    },
];

/// The FAQ's label in the view switcher.
///
/// Here rather than beside `view_label_explorer` and friends, which are curation
/// strings dispatched across four locale files, because this is one word and the
/// content it names already lives in this module.
///
/// One string, not four: "FAQ" is an initialism and is written the same way in every
/// language this app ships. A `match` here would be four identical arms, which is
/// what the lint is objecting to and what a reader would have to check anyway.
#[must_use]
pub const fn faq_nav_label(_locale: Locale) -> &'static str {
    "FAQ"
}

/// The `FAQPage` structured data for these questions.
///
/// Hand-built rather than derived, because the only consumer that matters reads the
/// shape rather than the string: a `FAQPage` with one `Question` per entry, each with
/// a non-empty `Answer`. Emitting it is what makes the answers findable by a search
/// engine and legible to an agent, which is the same reason the result set emits
/// dataset markup.
#[must_use]
pub fn faq_json_ld(locale: Locale) -> String {
    use std::fmt::Write as _;

    let mut json =
        String::from("{\"@context\":\"https://schema.org\",\"@type\":\"FAQPage\",\"mainEntity\":[");
    for (index, entry) in ENTRIES.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let _ = write!(
            json,
            "{{\"@type\":\"Question\",\"@id\":\"#{}\",\"name\":{},\"acceptedAnswer\":{{\"@type\":\"Answer\",\"text\":{}}}}}",
            entry.id,
            json_string(entry.question(locale)),
            json_string(entry.answer(locale))
        );
    }
    json.push_str("]}");
    json
}

/// A JSON string literal.
///
/// Hand-rolled to keep `serde_json` out of the render path for a page that is mostly
/// static text. Escapes the two characters that matter: a quote would end the string,
/// and `</` would end the `<script>` element the result is embedded in.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::{ENTRIES, FaqCategory, FaqEntry};
    use crate::i18n::Locale;

    const LOCALES: [Locale; 4] = [Locale::En, Locale::Fr, Locale::De, Locale::It];

    /// A string field of a JSON value, or `None` if absent or not a string.
    ///
    /// A function rather than `value["key"]` because the workspace denies
    /// `indexing_slicing`, which is the right default for a test that parses a string
    /// this module also built: an index that returns `Null` would fail the assertion
    /// anyway, but noisily.
    fn text<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
        value.get(key).and_then(serde_json::Value::as_str)
    }

    #[test]
    fn every_question_has_an_answer_in_every_locale() {
        for entry in ENTRIES {
            for locale in LOCALES {
                assert!(
                    !entry.question(locale).is_empty(),
                    "{} has no question in {locale:?}",
                    entry.id
                );
                assert!(
                    !entry.answer(locale).is_empty(),
                    "{} has no answer in {locale:?}",
                    entry.id
                );
            }
        }
    }

    #[test]
    fn every_locale_gives_a_distinct_translation() {
        // A locale that silently fell back to English would look fine in the app and
        // be wrong, which is the failure this catches: four rows of English presented
        // as four locales.
        for entry in ENTRIES {
            let english = entry.question(Locale::En);
            for locale in [Locale::Fr, Locale::De, Locale::It] {
                assert_ne!(
                    entry.question(locale),
                    english,
                    "{} is untranslated for {locale:?}",
                    entry.id
                );
            }
        }
    }

    #[test]
    fn question_anchors_are_unique_and_usable_in_a_url() {
        let mut seen: Vec<&str> = Vec::new();
        for entry in ENTRIES {
            assert!(
                !seen.contains(&entry.id),
                "duplicate anchor {} would make one question unlinkable",
                entry.id
            );
            assert!(
                entry
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{} is not a clean URL fragment",
                entry.id
            );
            seen.push(entry.id);
        }
    }

    #[test]
    fn every_question_belongs_to_a_category_that_is_shown() {
        for entry in ENTRIES {
            assert!(
                FaqCategory::ALL.contains(&entry.category),
                "{} is filed under a category the page never renders",
                entry.id
            );
        }
    }

    #[test]
    fn the_json_ld_is_valid_and_covers_every_question() {
        // Built by hand rather than through `serde_json`, so this checks the shape a
        // consumer actually parses: a `FAQPage` whose `mainEntity` is a Question per
        // entry, each with a non-empty `Answer`.
        let json = super::faq_json_ld(Locale::En);
        // Parsed rather than string-matched, because the shape is the contract: a
        // consumer reads `@type` and `acceptedAnswer`, so a JSON blob that merely
        // contains the right words in the wrong place is not a pass.
        let typed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(text(&typed, "@type"), Some("FAQPage"));
        let entities = typed
            .get("mainEntity")
            .and_then(serde_json::Value::as_array)
            .map_or(&[][..], std::vec::Vec::as_slice);
        assert_eq!(entities.len(), ENTRIES.len(), "one entity per question");

        // The `@id` is what makes a question addressable from outside the page, so it
        // has to be the same fragment the heading renders.
        for (entry, entity) in ENTRIES.iter().zip(entities) {
            assert_eq!(
                text(entity, "@id"),
                Some(format!("#{}", entry.id).as_str()),
                "the @id must be the same fragment the heading renders"
            );
            assert_eq!(text(entity, "@type"), Some("Question"));

            let answer = entity.get("acceptedAnswer");
            let answer_type = answer.and_then(|answer| text(answer, "@type"));
            assert_eq!(answer_type, Some("Answer"));
            assert!(
                answer
                    .and_then(|answer| text(answer, "text"))
                    .is_some_and(|text| !text.is_empty()),
                "{} has an empty answer in the structured data",
                entry.id
            );
        }
    }

    #[test]
    fn answers_are_short_enough_to_be_answers() {
        // A FAQ answer that runs long is documentation wearing a FAQ costume. The
        // long-form text already exists in `docs/`, linked from the answers that need
        // it, so anything past this is a sign of restating a document here.
        for entry in ENTRIES {
            assert!(
                entry.answer(Locale::En).chars().count() < 400,
                "{} is {} characters: that belongs in docs/, not an answer",
                entry.id,
                entry.answer(Locale::En).chars().count()
            );
        }
    }

    #[test]
    fn entries_declare_the_locales_they_carry() {
        for entry in ENTRIES {
            let declared: Vec<Locale> = entry.translations.iter().map(|(l, _, _)| *l).collect();
            for locale in LOCALES {
                assert!(
                    declared.contains(&locale),
                    "{} does not declare {locale:?}",
                    entry.id
                );
            }
            let _: &FaqEntry = entry;
        }
    }
}
