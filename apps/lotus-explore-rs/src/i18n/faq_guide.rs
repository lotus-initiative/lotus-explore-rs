// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The long-form reference behind the short answers: how a name becomes an identity.
//!
//! # Why this is separate from the questions
//!
//! The [`super`] Q&A answers "what does this do". This answers "why does it do that",
//! and the second question is the one a reader actually has when a result surprises
//! them — why a synonym returned a compound, why a genus returned nothing, why
//! `ATP` was refused. A FAQ that only states the behaviour cannot answer any of those,
//! and a reader who cannot resolve a surprise stops trusting the number.
//!
//! # Why the prose is English and the headings are not
//!
//! Every heading, summary and category label here is translated, because those are what
//! a reader scans and navigates by, and an untranslated heading over translated answers
//! reads as a broken translation rather than an absent one.
//!
//! The body prose is English, and the page says so where the reader will see it. The
//! alternative -- machine-translating several thousand words of taxonomy and query
//! mechanics into three languages with nobody checking the result -- produces text that
//! *looks* localised and is wrong in the specifics that matter most here. A wrong
//! statement about when a lookup runs is worse than an honest English one.
//!
//! [`FaqBlock::Table`] carries most of the content, deliberately: a table of measured
//! numbers states its facts faster than prose does, and does not need translating to be
//! readable.

use crate::i18n::Locale;

/// One collapsible section of the reference.
pub struct GuideSection {
    /// Stable anchor, and the `#fragment` that links to this section.
    pub id: &'static str,
    /// The heading, and the label on the disclosure control.
    titles: Labels,
    /// One sentence shown when the section is collapsed.
    ///
    /// This is what a reader sees before deciding to open it, so it has to stand alone:
    /// it is a summary, not a teaser.
    summaries: Labels,
    /// The section's body. English.
    pub blocks: &'static [FaqBlock],
}

/// A piece of a section's body.
///
/// `Copy` because a block is static data and the renderer takes it by value rather than
/// borrowing through `&'static`, which keeps the `rsx!` arms free of lifetime noise.
#[derive(Clone, Copy)]
pub enum FaqBlock {
    /// A subheading, one level below the section.
    Heading(&'static str),
    /// A paragraph.
    Text(&'static str),
    /// A bulleted list.
    ///
    /// Present because prose this long will want one eventually, and a content-only
    /// addition should not need a renderer change. It has no user right now: every list
    /// worth having here turned out to be a table, and a table carries column headers.
    /// Kept out rather than left as a dead variant the build denies -- add it back with
    /// the first section that needs it.
    #[allow(dead_code)]
    List(&'static [&'static str]),
    /// A table. The first row is the header.
    Table(&'static [&'static [&'static str]]),
    /// A caveat worth setting apart: a limitation, or a deliberate omission.
    Note(&'static str),
}

/// The three translations of a section's own words.
///
/// A tuple rather than a lookup table because there are exactly four locales and the
/// pairing is checked by a test; a `BTreeMap` would be a runtime lookup for a value
/// known at compile time.
#[derive(Clone, Copy)]
struct Labels {
    en: &'static str,
    fr: &'static str,
    de: &'static str,
    it: &'static str,
}

impl Labels {
    const fn pick(self, locale: Locale) -> &'static str {
        match locale {
            Locale::En => self.en,
            Locale::Fr => self.fr,
            Locale::De => self.de,
            Locale::It => self.it,
        }
    }
}

/// The notice shown above the reference when the reader's locale is not English.
pub fn untranslated_notice(locale: Locale) -> Option<(&'static str, &'static str)> {
    match locale {
        Locale::En => None,
        Locale::Fr => Some((
            "Cette section est en anglais",
            "Les titres sont traduits, mais le texte détaillé est la source : la taxonomie et le \
             SPARQL sont expliqués mot pour mot, et une traduction automatique se tromperait \
             précisément sur les chiffres et les délais.",
        )),
        Locale::De => Some((
            "Dieser Abschnitt ist auf Englisch",
            "Die Überschriften sind übersetzt, der Fließtext ist die Quelle: Taxonomie und SPARQL \
             werden wörtlich erklärt, und eine automatische Übersetzung würde gerade bei Zahlen \
             und Zeiten danebenliegen.",
        )),
        Locale::It => Some((
            "Questa sezione è in inglese",
            "Le intestazioni sono tradotte, ma il testo è l'originale: tassonomia e SPARQL sono \
             spiegati alla lettera, e una traduzione automatica sbaglierebbe proprio sui numeri \
             e sui tempi.",
        )),
    }
}

/// Every section, in reading order.
pub const SECTIONS: &[GuideSection] = &[
    GuideSection {
        id: "taxon-name-to-qid",
        titles: Labels {
            en: "How a taxon name becomes an identifier",
            fr: "Comment un nom de taxon devient un identifiant",
            de: "Wie ein Taxonname zu einer Kennung wird",
            it: "Come un nome di taxon diventa un identificatore",
        },
        summaries: Labels {
            en: "Two lookups, one fast and indexed, one slow and exhaustive — and why the slow one is not folded into the fast one.",
            fr: "Deux recherches : l'une rapide et indexée, l'autre exhaustive et lente — et pourquoi la lente n'est pas fusionnée avec la rapide.",
            de: "Zwei Suchen, eine schnell und indiziert, die andere langsam und vollständig — und warum die langsame nicht in der schnellen aufgeht.",
            it: "Due ricerche, una veloce e indicizzata, l'altra lenta ed esaustiva — e perché la lenta non è fusa con la rapida.",
        },
        blocks: &[
            FaqBlock::Text(
                "Everything downstream is keyed on a Wikidata QID, so a name has to become one \
                 first. A bare QID skips that step entirely; a scientific name takes one indexed \
                 lookup; anything else takes a second, slower one.",
            ),
            FaqBlock::Table(&[
                &["Input", "What happens", "Cost"],
                &[
                    "A QID, e.g. Q128267",
                    "Used as-is, no request at all",
                    "none",
                ],
                &[
                    "A scientific name, e.g. Gentiana lutea",
                    "One indexed lookup on wdt:P225",
                    "about 0.3s",
                ],
                &[
                    "A common name, e.g. bitterwort",
                    "The scientific lookup first, then a full scan of P1843",
                    "0.3s, then about 2.5s",
                ],
                &[
                    "*",
                    "No taxon constraint at all — every organism",
                    "one lookup",
                ],
            ]),
            FaqBlock::Heading("Why the common-name lookup is a second request"),
            FaqBlock::Text(
                "A common name is a Wikidata statement, not an indexed value: there are 818,120 \
                 of them across 237,221 taxa, and matching one means comparing lexical forms, \
                 which is a scan of every statement. An indexed scientific name answers in about \
                 0.3s.",
            ),
            FaqBlock::Text(
                "Folding both into one query with a UNION would charge that 2.5s to every taxon \
                 search — including the overwhelming majority that match a scientific name on \
                 the first request and never need the second. That is exactly the reader who is \
                 already waiting.",
            ),
            FaqBlock::Note(
                "If the endpoint is unreachable the search fails rather than falling through to \
                 the slow lookup. If the endpoint is down, the scan would be down too, and two \
                 failed round trips is a slower way to learn the same thing.",
            ),
            FaqBlock::Heading("Why a common name resolves at all"),
            FaqBlock::Text(
                "Searching by a common name is discouraged but supported, and when it answers you \
                 are told so. Someone who typed bitterwort and got \"not found\" would go to \
                 Wikidata, perform the lookup this tool had just done, and come back with the same \
                 QID and less context than the notice gives them. A labelled answer beats no \
                 answer.",
            ),
            FaqBlock::Note(
                "The notice is part of the cached result, not recomputed. A repeat search \
                 reproduces it rather than silently losing it — which is what made the two \
                 notices look like they were taking turns.",
            ),
            FaqBlock::Heading("Names are standardised first"),
            FaqBlock::Text(
                "Underscores become spaces and the genus is capitalised before the lookup. When \
                 that changes what you typed, a notice says so rather than searching something \
                 you did not ask for.",
            ),
        ],
    },
    GuideSection {
        id: "nomenclature",
        titles: Labels {
            en: "The four other names a taxon has",
            fr: "Les quatre autres noms d'un taxon",
            de: "Die vier anderen Namen eines Taxons",
            it: "I quattro altri nomi di un taxon",
        },
        summaries: Labels {
            en: "Accepted name, basionym, original combination, replacement name — four separate switches, because they answer four different questions.",
            fr: "Nom accepté, basionyme, combination originale, nom de remplacement — quatre interrupteurs distincts, car ils répondent à quatre questions différentes.",
            de: "Akzeptierter Name, Basionym, Originalkombination, Ersatzname — vier getrennte Schalter, weil sie vier verschiedene Fragen beantworten.",
            it: "Nome accettato, basionimo, combinazione originale, nome sostitutivo — quattro interruttori separati, perché rispondono a quattro domande diverse.",
        },
        blocks: &[
            FaqBlock::Text(
                "A taxon search follows the taxon's nomenclatural closure: the name you gave, \
                 plus the other names Wikidata links to it, plus the descendants of all of them. \
                 The point is that the organism a paper names is rarely the organism Wikidata \
                 files the compound under — taxonomy is revised, the publications are not.",
            ),
            FaqBlock::Table(&[
                &["Relationship", "Property", "What it is"],
                &[
                    "Accepted name and its synonyms",
                    "P1420",
                    "The name a taxonomic authority currently endorses, and the other names for the same taxon. Either may be older; no chronology is recorded.",
                ],
                &[
                    "Basionym and new combination",
                    "P566",
                    "The name the taxon was first described under, which fixes the type specimen, and the combination its genus was later moved into.",
                ],
                &[
                    "Original combination / protonym",
                    "P1403",
                    "The binomial exactly as first published, before any later reclassification. The zoological word for the same idea is protonym.",
                ],
                &[
                    "Replacement name (nomen novum)",
                    "P694",
                    "A name that exists because the old one cannot be used at all — usually because it is a homonym, already taken by a different taxon.",
                ],
            ]),
            FaqBlock::Heading("Why four switches rather than one"),
            FaqBlock::Text(
                "Accepted-versus-synonym is a taxonomic judgement and can change: under a \
                 different opinion, a basionym may be the accepted name again. Old-versus-new is \
                 a nomenclatural fact and does not. Neither determines the other, which is why \
                 Wikidata stores them under four separate property pairs — and why one \
                 \"include synonyms\" switch could not tell you which relationship you had just \
                 turned off.",
            ),
            FaqBlock::Heading("Each switch changes a real answer"),
            FaqBlock::Text(
                "Distinct compounds returned for the same taxon, as each switch is turned off in \
                 turn. These are the numbers that justify four controls instead of one.",
            ),
            FaqBlock::Table(&[
                &[
                    "Taxon",
                    "Accepted/synonym off",
                    "Basionym off",
                    "Original comb. off",
                    "Replacement off",
                ],
                &["Leontopodium nivale", "0", "33", "33", "33"],
                &["Houpoea officinalis", "226", "0", "226", "226"],
                &["Gonyaulax tamarensis", "10", "10", "8", "10"],
                &["Salvia rosmarinus", "304", "304", "304", "25"],
                &["Rosmarinus officinalis", "304", "304", "304", "302"],
            ]),
            FaqBlock::Text(
                "Leontopodium nivale returns nothing at all with the first switch off — its 33 \
                 compounds are filed under the name Leontopodium alpinum, which Wikidata treats \
                 as the accepted name. Houpoea officinalis is the same failure from the other \
                 direction: with the basionym switch off, the 226 compounds filed under Magnolia \
                 officinalis become invisible.",
            ),
            FaqBlock::Heading("A genus is where the switches stop mattering"),
            FaqBlock::Text(
                "Searching Salvia and searching Rosmarinus return the same 9,440 rows and the \
                 same 2,536 compounds once the closure is followed, because in the current \
                 taxonomy they are the same genus. With the switches off they diverge sharply: \
                 Rosmarinus returns 3 taxa and Salvia 216.",
            ),
            FaqBlock::Note(
                "The closure is deliberately not the same as a taxonomic parent walk. Crossing a \
                 name link is a rename; crossing a parent link is descent into a different \
                 organism. Only parent links descend.",
            ),
            FaqBlock::Note(
                "Hybrid parentage (P1531) is excluded on purpose. A hybrid is a different \
                 organism with a parent of its own; P1531 records which parent it was bred \
                 from, which is provenance rather than identity. It is also asymmetric in the \
                 wrong direction — following it from a parent would pull in every garden hybrid \
                 ever bred from that parent, compounding at each generation.",
            ),
            FaqBlock::Note(
                "One over-reach is documented rather than patched. Following Salvia's closure \
                 reaches Mentha, because Wikidata records Mentha as a synonym of Audibertia and \
                 Audibertia as a synonym of Salvia. Each statement is individually defensible; \
                 their composition is not. Capping the closure does not remove it, and a \
                 hand-kept list of genera some curator once cross-linked would be worse than an \
                 honest result you can audit.",
            ),
            FaqBlock::Note(
                "Wikidata is a live graph, so every number above drifts as curation proceeds. \
                 The shape of the table is the durable part; the absolute counts are a snapshot.",
            ),
        ],
    },
    GuideSection {
        id: "compound-identity",
        titles: Labels {
            en: "How a name, a structure or a key becomes a compound",
            fr: "Comment un nom, une structure ou une clé devient un composé",
            de: "Wie ein Name, eine Struktur oder ein Schlüssel zu einem Molekül wird",
            it: "Come un nome, una struttura o una chiave diventa un composto",
        },
        summaries: Labels {
            en: "Four kinds of input, three modes of search, and one rule that is deliberately asymmetric: a name that matches nothing is an error, a structure that matches nothing is not.",
            fr: "Quatre types d'entrée, trois modes de recherche, et une règle délibérément asymétrique : un nom sans correspondance est une erreur, une structure sans correspondance non.",
            de: "Vier Arten von Eingabe, drei Suchmodi und eine Regel, die absichtlich asymmetrisch ist: ein Name ohne Treffer ist ein Fehler, eine Struktur ohne Treffer nicht.",
            it: "Quattro tipi di ingresso, tre modalità di ricerca e una regola volutamente asimmetrica: un nome senza corrispondenze è un errore, una struttura no.",
        },
        blocks: &[
            FaqBlock::Text(
                "Everything in the structure field is resolved to a compound identity before any \
                 search runs, a structure included, because the default mode needs a QID and a \
                 structure is not one.",
            ),
            FaqBlock::Table(&[
                &["You type", "Resolved by", "Cost"],
                &[
                    "A QID, e.g. Q23118",
                    "One VALUES — nothing to match",
                    "instant",
                ],
                &[
                    "An InChIKey, e.g. DBOVHQOUSDWAPQ-WTONXPSSSA-N",
                    "One indexed equality on P235",
                    "about 0.2s",
                ],
                &[
                    "A compound name, e.g. amarogentina",
                    "Its label, then its alias",
                    "about 0.15s, then 0.14s",
                ],
                &[
                    "A structure, e.g. C[C@H](O)CO",
                    "The structure service",
                    "one service call",
                ],
            ]),
            FaqBlock::Heading("The three modes"),
            FaqBlock::Table(&[
                &["Mode", "The question it answers", "Needs"],
                &[
                    "Exact (default)",
                    "This compound, and nothing else",
                    "Nothing beyond the QID",
                ],
                &[
                    "Substructure",
                    "Every compound containing this structure",
                    "The structure service",
                ],
                &[
                    "Similarity",
                    "Every compound at least this similar",
                    "The structure service and your cutoff",
                ],
            ]),
            FaqBlock::Text(
                "All three modes are available for every kind of input. A reader who types a name \
                 and wants the compounds that contain it is asking a real question, and knowing \
                 which compound the name referred to does not answer it.",
            ),
            FaqBlock::Heading("Why exact is the default"),
            FaqBlock::Text(
                "Exact is an index scan on one identifier. The broader modes load and score \
                 every candidate compound. That is an order of magnitude apart, so defaulting to \
                 a broad mode would tax every search to make the rare one cheaper.",
            ),
            FaqBlock::Heading("A miss is an error — except for a structure"),
            FaqBlock::Text(
                "A QID, a name or an InChIKey that matched nothing is a claim about a compound \
                 that does not exist, and there is nothing to guess at. You get \"not found\" \
                 with your input quoted back, rather than rows for a search you did not ask for.",
            ),
            FaqBlock::Text(
                "A structure that Wikidata has no compound for is not a miss. It is a good \
                 structure that is simply not in the database, and searching it is what you \
                 asked for. Nothing about a SMILES is a claim that can turn out to be wrong — \
                 which is why refusing it would break every structure search there ever was.",
            ),
            FaqBlock::Heading("Names that look like structures"),
            FaqBlock::Text(
                "ATP, GDP and NAD are compound names Wikidata knows, and they are refused. They \
                 are written the same way as CC and CCC, which are both plausible things to type \
                 in a structure box. A structure silently replaced by a same-spelled Wikidata \
                 item is a failure you cannot detect; a name you have to retype as a structure or \
                 an InChIKey is a friction you can route around.",
            ),
            FaqBlock::Note(
                "Compound names resolve across fifteen language tags rather than all of them, \
                 because matching every tag means scanning the label property — 17.9 million \
                 rows, which does not finish. A compound name in one of the remaining tags will \
                 not resolve. That is a real limitation and it is not hidden.",
            ),
            FaqBlock::Note(
                "Label and alias stay two separate requests because a single UNION of them does \
                 not finish on the endpoint either — it falls back to scanning each property.",
            ),
        ],
    },
    GuideSection {
        id: "results-and-limits",
        titles: Labels {
            en: "Results, identifiers and limits",
            fr: "Résultats, identifiants et limites",
            de: "Ergebnisse, Kennungen und Grenzen",
            it: "Risultati, identificatori e limiti",
        },
        summaries: Labels {
            en: "Every matching row is fetched and counted. What is limited, and what is not.",
            fr: "Toutes les lignes correspondantes sont récupérées et comptées. Ce qui est limité, et ce qui ne l'est pas.",
            de: "Jede passende Zeile wird geladen und gezählt. Was begrenzt ist und was nicht.",
            it: "Vengono recuperate e contate tutte le righe corrispondenti. Cosa è limitato e cosa no.",
        },
        blocks: &[
            FaqBlock::Table(&[
                &["Limited", "How"],
                &[
                    "Rows per search",
                    "Not limited. The table holds every matching row, and the counts are computed from that same set rather than by a separate query.",
                ],
                &[
                    "Rows rendered at once",
                    "Only the window you are looking at. Scrolling materialises the next screenful.",
                ],
                &[
                    "Structure service candidates",
                    "Five, then the extras surface as an ambiguity notice rather than being hidden.",
                ],
                &[
                    "Export size",
                    "Effectively unlimited. The file is written a chunk at a time rather than assembled in memory.",
                ],
            ]),
            FaqBlock::Heading("Identifiers"),
            FaqBlock::Text(
                "A QID is a Wikidata identifier: Q followed by a number, naming one entity \
                 unambiguously. It is the join key between your query and everything else on the \
                 graph, and the two hash buttons give you the query and the result set as \
                 identifiers you can quote and compare between runs.",
            ),
            FaqBlock::Heading("Filters change the table, not the export"),
            FaqBlock::Text(
                "Column filters and sorting apply to what is on screen. A download is produced \
                 from the query's result set, because the file is written by walking that set \
                 rather than by asking anyone to run the query again. The two agree on rows and \
                 differ on presentation.",
            ),
            FaqBlock::Note(
                "The reference year is exported as a year, not as a full date: the result store \
                 keeps only the year, and emitting a date column holding a year would be a wrong \
                 value rather than a missing one. DOIs are exported in Wikidata's canonical \
                 upper-cased form.",
            ),
        ],
    },
    GuideSection {
        id: "export",
        titles: Labels {
            en: "Exports and large downloads",
            fr: "Exports et téléchargements volumineux",
            de: "Exporte und große Downloads",
            it: "Esportazioni e download grandi",
        },
        summaries: Labels {
            en: "CSV, JSON and RDF are written from the rows already in the tab. Why that matters when the file is large.",
            fr: "CSV, JSON et RDF sont écrits à partir des lignes déjà présentes dans l'onglet. Pourquoi c'est important pour un gros fichier.",
            de: "CSV, JSON und RDF werden aus den bereits geladenen Zeilen geschrieben. Warum das bei großen Dateien zählt.",
            it: "CSV, JSON e RDF sono scritti dalle righe già in scheda. Perché conta quando il file è grande.",
        },
        blocks: &[
            FaqBlock::Text(
                "An export used to be produced by a SPARQL endpoint that re-ran your query and \
                 named the file itself — which is why the name on the button and the name on \
                 disk could disagree. It is now written out of the rows the table is already \
                 drawn from, so the filename is the only name in play and the query is not run \
                 twice.",
            ),
            FaqBlock::Table(&[
                &["Format", "What you get"],
                &[
                    "CSV",
                    "One header row and the rows, RFC 4180 quoted. The compact, lossless default for bulk work.",
                ],
                &[
                    "JSON",
                    "SPARQL Results JSON: a head declaring the columns, then one binding object per row.",
                ],
                &["RDF", "Turtle triples, for loading into a triple store."],
            ]),
            FaqBlock::Heading("Why a large export does not exhaust the tab"),
            FaqBlock::Text(
                "The file is produced in chunks and handed on a chunk at a time, so the whole \
                 export never exists in memory. Where the browser can write a file directly, each \
                 chunk goes straight to disk as it is produced.",
            ),
            FaqBlock::Table(&[
                &["Browser", "Where the bytes go", "Peak memory"],
                &[
                    "Chrome, Edge, Opera",
                    "A file you choose the location for",
                    "One chunk",
                ],
                &[
                    "Safari 15.2+, Firefox 111+",
                    "Private storage, then downloaded from there",
                    "One chunk",
                ],
                &[
                    "Anything older",
                    "Assembled in the tab and handed over at the end",
                    "The whole file",
                ],
            ]),
            FaqBlock::Note(
                "On the older-browser path a full-size export can still exhaust the tab. If you \
                 are on an older browser and a large export fails, that is why — the file is \
                 being built in memory because there is nowhere else to put it.",
            ),
        ],
    },
];

impl GuideSection {
    /// The section title in `locale`.
    #[must_use]
    pub fn title(&self, locale: Locale) -> &'static str {
        self.titles.pick(locale)
    }

    /// The collapsed-section summary in `locale`.
    #[must_use]
    pub fn summary(&self, locale: Locale) -> &'static str {
        self.summaries.pick(locale)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::{FaqBlock, Labels, SECTIONS, untranslated_notice};
    use crate::i18n::Locale;

    const LOCALES: [Locale; 4] = [Locale::En, Locale::Fr, Locale::De, Locale::It];

    #[test]
    fn every_section_has_a_title_and_summary_in_every_locale() {
        for section in SECTIONS {
            for locale in LOCALES {
                assert!(
                    !section.title(locale).trim().is_empty(),
                    "{} has no title in {locale:?}",
                    section.id
                );
                assert!(
                    !section.summary(locale).trim().is_empty(),
                    "{} has no summary in {locale:?}",
                    section.id
                );
            }
        }
    }

    #[test]
    fn the_summary_is_long_enough_to_be_a_summary() {
        // A collapsed section shows only its summary. One that is shorter than its title
        // is not a summary, and a reader opening the section learns nothing from having
        // done so.
        for section in SECTIONS {
            for locale in LOCALES {
                assert!(
                    section.summary(locale).chars().count() > 30,
                    "{}: the {locale:?} summary is shorter than a sentence, so a collapsed                      section says nothing",
                    section.id
                );
            }
        }
    }

    #[test]
    fn no_locale_serves_the_english_title_or_summary() {
        // A missing translation that falls through to English is invisible in review and
        // obvious to a reader.
        for section in SECTIONS {
            for locale in [Locale::Fr, Locale::De, Locale::It] {
                assert_ne!(
                    section.title(locale),
                    section.title(Locale::En),
                    "{} is untranslated for {locale:?}",
                    section.id
                );
                assert_ne!(
                    section.summary(locale),
                    section.summary(Locale::En),
                    "{} is untranslated for {locale:?}",
                    section.id
                );
            }
        }
    }

    #[test]
    fn section_anchors_are_unique_and_url_clean() {
        let mut seen: Vec<&str> = Vec::new();
        for section in SECTIONS {
            assert!(
                !seen.contains(&section.id),
                "duplicate anchor {}",
                section.id
            );
            assert!(
                section
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{} is not a clean URL fragment",
                section.id
            );
            seen.push(section.id);
        }
    }

    #[test]
    fn no_section_is_empty() {
        // A disclosure that opens onto nothing is a dead end.
        for section in SECTIONS {
            assert!(!section.blocks.is_empty(), "{} has no content", section.id);
            // And it must not be *only* headings.
            assert!(
                section
                    .blocks
                    .iter()
                    .any(|b| matches!(b, FaqBlock::Text(_) | FaqBlock::Table(_))),
                "{} has headings but no prose",
                section.id
            );
        }
    }

    #[test]
    fn every_table_has_a_header_and_at_least_one_row() {
        // A one-row "table" is a header with no data, which reads as an empty section.
        for section in SECTIONS {
            for block in section.blocks {
                let FaqBlock::Table(rows) = block else {
                    continue;
                };
                assert!(
                    rows.len() >= 2,
                    "{}: a table needs a header and a row",
                    section.id
                );
                let width = rows.first().map_or(0, |header| header.len());
                assert!(
                    width > 1,
                    "{}: a single-column table is a paragraph",
                    section.id
                );
                for (index, row) in rows.iter().enumerate() {
                    assert_eq!(
                        row.len(),
                        width,
                        "{}: row {index} has {} cells against a {width}-cell header",
                        section.id,
                        row.len()
                    );
                }
            }
        }
    }

    #[test]
    fn the_notice_appears_for_every_locale_but_english() {
        // The deep-dive body is English. A reader in another language has to be told, at
        // the point where they will see it, rather than inferring it from a page that is
        // otherwise translated.
        assert!(
            untranslated_notice(Locale::En).is_none(),
            "an English reader does not need to be told the page is in English"
        );
        for locale in [Locale::Fr, Locale::De, Locale::It] {
            let Some((label, detail)) = untranslated_notice(locale) else {
                panic!("{locale:?} has no notice for the English-only body");
            };
            assert!(!label.trim().is_empty(), "{locale:?} notice label is empty");
            assert!(
                detail.chars().count() > 40,
                "{locale:?} notice says nothing"
            );
        }
    }

    #[test]
    fn labels_pick_the_locale_that_was_asked_for() {
        // A guard on the lookup itself: the four arms are positional, and a mistake there
        // would silently serve one language to every reader.
        let labels = Labels {
            en: "english",
            fr: "french",
            de: "german",
            it: "italian",
        };
        assert_eq!(labels.pick(Locale::En), "english");
        assert_eq!(labels.pick(Locale::Fr), "french");
        assert_eq!(labels.pick(Locale::De), "german");
        assert_eq!(labels.pick(Locale::It), "italian");
    }
}
