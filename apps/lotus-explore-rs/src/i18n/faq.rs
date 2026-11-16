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
                "LOTUS is an open collection of natural product occurrences: which compound was reported from which organism, in which article. The LOTUS initiative curates these records and stores them in Wikidata, where anyone can read, correct and add to them. The LOTUS Explorer is a search tool for them.",
            ),
            (
                Locale::Fr,
                "Qu'est-ce que LOTUS ?",
                "LOTUS est une collection ouverte d'occurrences de produits naturels : quel composé a été rapporté dans quel organisme, et dans quel article. L'initiative LOTUS vérifie ces données et les enregistre dans Wikidata, où chacun peut les lire, les corriger et les compléter. LOTUS Explorer est un outil de recherche pour ces données.",
            ),
            (
                Locale::De,
                "Was ist LOTUS?",
                "LOTUS ist eine offene Sammlung von Naturstoff-Vorkommen: welche Verbindung in welchem Organismus beschrieben wurde und in welchem Artikel. Die LOTUS-Initiative prüft diese Daten und legt sie in Wikidata ab, wo jeder sie lesen, korrigieren und ergänzen kann. Der LOTUS Explorer ist ein Suchwerkzeug für diese Daten.",
            ),
            (
                Locale::It,
                "Che cos'è LOTUS?",
                "LOTUS è una raccolta aperta di occorrenze di sostanze naturali: quale composto è stato riportato in quale organismo, e in quale articolo. L'iniziativa LOTUS verifica questi dati e li registra in Wikidata, dove chiunque può leggerli, correggerli e completarli. LOTUS Explorer è uno strumento di ricerca per questi dati.",
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
                "Yes: what you search for. Each search is sent to a public Wikidata query service (QLever, or the Wikidata Query Service when QLever cannot be reached); a structure search is passed on to the IDSM structure service. Structure images come from the CDK Depict service, which receives the SMILES of the compounds shown. Simple Analytics counts page visits. Filtering and sorting happen in this tab.",
            ),
            (
                Locale::Fr,
                "Est-ce que des données quittent mon navigateur ?",
                "Oui : ce que vous recherchez. Chaque recherche est envoyée à un service de requête public de Wikidata (QLever, ou le Wikidata Query Service si QLever est injoignable) ; une recherche par structure est transmise au service de structures IDSM. Les images des structures viennent du service CDK Depict, qui reçoit les SMILES des composés affichés. Simple Analytics compte les visites. Les filtres et le tri se font dans cet onglet.",
            ),
            (
                Locale::De,
                "Verlässt etwas meinen Browser?",
                "Ja: das, wonach Sie suchen. Jede Suche geht an einen öffentlichen Wikidata-Abfragedienst (QLever oder, wenn QLever nicht erreichbar ist, den Wikidata Query Service); eine Struktursuche wird an den Strukturdienst IDSM weitergegeben. Die Strukturbilder stammen vom Dienst CDK Depict, der die SMILES der angezeigten Verbindungen erhält. Simple Analytics zählt die Seitenaufrufe. Filtern und Sortieren geschehen in diesem Tab.",
            ),
            (
                Locale::It,
                "Qualcosa lascia il mio browser?",
                "Sì: ciò che cerchi. Ogni ricerca viene inviata a un servizio pubblico di interrogazione di Wikidata (QLever, oppure il Wikidata Query Service se QLever non è raggiungibile); una ricerca per struttura viene trasmessa al servizio di strutture IDSM. Le immagini delle strutture provengono dal servizio CDK Depict, che riceve gli SMILES dei composti mostrati. Simple Analytics conta le visite. Filtri e ordinamento avvengono in questa scheda.",
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
                "Type an organism name or a compound name in the search form. Both are looked up in Wikidata: an organism by its scientific or common name, a compound by its name and synonyms.",
            ),
            (
                Locale::Fr,
                "Comment rechercher par nom ?",
                "Saisissez un nom d'organisme ou de composé dans le formulaire de recherche. Les deux sont recherchés dans Wikidata : un organisme par son nom scientifique ou commun, un composé par son nom et ses synonymes.",
            ),
            (
                Locale::De,
                "Wie suche ich nach Namen?",
                "Geben Sie einen Organismen- oder Verbindungsnamen in das Suchformular ein. Beide werden in Wikidata nachgeschlagen: ein Organismus über seinen wissenschaftlichen oder gebräuchlichen Namen, eine Verbindung über ihren Namen und ihre Synonyme.",
            ),
            (
                Locale::It,
                "Come si cerca per nome?",
                "Digita il nome di un organismo o di un composto nel modulo di ricerca. Entrambi vengono cercati in Wikidata: un organismo per nome scientifico o comune, un composto per nome e sinonimi.",
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
                "Paste a SMILES or a Molfile into the compound field. Exact returns the compounds the IDSM structure service scores as identical to your structure; this can include stereoisomers and other close isomers. Substructure returns compounds that contain it. Similarity returns compounds at or above the Tanimoto score you set.",
            ),
            (
                Locale::Fr,
                "Comment rechercher par structure chimique ?",
                "Collez un SMILES ou un Molfile dans le champ composé. Exact renvoie les composés que le service de structures IDSM juge identiques à votre structure ; cela peut inclure des stéréo-isomères et d'autres isomères proches. Sous-structure renvoie les composés qui la contiennent. Similarité renvoie les composés dont le score de Tanimoto atteint le seuil choisi.",
            ),
            (
                Locale::De,
                "Wie suche ich nach chemischer Struktur?",
                "Fügen Sie ein SMILES oder ein Molfile in das Verbindungsfeld ein. Exakt liefert die Verbindungen, die der Strukturdienst IDSM als identisch mit Ihrer Struktur bewertet; dazu können Stereoisomere und andere nahe Isomere gehören. Substruktur liefert Verbindungen, die sie enthalten. Ähnlichkeit liefert Verbindungen, deren Tanimoto-Wert den gewählten Schwellenwert erreicht.",
            ),
            (
                Locale::It,
                "Come si cerca per struttura chimica?",
                "Incolla uno SMILES o un Molfile nel campo del composto. Esatto restituisce i composti che il servizio di strutture IDSM valuta identici alla tua struttura; possono esserci stereoisomeri e altri isomeri vicini. Sottostruttura restituisce i composti che la contengono. Somiglianza restituisce i composti il cui punteggio di Tanimoto raggiunge la soglia scelta.",
            ),
        ],
    },
    FaqEntry {
        id: "structure-only",
        category: FaqCategory::Searching,
        translations: &[
            (
                Locale::En,
                "What happens if I give only a structure?",
                "The structure is matched on its own, with no taxon filter, and occurrences are optional. That is deliberate: a compound nobody has recorded an organism for is still returned, which is how a compound with no occurrence data gets found at all. Add a taxon and the filter applies, and every row is then an occurrence in that taxon.",
            ),
            (
                Locale::Fr,
                "Que se passe-t-il si je donne seulement une structure ?",
                "La structure est recherchée seule, sans filtre de taxon, et les occurrences sont facultatives. C'est délibéré : un composé auquel aucun organisme n'a été associé est quand même renvoyé, ce qui est justement la façon de trouver un composé sans aucune donnée d'occurrence. Ajoutez un taxon et le filtre s'applique ; chaque ligne est alors une occurrence dans ce taxon.",
            ),
            (
                Locale::De,
                "Was passiert, wenn ich nur eine Struktur angebe?",
                "Die Struktur wird für sich allein gesucht, ohne Taxon-Filter, und Vorkommen sind optional. Das ist beabsichtigt: Eine Verbindung, der kein Organismus zugeordnet wurde, wird trotzdem zurückgegeben – so findet man eine Verbindung ganz ohne Vorkommensdaten. Geben Sie zusätzlich ein Taxon an, greift der Filter und jede Zeile ist dann ein Vorkommen in diesem Taxon.",
            ),
            (
                Locale::It,
                "Cosa succede se indico solo una struttura?",
                "La struttura viene cercata da sola, senza filtro sul taxon, e le occorrenze sono facoltative. È intenzionale: un composto a cui non è stato associato alcun organismo viene comunque restituito, ed è così che si trovano i composti privi di dati di occorrenza. Aggiungi un taxon e il filtro si applica: ogni riga è allora un'occorrenza in quel taxon.",
            ),
        ],
    },
    FaqEntry {
        id: "wildcard-taxon",
        category: FaqCategory::Searching,
        translations: &[
            (
                Locale::En,
                "What does * mean in the taxon box?",
                "* asks for everything that has a recorded occurrence, across all taxa. It is not the same as leaving the box empty: an empty box also returns the compounds nobody has tied to an organism, and * leaves those out.",
            ),
            (
                Locale::Fr,
                "Que signifie * dans le champ taxon ?",
                "* demande tout ce qui possède une occurrence enregistrée, dans tous les taxons. Ce n'est pas la même chose que de laisser le champ vide : un champ vide renvoie aussi les composés auxquels aucun organisme n'a été associé, et * les exclut.",
            ),
            (
                Locale::De,
                "Was bedeutet * im Taxon-Feld?",
                "* fragt alles ab, was ein erfasstes Vorkommen hat, über alle Taxa hinweg. Das ist nicht dasselbe wie ein leeres Feld: Ein leeres Feld liefert auch die Stoffe, denen kein Organismus zugeordnet wurde, und * lässt sie weg.",
            ),
            (
                Locale::It,
                "Che cosa significa * nel campo taxon?",
                "* chiede tutto ciò che ha un'occorrenza registrata, in tutti i taxa. Non è la stessa cosa di lasciare il campo vuoto: un campo vuoto restituisce anche i composti a cui non è stato associato alcun organismo, mentre * li esclude.",
            ),
        ],
    },
    FaqEntry {
        id: "empty-taxon-box",
        category: FaqCategory::Searching,
        translations: &[
            (
                Locale::En,
                "Why does an empty taxon box return so much?",
                "An empty box sets no condition, so the search asks for every compound in LOTUS, including those with no organism recorded. That is a very large table and takes time to load. Add a taxon, a structure or a reference to narrow it.",
            ),
            (
                Locale::Fr,
                "Pourquoi un champ taxon vide renvoie-t-il autant de résultats ?",
                "Un champ vide n'impose aucune condition : la recherche demande donc tous les composés de LOTUS, y compris ceux sans organisme enregistré. C'est un très grand tableau, long à charger. Ajoutez un taxon, une structure ou une référence pour restreindre la recherche.",
            ),
            (
                Locale::De,
                "Warum liefert ein leeres Taxon-Feld so viele Ergebnisse?",
                "Ein leeres Feld setzt keine Bedingung, also fragt die Suche jede Verbindung in LOTUS ab, auch die ohne zugeordneten Organismus. Das ist eine sehr große Tabelle, deren Laden Zeit braucht. Geben Sie ein Taxon, eine Struktur oder eine Referenz an, um die Suche einzugrenzen.",
            ),
            (
                Locale::It,
                "Perché un campo taxon vuoto restituisce così tanto?",
                "Un campo vuoto non pone alcuna condizione, quindi la ricerca chiede tutti i composti di LOTUS, compresi quelli senza organismo registrato. È una tabella molto grande, che richiede tempo per essere caricata. Aggiungi un taxon, una struttura o un riferimento per restringere la ricerca.",
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
                "The name matched neither a scientific nor a common name in Wikidata. A QID on its own is accepted directly, without a name lookup.",
            ),
            (
                Locale::Fr,
                "Que signifie « taxon introuvable » ?",
                "Le nom ne correspondait à aucun nom scientifique ni commun dans Wikidata. Un QID seul est accepté directement, sans recherche du nom.",
            ),
            (
                Locale::De,
                "Was bedeutet „Taxon nicht gefunden“?",
                "Der Name traf weder einen wissenschaftlichen noch einen gebräuchlichen Namen in Wikidata. Eine QID allein wird direkt übernommen, ohne Namenssuche.",
            ),
            (
                Locale::It,
                "Cosa significa « taxon non trovato »?",
                "Il nome non corrispondeva né a un nome scientifico né a un nome comune in Wikidata. Un QID da solo viene accettato direttamente, senza ricerca del nome.",
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
                "A Wikidata identifier: the letter Q followed by a number, naming one item without ambiguity: a compound, an organism or an article. Each row links to the Wikidata items it is built from.",
            ),
            (
                Locale::Fr,
                "Qu'est-ce qu'un QID ?",
                "Un identifiant Wikidata : la lettre Q suivie d'un nombre, qui désigne un élément sans ambiguïté : un composé, un organisme ou un article. Chaque ligne renvoie aux éléments Wikidata dont elle est tirée.",
            ),
            (
                Locale::De,
                "Was ist eine QID?",
                "Eine Wikidata-Kennung: der Buchstabe Q, gefolgt von einer Zahl. Sie bezeichnet genau ein Objekt: eine Verbindung, einen Organismus oder einen Artikel. Jede Zeile verweist auf die Wikidata-Objekte, aus denen sie stammt.",
            ),
            (
                Locale::It,
                "Cos'è un QID?",
                "Un identificatore Wikidata: la lettera Q seguita da un numero, che indica un solo elemento senza ambiguità: un composto, un organismo o un articolo. Ogni riga rimanda agli elementi Wikidata da cui è tratta.",
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
                "Every row that matches, as long as the query service can send them within its time limit. When it cannot, a message says the result is incomplete. The counts above the table are computed from the rows in the table.",
            ),
            (
                Locale::Fr,
                "Combien de lignes une recherche peut-elle renvoyer ?",
                "Toutes les lignes correspondantes, tant que le service de requête peut les envoyer dans son délai. Sinon, un message indique que le résultat est incomplet. Les compteurs au-dessus du tableau sont calculés à partir des lignes du tableau.",
            ),
            (
                Locale::De,
                "Wie viele Zeilen kann eine Suche liefern?",
                "Alle passenden Zeilen, sofern der Abfragedienst sie innerhalb seines Zeitlimits senden kann. Andernfalls weist eine Meldung darauf hin, dass das Ergebnis unvollständig ist. Die Zähler über der Tabelle werden aus den Zeilen der Tabelle berechnet.",
            ),
            (
                Locale::It,
                "Quante righe può restituire una ricerca?",
                "Tutte le righe corrispondenti, purché il servizio di interrogazione riesca a inviarle entro il suo limite di tempo. In caso contrario, un messaggio segnala che il risultato è incompleto. I conteggi sopra la tabella sono calcolati dalle righe della tabella.",
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
                "No. Filters and sorting change what the table shows. A download contains every row the search returned.",
            ),
            (
                Locale::Fr,
                "Les filtres modifient-ils le téléchargement ?",
                "Non. Les filtres et le tri modifient ce que le tableau affiche. Un téléchargement contient toutes les lignes renvoyées par la recherche.",
            ),
            (
                Locale::De,
                "Verändern die Filter den Download?",
                "Nein. Filter und Sortierung ändern, was die Tabelle anzeigt. Ein Download enthält alle Zeilen, die die Suche geliefert hat.",
            ),
            (
                Locale::It,
                "I filtri modificano il download?",
                "No. Filtri e ordinamento cambiano ciò che la tabella mostra. Un download contiene tutte le righe restituite dalla ricerca.",
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
                "CSV, JSON and RDF/Turtle. CSV is compact and keeps every column; RDF is a set of triples for loading into a triple store.",
            ),
            (
                Locale::Fr,
                "Quels formats puis-je télécharger ?",
                "CSV, JSON et RDF/Turtle. Le CSV est compact et conserve toutes les colonnes ; le RDF est un ensemble de triplets à charger dans un triplestore.",
            ),
            (
                Locale::De,
                "Welche Formate kann ich herunterladen?",
                "CSV, JSON und RDF/Turtle. CSV ist kompakt, enthält alle Spalten und eignet sich für große Datenmengen; RDF ist eine Menge von Tripeln zum Laden in einen Triplestore.",
            ),
            (
                Locale::It,
                "Quali formati posso scaricare?",
                "CSV, JSON e RDF/Turtle. Il CSV è compatto e conserva tutte le colonne; l'RDF è un insieme di triple da caricare in un triplestore.",
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
        id: "cli-export",
        category: FaqCategory::Export,
        translations: &[
            (
                Locale::En,
                "What does the lotus export command do?",
                "The lotus command-line tool writes the full LOTUS compound–taxon–reference table as frozen.csv and frozen_metadata.csv into a directory you pick. No filters: it exports everything it can fetch. The table is assembled in memory before it is written, so run it on a machine with room for it.",
            ),
            (
                Locale::Fr,
                "Que fait la commande lotus export ?",
                "L'outil en ligne de commande lotus écrit le tableau complet composé–taxon–référence de LOTUS dans frozen.csv et frozen_metadata.csv, dans un répertoire que vous choisissez. Sans filtres : il exporte tout ce qu'il peut récupérer. Le tableau est assemblé en mémoire avant d'être écrit : lancez la commande sur une machine qui a la place nécessaire.",
            ),
            (
                Locale::De,
                "Was macht der Befehl lotus export?",
                "Das Kommandozeilenwerkzeug lotus schreibt die vollständige LOTUS-Tabelle Verbindung–Taxon–Referenz als frozen.csv und frozen_metadata.csv in ein Verzeichnis Ihrer Wahl. Ohne Filter: Es exportiert alles, was es abrufen kann. Die Tabelle wird im Arbeitsspeicher zusammengestellt, bevor sie geschrieben wird; der Rechner braucht dafür genügend Speicher.",
            ),
            (
                Locale::It,
                "Cosa fa il comando lotus export?",
                "Lo strumento da riga di comando lotus scrive la tabella completa composto–taxon–riferimento di LOTUS in frozen.csv e frozen_metadata.csv, in una directory a tua scelta. Senza filtri: esporta tutto ciò che riesce a recuperare. La tabella viene composta in memoria prima di essere scritta: usa una macchina con spazio sufficiente.",
            ),
        ],
    },
    // NOTE: `lotus export-pubchem-tree` and `lotus archive` are CLI-only tools
    // that fetch the entire LOTUS dataset — they are documented in docs/cli.md
    // and never surfaced in the website's FAQ.
];

/// The page's own words: the heading, the standfirst, and the two labels the contents
/// list needs.
///
/// [`ENTRIES`] carries a question and an answer per locale; this is the furniture
/// around them, one string each rather than a row per entry. It was hardcoded in the
/// component, so the whole page read as English in the other three locales while every
/// answer underneath it was translated — worse than being untranslated throughout.
pub struct FaqChrome {
    /// The page heading.
    pub heading: &'static str,
    /// One sentence under the heading.
    pub intro: &'static str,
    /// `aria-label` on the contents nav.
    pub contents_label: &'static str,
    /// Heading above the long-form reference.
    pub reference_heading: &'static str,
    /// One line introducing it.
    pub reference_intro: &'static str,
    /// The contents list's own heading.
    pub contents_heading: &'static str,
}

/// The furniture around the questions, per locale.
#[must_use]
pub fn faq_chrome(locale: Locale) -> FaqChrome {
    match locale {
        Locale::En => FaqChrome {
            heading: "Frequently asked questions",
            intro: "What this searches, what the identifiers mean, and what comes out of an export.",
            contents_label: "Sections",
            contents_heading: "On this page",
            reference_heading: "How it works",
            reference_intro: "Why the tool answers the way it does. Open a section for the detail.",
        },
        Locale::Fr => FaqChrome {
            heading: "Questions fréquentes",
            intro: "Ce que recherche cet outil, ce que signifient les identifiants, et ce que contient un export.",
            contents_label: "Sections",
            contents_heading: "Sur cette page",
            reference_heading: "Comment ça marche",
            reference_intro: "Pourquoi l’outil répond comme il le fait. Ouvrez une section pour le détail.",
        },
        Locale::De => FaqChrome {
            heading: "Häufige Fragen",
            intro: "Wonach hier gesucht wird, was die Kennungen bedeuten und was ein Export enthält.",
            contents_label: "Abschnitte",
            contents_heading: "Auf dieser Seite",
            reference_heading: "So funktioniert es",
            reference_intro: "Warum das Werkzeug so antwortet, wie es antwortet. Abschnitt für Details öffnen.",
        },
        Locale::It => FaqChrome {
            heading: "Domande frequenti",
            intro: "Che cosa cerca questo strumento, cosa significano gli identificatori e cosa contiene un'esportazione.",
            contents_label: "Sezioni",
            contents_heading: "In questa pagina",
            reference_heading: "Come funziona",
            reference_intro: "Perché lo strumento risponde in questo modo. Apri una sezione per i dettagli.",
        },
    }
}

/// The FAQ's label in the view switcher.
#[must_use]
pub const fn faq_nav_label(_locale: Locale) -> &'static str {
    "FAQ"
}

/// The `FAQPage` structured data for these questions.
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
#[path = "faq/tests.rs"]
mod tests;
