// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The `/faq` page: the questions readers actually ask, grouped and linkable.
//!
//! Deliberately not a disclosure widget. A `<details>` per question hides the answer
//! behind a click, which costs a keyboard user a tab stop per question and hides the
//! text from anything that reads the page rather than renders it -- a search engine,
//! a reader mode, an agent. The answers here are two or three sentences each, so
//! showing all of them costs one scroll and buys a page that can be read, indexed and
//! quoted in any order.
//!
//! The structure is what makes it navigable rather than merely readable: one `h1`
//! owned by the page header, an `h2` for the page, an `h3` per category and an `h4`
//! per question, no level skipped, and every question carrying a stable `id`. Those
//! ids are the same fragments emitted as `@id` in the `FAQPage` structured data, so a
//! link to one question works from the page, from a search result, and from an agent
//! that read the markup.

use crate::app::routes::RouteQuery;
use crate::components::layout::escape_faq_script_element;
use crate::hooks::use_locale;
use crate::i18n::faq::{FaqEntry, faq_chrome};
use crate::i18n::{ENTRIES, FaqCategory, faq_json_ld};
use dioxus::prelude::*;

/// The FAQ page.
#[component]
pub fn FaqPage(query: RouteQuery, hash: String) -> Element {
    let locale = use_locale();
    let chrome = faq_chrome(locale);
    // Deep-linked already: the router puts `/faq#download-formats` in `hash`, and the
    // browser scrolls to it once the element with that id exists. Doing it here as
    // well would race the render.
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
            // `aria-labelledby` rather than `aria-label`, so the section announces the
            // same heading a sighted reader sees instead of a second string to keep in
            // sync.
            aria_labelledby: "faq-heading",

            // The structured data sits in the tree, not the head: `document::Script`
            // installs itself once with `use_hook` and would keep showing the first
            // locale's answers after the reader switched language. JSON-LD is valid in
            // the body and consumers read it from there.
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

                // A table of contents, because the point of grouping is that a reader
                // can aim at one group rather than scroll twelve questions to find out
                // whether downloads are covered.
                nav {
                    class: "mt-6",
                    aria_label: "{chrome.contents_label}",
                    h3 { class: "text-subtitle font-semibold text-text", "{chrome.contents_heading}" }
                    ul { class: "mt-2 flex flex-wrap gap-x-4 gap-y-1 list-disc pl-5",
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
                                // The question is the term and the answer the
                                // description, which is what makes "copy the definition"
                                // and most screen-reader browse modes work here rather
                                // than only reading top to bottom.
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
            }
        }
    }
}

/// The anchor for a category heading.
///
/// `rsx!` interpolates a `&str`, so the heading needs a string and `FaqCategory` is an
/// enum that does not have one. Deriving `as_str` on the enum would put the id next to
/// the category it names, which is where it belongs; this match keeps it next to the
/// markup that renders it, and the test below asserts the two cannot drift apart.
fn category_anchor(category: FaqCategory) -> &'static str {
    match category {
        FaqCategory::About => "faq-about",
        FaqCategory::Searching => "faq-searching",
        FaqCategory::Results => "faq-results",
        FaqCategory::Export => "faq-export",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::category_anchor;
    use crate::i18n::{ENTRIES, FaqCategory};
    use std::collections::BTreeSet;

    /// The page's furniture must not be written inline.
    ///
    /// An explicit list rather than a scanner for string literals. A scanner was tried
    /// and produced false positives on class lists, on `//` comments, and on the
    /// assertion messages of the tests themselves -- three false positives in one run,
    /// which is how a guard like that ends up ignored. Naming the strings that actually
    /// regressed is narrower, has no false positives, and says what it is for.
    #[test]
    fn no_user_visible_text_is_hardcoded_in_the_page() {
        let source = include_str!("faq.rs");
        // Only the rendered component; the test module below is allowed to hold strings.
        let rendered = source.split("#[cfg(test)]").next().unwrap_or_default();

        for literal in [
            "Frequently asked questions",
            "On this page",
            "What this searches",
        ] {
            assert!(
                !rendered.contains(literal),
                "`{literal}` is written inline; the page's own words belong in \
                 `faq_chrome`, which is translated. Every answer on this page was \
                 translated while the heading above them was not."
            );
        }

        // And positively: the lookup has to be used, or the assertions above would pass
        // against a page that simply deleted its heading.
        assert!(
            rendered.contains("faq_chrome(locale)"),
            "the page must resolve its furniture through `faq_chrome`"
        );
        for field in [
            "chrome.heading",
            "chrome.intro",
            "chrome.contents_label",
            "chrome.contents_heading",
        ] {
            assert!(
                rendered.contains(field),
                "`{field}` is not rendered, so the lookup is not actually used"
            );
        }
    }

    #[test]
    fn category_anchors_are_unique() {
        let anchors: BTreeSet<&str> = FaqCategory::ALL
            .iter()
            .copied()
            .map(category_anchor)
            .collect();
        assert_eq!(
            anchors.len(),
            FaqCategory::ALL.len(),
            "two categories share an anchor, so one heading is unreachable by link"
        );
    }

    #[test]
    fn no_question_anchor_collides_with_a_category_anchor() {
        for entry in ENTRIES {
            for category in FaqCategory::ALL {
                assert_ne!(
                    entry.id,
                    category_anchor(category),
                    "{} collides with a category anchor",
                    entry.id
                );
            }
        }
    }

    #[test]
    fn every_category_actually_has_questions() {
        // An empty category renders a heading with nothing under it, which is a
        // dead end for a reader and a useless entry in the contents list.
        for category in FaqCategory::ALL {
            assert!(
                ENTRIES.iter().any(|e| e.category == category),
                "{category:?} has no questions"
            );
        }
    }

    #[test]
    fn the_anchor_helpers_agree_on_the_same_ids() {
        // Guards the rename that would otherwise only show up as a broken link.
        assert_eq!(category_anchor(FaqCategory::About), "faq-about");
        assert_eq!(category_anchor(FaqCategory::Searching), "faq-searching");
        assert_eq!(category_anchor(FaqCategory::Results), "faq-results");
        assert_eq!(category_anchor(FaqCategory::Export), "faq-export");
    }
}
