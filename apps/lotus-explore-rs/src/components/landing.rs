// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::app::routes::{Route, RouteQuery};
use crate::hooks::use_locale;
use crate::i18n::{TextKey, t};
use dioxus::prelude::*;

#[component]
pub fn LandingPage() -> Element {
    let locale = use_locale();
    let route: Route = use_route();
    // A route rather than a string, and Dioxus's `Link` rather than a bare anchor:
    // an anchor is a full page load, which in a browser is a round trip to a server
    // answering every path with the app and in a window lands on a URL the app's own
    // protocol does not serve, so the button did nothing. `Link` asks the router to
    // navigate, the same on both. The landing route already holds the query string,
    // so the search view opens on the filters the visitor arrived with.
    let search_target = match &route {
        Route::Landing { query, hash } => Route::Search {
            query: query.clone(),
            hash: hash.clone(),
        },
        _ => Route::Search {
            query: RouteQuery::default(),
            hash: String::new(),
        },
    };
    rsx! {
        section {
            class: "w-full max-w-none px-4 pt-3 sm:px-6 lg:px-8",
            aria_labelledby: "landing-welcome-heading",
            div {
                class: "mx-auto min-w-0 w-full max-w-6xl",
                div {
                    class: "min-w-0 w-full max-w-full overflow-hidden rounded-xl border border-shell-border bg-shell-raised p-6 shadow-xs sm:p-8",
                h2 {
                    id: "landing-welcome-heading",
                    class: "text-title font-semibold text-text",
                    "{t(locale, TextKey::LandingTitle)}"
                }
                p {
                    class: "mt-3 w-full min-w-0 break-words text-body leading-relaxed text-muted",
                    "{t(locale, TextKey::WelcomeLeadA)}"
                    "{t(locale, TextKey::WelcomeLeadB)}"
                    a {
                        href: "https://www.wikidata.org/wiki/Q104225190",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "mx-1 font-medium text-accent hover:underline",
                        "LOTUS initiative"
                    }
                    "{t(locale, TextKey::WelcomeLeadC)}"
                    a {
                        href: "https://www.wikidata.org/",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "mx-1 font-medium text-accent hover:underline",
                        "Wikidata"
                    }
                    "{t(locale, TextKey::WelcomeLeadD)}"
                    a {
                        href: "https://qlever.dev/wikidata",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "mx-1 font-medium text-accent hover:underline",
                        "QLever"
                    }
                    "{t(locale, TextKey::WelcomeLeadE)}"
                    " "
                    span {
                        class: "text-ui italic text-subtle",
                        "{t(locale, TextKey::LabelLanguagePolicy)}"
                    }
                }
                }
                div {
                    class: "mt-6 flex justify-center",
                    Link {
                        to: NavigationTarget::Internal(search_target.navigation_string()),
                        class: "inline-flex min-h-[40px] items-center justify-center gap-2 rounded-xl bg-accent px-3.5 py-2 text-ui font-semibold text-bg no-underline shadow-xs transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] hover:bg-accent-2 active:bg-accent-2 active:scale-[0.98] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 cursor-pointer",
                        "{t(locale, TextKey::OpenSearch)}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn NotFoundPage() -> Element {
    let locale = use_locale();
    // A route rather than an href, for the same reason as the landing page's
    // "Open search": an anchor is a page load, and a window has no server to
    // answer it.
    let home_target = Route::Landing {
        query: RouteQuery::default(),
        hash: String::new(),
    };
    rsx! {
        section {
            class: "w-full max-w-none px-4 pt-3 sm:px-6 lg:px-8",
            aria_labelledby: "not-found-heading",
            div {
                class: "w-full max-w-3xl rounded-xl border border-shell-border bg-shell-raised p-6 shadow-xs sm:p-8",
                h2 {
                    id: "not-found-heading",
                    class: "text-title font-semibold text-text",
                    "{t(locale, TextKey::PageNotFound)}"
                }
                p {
                    class: "mt-3 text-body leading-relaxed text-muted",
                    "{t(locale, TextKey::PageNotFoundDescription)}"
                }
                Link {
                    to: NavigationTarget::Internal(home_target.navigation_string()),
                    class: "inline-flex min-h-[40px] items-center justify-center gap-2 rounded-xl bg-accent px-3.5 py-2 text-ui font-semibold text-bg no-underline shadow-xs transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] hover:bg-accent-2 active:bg-accent-2 active:scale-[0.98] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 cursor-pointer",
                    "{t(locale, TextKey::ReturnHome)}"
                }
            }
        }
    }
}
