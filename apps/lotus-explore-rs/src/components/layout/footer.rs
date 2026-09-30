// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

// Dioxus's `asset!` macro resolves to `&[u8]`, flagged by `volatile_composites`
// at every call site; the value type is fixed by the framework.
#![allow(clippy::volatile_composites)]
//! Site footer component shared across pages.

use crate::hooks::use_locale;
use crate::i18n::{Locale, TextKey, t};
use dioxus::prelude::*;

#[component]
pub fn Footer() -> Element {
    let locale = use_locale();
    rsx! {
        div {
            class: "flex flex-col gap-3 py-1 border-b border-border last:border-b-0 sm:flex-row sm:items-start sm:gap-x-6 sm:gap-y-0",
            div {
                class: "flex items-center gap-2 py-0.5 sm:flex-[1_1_280px] sm:min-w-[280px] flex-nowrap",
                span {
                    class: "inline-flex items-center font-bold uppercase tracking-[0.06em] whitespace-nowrap leading-normal px-2 py-1 rounded-xl border border-current/30 border-l-4 border-current bg-current/14 min-h-[34px] text-sm max-[480px]:whitespace-normal max-[480px]:text-micro max-[480px]:px-1.5 max-[480px]:py-[2px] max-[480px]:min-h-0 shrink-0 text-wd-compound",
                    "{t(locale, TextKey::FooterArchive)}"
                }
                ul {
                    class: "flex items-center gap-x-2.5 list-none m-0 p-0 min-w-0 overflow-x-auto whitespace-nowrap pb-1 scrollbar-hidden-x flex-nowrap",
                    li {
                        class: "shrink-0",
                        a {
                            class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-compound",
                            href: "https://doi.org/10.5281/zenodo.5794106",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "LOTUS Frozen"
                        }
                    }
                }
            }
            FooterCitationRow { locale }
        }
        div {
            class: "flex flex-col gap-3 py-1 border-b border-border last:border-b-0 sm:flex-row sm:items-start sm:gap-x-6 sm:gap-y-0",
            div {
                class: "flex items-center gap-2 py-0.5 sm:flex-[1_1_280px] sm:min-w-[280px] flex-nowrap",
                span {
                    class: "inline-flex items-center font-bold uppercase tracking-[0.06em] whitespace-nowrap leading-normal px-2 py-1 rounded-xl border border-current/30 border-l-4 border-current bg-current/14 min-h-[34px] text-sm max-[480px]:whitespace-normal max-[480px]:text-micro max-[480px]:px-1.5 max-[480px]:py-[2px] max-[480px]:min-h-0 shrink-0 text-wd-taxon",
                    "{t(locale, TextKey::FooterCode)}"
                }
                ul {
                    class: "flex items-center gap-x-2.5 list-none m-0 p-0 min-w-0 overflow-x-auto whitespace-nowrap pb-1 scrollbar-hidden-x flex-nowrap",
                    li {
                        class: "shrink-0",
                        a {
                            class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-taxon",
                            href: env!("CARGO_PKG_REPOSITORY"),
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "lotus-explore-rs"
                        }
                    }
                }
            }
            div {
                class: "flex items-center gap-2 py-0.5 sm:flex-[1_1_280px] sm:min-w-[280px] flex-nowrap",
                span {
                    class: "inline-flex items-center font-bold uppercase tracking-[0.06em] whitespace-nowrap leading-normal px-2 py-1 rounded-xl border border-current/30 border-l-4 border-current bg-current/14 min-h-[34px] text-sm max-[480px]:whitespace-normal max-[480px]:text-micro max-[480px]:px-1.5 max-[480px]:py-[2px] max-[480px]:min-h-0 shrink-0 text-wd-taxon",
                    "{t(locale, TextKey::FooterData)}"
                }
                ul {
                    class: "flex items-center gap-x-2.5 list-none m-0 p-0 min-w-0 overflow-x-auto whitespace-nowrap pb-1 scrollbar-hidden-x flex-nowrap",
                    li {
                        class: "shrink-0",
                        a {
                            class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-taxon",
                            href: "https://www.wikidata.org/wiki/Q104225190",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "LOTUS Initiative"
                        }
                    }
                    li {
                        class: "shrink-0",
                        a {
                            class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-taxon",
                            href: "https://www.wikidata.org/",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "Wikidata"
                        }
                    }
                }
            }
        }
        div {
            class: "flex flex-col gap-3 py-1 border-b border-border last:border-b-0 sm:flex-row sm:items-start sm:gap-x-6 sm:gap-y-0",
            div {
                class: "flex items-center gap-2 py-0.5 sm:flex-[1_1_280px] sm:min-w-[280px] flex-nowrap",
                span {
                    class: "inline-flex items-center font-bold uppercase tracking-[0.06em] whitespace-nowrap leading-normal px-2 py-1 rounded-xl border border-current/30 border-l-4 border-current bg-current/14 min-h-[34px] text-sm max-[480px]:whitespace-normal max-[480px]:text-micro max-[480px]:px-1.5 max-[480px]:py-[2px] max-[480px]:min-h-0 shrink-0 text-wd-reference",
                    "{t(locale, TextKey::FooterPrograms)}"
                }
                ul {
                    class: "flex items-center gap-x-2.5 list-none m-0 p-0 min-w-0 overflow-x-auto whitespace-nowrap pb-1 scrollbar-hidden-x flex-nowrap",
                    li {
                        class: "shrink-0",
                        a {
                            class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-reference",
                            href: "https://github.com/cdk/depict",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "CDK Depict"
                        }
                    }
                    li {
                        class: "shrink-0",
                        a {
                            class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-reference",
                            href: "https://citation.js.org",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "Citation.js"
                        }
                    }
                    li {
                        class: "shrink-0",
                        a {
                            class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-reference",
                            href: "https://lifescience.opensource.epam.com/ketcher",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "Ketcher"
                        }
                    }
                    li {
                        class: "shrink-0",
                        a {
                            class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-reference",
                            href: "https://qlever.dev/wikidata",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "QLever SPARQL"
                        }
                    }
                    li {
                        class: "shrink-0",
                        a {
                            class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-reference",
                            href: "https://www.rdkitjs.com",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "RDKit.js"
                        }
                    }
                    li {
                        class: "shrink-0",
                        a {
                            class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-reference",
                            href: "https://doi.org/10.1186/s13321-018-0282-y",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "Sachem"
                        }
                    }
                }
            }
            FooterLicenseRow { locale }
        }
    }
}

#[component]
fn FooterCitationRow(locale: Locale) -> Element {
    rsx! {
        div {
            class: "flex items-center gap-2 py-0.5 sm:flex-[1_1_280px] sm:min-w-[280px] flex-nowrap",
            span {
                class: "inline-flex items-center font-bold uppercase tracking-[0.06em] whitespace-nowrap leading-normal px-2 py-1 rounded-xl border border-current/30 border-l-4 border-current bg-current/14 min-h-[34px] text-sm max-[480px]:whitespace-normal max-[480px]:text-micro max-[480px]:px-1.5 max-[480px]:py-[2px] max-[480px]:min-h-0 shrink-0 text-wd-compound",
                "{t(locale, TextKey::FooterCitation)}"
            }
            ul {
                class: "flex items-center gap-x-2.5 list-none m-0 p-0 min-w-0 overflow-x-auto whitespace-nowrap pb-1 scrollbar-hidden-x flex-nowrap",
                li {
                    class: "shrink-0",
                    a {
                        class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-compound",
                        href: "https://doi.org/10.7554/eLife.70780",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "LOTUS Article"
                    }
                }
                li {
                    class: "shrink-0",
                    a {
                        class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-compound",
                        href: asset!("/public/docs/references.bib"),
                        download: "references.bib",
                        "BibTeX"
                    }
                }
            }
        }
    }
}

#[component]
fn FooterLicenseRow(locale: Locale) -> Element {
    rsx! {
        div {
            class: "flex items-center gap-2 py-0.5 sm:flex-[1_1_280px] sm:min-w-[280px] flex-nowrap",
            span {
                class: "inline-flex items-center font-bold uppercase tracking-[0.06em] whitespace-nowrap leading-normal px-2 py-1 rounded-xl border border-current/30 border-l-4 border-current bg-current/14 min-h-[34px] text-sm max-[480px]:whitespace-normal max-[480px]:text-micro max-[480px]:px-1.5 max-[480px]:py-[2px] max-[480px]:min-h-0 shrink-0 text-wd-entries",
                "{t(locale, TextKey::FooterLicense)}"
            }
            ul {
                class: "flex items-center gap-x-2.5 list-none m-0 p-0 min-w-0 overflow-x-auto whitespace-nowrap pb-1 scrollbar-hidden-x flex-nowrap",
                li {
                    class: "shrink-0",
                    a {
                        class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-entries",
                        href: "https://creativecommons.org/publicdomain/zero/1.0/",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "CC0 1.0"
                    }
                    span {
                        class: "text-subtle whitespace-nowrap text-micro",
                        "({t(locale, TextKey::FooterForData)})"
                    }
                }
                li {
                    class: "shrink-0",
                    a {
                        class: "no-underline text-ui leading-[1.45] min-h-[34px] inline-flex items-center px-2 py-1 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 max-[480px]:px-1.5 max-[480px]:py-[3px] max-[480px]:text-micro max-[480px]:min-h-[32px] font-medium text-wd-entries",
                        href: "https://www.gnu.org/licenses/agpl-3.0.html",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "AGPL-3.0"
                    }
                    span {
                        class: "text-subtle whitespace-nowrap text-micro",
                        "({t(locale, TextKey::FooterForCode)})"
                    }
                }
            }
        }
    }
}
