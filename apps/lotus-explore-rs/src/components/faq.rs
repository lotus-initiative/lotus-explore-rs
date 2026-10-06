// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The `/faq` page: the questions readers actually ask, grouped and linkable.
//!
//! Deliberately not a disclosure widget: a `<details>` per question costs a keyboard
//! user a tab stop each and hides the text from anything that reads the page rather
//! than renders it — a search engine, a reader mode, an agent. At two or three
//! sentences per answer, showing all of them costs one scroll and buys a page that
//! can be read, indexed and quoted in any order.
//!
//! Navigable rather than merely readable: one `h1` owned by the page header, an `h2`
//! for the page, an `h3` per category, an `h4` per question, no level skipped, and a
//! stable `id` per question. Those ids are the same fragments emitted as `@id` in the
//! `FAQPage` structured data, so a link to one question works from the page, from a
//! search result, and from an agent that read the markup.

use crate::app::routes::RouteQuery;
use crate::components::layout::escape_faq_script_element;
use crate::hooks::use_locale;
use crate::i18n::faq::{FaqEntry, faq_chrome};
use crate::i18n::faq_guide::{FaqBlock, SECTIONS, untranslated_notice};
use crate::i18n::{ENTRIES, FaqCategory, faq_json_ld};
use dioxus::prelude::*;

/// The FAQ page.
#[component]
pub fn FaqPage(query: RouteQuery, hash: String) -> Element {
    let locale = use_locale();
    let chrome = faq_chrome(locale);
    let reference_heading = chrome.reference_heading;
    let reference_intro = chrome.reference_intro;
    // Deep-linked already: the router puts `/faq#download-formats` in `hash` and the
    // browser scrolls once the element exists. Scrolling here too would race the render.
    let _ = (query, hash);

    let questions_by_category: Vec<(FaqCategory, Vec<&'static FaqEntry>)> = FaqCategory::ALL
        .iter()
        .map(|category| {
            let entries: Vec<&'static FaqEntry> =
                ENTRIES.iter().filter(|e| e.category == *category).collect();
            (*category, entries)
        })
        .filter(|(_, entries)| !entries.is_empty())
        .collect();

    rsx! {
        section {
            class: "w-full max-w-none px-4 pt-3 sm:px-6 lg:px-8",
            // `aria-labelledby` so the section announces the heading a sighted reader
            // sees, not a second string to keep in sync.
            aria_labelledby: "faq-heading",

            // In the tree, not the head: `document::Script` installs itself once with
            // `use_hook` and would keep showing the first locale's answers after a
            // language switch. JSON-LD is valid in the body.
            script {
                type: "application/ld+json",
                dangerous_inner_html: escape_faq_script_element(&faq_json_ld(locale)),
            }

            div { class: "mx-auto min-w-0 w-full max-w-3xl",
                h2 {
                    id: "faq-heading",
                    class: "text-title font-semibold text-text",
                    "{chrome.heading}"
                }

                p {
                    class: "mt-3 text-body leading-relaxed text-muted",
                    "{chrome.intro}"
                }

                // A table of contents: grouping exists so a reader can aim at one group
                // rather than scroll twelve questions to learn whether downloads are covered.
                nav {
                    class: "mt-6",
                    aria_label: "{chrome.contents_label}",
                    h3 { class: "text-subtitle font-semibold text-text", "{chrome.contents_heading}" }
                    // No list markers, deliberately. A marker is drawn *outside* the box
                    // it belongs to, so a flex container's `gap` spaces the items while
                    // the dots stay pinned to the container's padding edge, drifting
                    // apart as the row narrows until on a phone the dots touch the text.
                    // Four short links read better inline than bulleted anyway.
                    ul { class: "mt-2 flex list-none flex-wrap items-baseline gap-x-4 gap-y-1 text-body",
                        for (category, _) in questions_by_category.iter() {
                            li {
                                a {
                                    href: "#{category_anchor(*category)}",
                                    class: "text-body text-accent underline",
                                    "{category.label(locale)}"
                                }
                            }
                        }
                    }
                }

                for (category, entries) in questions_by_category.iter() {
                    h3 {
                        id: category_anchor(*category),
                        class: "mt-8 scroll-mt-24 text-subtitle font-semibold text-text",
                        "{category.label(locale)}"
                    }
                    dl { class: "mt-3 flex flex-col gap-5",
                        for entry in entries {
                            div { class: "min-w-0",
                                // Term and description, so "copy the definition" and
                                // most screen-reader browse modes work, not only reading
                                // top to bottom.
                                dt {
                                    class: "text-body font-semibold text-text",
                                    h4 { id: "{entry.id}",
                                        class: "scroll-mt-24",
                                        "{entry.question(locale)}"
                                    }
                                }
                                dd {
                                    class: "mt-1 text-body leading-relaxed text-muted",
                                    "{entry.answer(locale)}"
                                }
                            }
                        }
                    }
                }

                //
                // The Q&A says what the tool does; this says why, which is what a reader
                // needs when a result surprises them.
                h3 {
                    id: "faq-how-it-works",
                    class: "mt-10 scroll-mt-24 text-subtitle font-semibold text-text",
                    "{reference_heading}"
                }
                p {
                    class: "mt-2 text-body leading-relaxed text-muted",
                    "{reference_intro}"
                }

                // Said where the reader sees it, not left to be inferred.
                if let Some((label, detail)) = untranslated_notice(locale) {
                    div {
                        class: "mt-4 rounded-lg border border-border bg-surface p-4",
                        role: "note",
                        p { class: "text-body font-semibold text-text", "{label}" }
                        p {
                            class: "mt-1 text-body leading-relaxed text-muted",
                            "{detail}"
                        }
                    }
                }

                div { class: "mt-4 flex flex-col gap-3",
                    for section in SECTIONS {
                        // Collapsed by default: five sections, and most readers arrived for
                        // the questions above. A disclosure is keyboard-operable and its
                        // content stays in the accessibility tree and the document, so
                        // collapsing hides nothing from a screen reader or a crawler.
                        details {
                            class: "rounded-xl border border-shell-border bg-shell-raised",
                            id: "{section.id}",
                            open: false,
                            summary {
                                class: "cursor-pointer list-none px-4 py-3 text-body font-semibold text-text marker:content-none",
                                div { "{section.title(locale)}" }
                                // Stands alone when collapsed, so a summary not a teaser.
                                p {
                                    class: "mt-1 text-sm font-normal leading-relaxed text-muted",
                                    "{section.summary(locale)}"
                                }
                            }
                            div { class: "border-t border-shell-border px-4 py-4",
                                for (index, block) in section.blocks.iter().enumerate() {
                                    {render_block(*block, index == 0)}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The anchor for a category heading.
///
/// `rsx!` interpolates a `&str` and `FaqCategory` is an enum without one. Deriving
/// `as_str` would put the id next to the category it names; this match keeps it next to
/// the markup, and the test below asserts they cannot drift.
fn category_anchor(category: FaqCategory) -> &'static str {
    match category {
        FaqCategory::About => "faq-about",
        FaqCategory::Searching => "faq-searching",
        FaqCategory::Results => "faq-results",
        FaqCategory::Export => "faq-export",
    }
}

/// Render one block of a reference section.
///
/// `first` is true for the block immediately after the disclosure is opened, so the
/// first paragraph does not carry a top margin against the border above it.
fn render_block(block: FaqBlock, first: bool) -> Element {
    let top = if first { "mt-0" } else { "mt-4" };
    match block {
        FaqBlock::Heading(text) => rsx! {
            // `h4` under the `h3` section heading, and the questions above use `h4`
            // too, so the outline is flat rather than skipping a level.
            h4 { class: "mt-5 text-body font-semibold text-text", "{text}" }
        },
        FaqBlock::Text(text) => rsx! {
            p { class: "{top} text-body leading-relaxed text-muted", "{text}" }
        },
        FaqBlock::List(items) => rsx! {
            // Block flow, not flex, for the same reason as the contents list above: a
            // flex container puts the marker outside the gap it is measuring.
            ul { class: "{top} flex list-none flex-col gap-1 text-body leading-relaxed text-muted",
                for item in items.iter() {
                    li { "{*item}" }
                }
            }
        },
        FaqBlock::Table(rows) => {
            // A real table with a real header row, not a grid of divs: the only
            // structure a screen reader announces row and column headers for, and
            // these tables are mostly measured numbers compared across columns. The
            // caption repeats the header row for anyone arriving by cell rather than by
            // reading headers.
            let caption = rows
                .first()
                .and_then(|row| row.first())
                .copied()
                .unwrap_or_default();
            rsx! {
                div { class: "mt-4 overflow-x-auto",
                    table { class: "w-full min-w-[36rem] border-collapse text-left text-sm",
                        caption { class: "sr-only", "{caption}" }
                        thead {
                            tr {
                                for cell in rows.first().copied().unwrap_or_default().iter() {
                                    th {
                                        class: "border-b border-shell-border px-2 py-1.5 font-semibold text-text",
                                        scope: "col",
                                        "{*cell}"
                                    }
                                }
                            }
                        }
                        tbody {
                            for row in rows.iter().skip(1) {
                                tr {
                                    for (index, cell) in row.iter().enumerate() {
                                        if index == 0 {
                                            th {
                                                class: "border-b border-shell-border px-2 py-1.5 text-left font-medium text-text",
                                                scope: "row",
                                                "{*cell}"
                                            }
                                        } else {
                                            td {
                                                class: "border-b border-shell-border px-2 py-1.5 align-top text-muted",
                                                "{*cell}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        FaqBlock::Note(text) => rsx! {
            // `role="note"` rather than a warning role: these are caveats and known
            // limits, not errors, and a screen reader announcing "alert" for a
            // documentation caveat would be worse than saying nothing.
            div { class: "{top} rounded-lg border border-border bg-surface px-3 py-2",
                role: "note",
                p { class: "text-sm leading-relaxed text-muted", "{text}" }
            }
        },
    }
}

#[cfg(test)]
#[path = "faq/tests.rs"]
mod tests;
