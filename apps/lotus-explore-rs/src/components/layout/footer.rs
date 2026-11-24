// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

// Dioxus's `asset!` macro resolves to `&[u8]`, flagged by `volatile_composites`
// at every call site; the value type is fixed by the framework.
#![allow(
    clippy::volatile_composites,
    reason = "expands from Dioxus's `asset!` macro, which reads bytes through a volatile pointer; the shape is fixed by the macro"
)]
//! Site footer component shared across pages.

use crate::hooks::use_locale::use_locale;
use crate::i18n::{Locale, TextKey, t};
use dioxus::prelude::*;

const FOOTER_PAIR_CLASS: &str = "flex flex-col gap-3 py-1 border-b border-border last:border-b-0 sm:flex-row sm:items-start sm:gap-x-6 sm:gap-y-0";
const FOOTER_GROUP_CLASS: &str =
    "flex flex-wrap items-start gap-2 py-0.5 sm:flex-[1_1_280px] sm:min-w-[280px]";
const FOOTER_BADGE_BASE: &str = "inline-flex items-center font-bold uppercase tracking-[0.06em] leading-normal px-2 py-1 rounded-xl border border-current/30 border-l-4 border-current bg-current/14 min-h-[36px] text-ui shrink-0 mt-1";
const FOOTER_UL_CLASS: &str =
    "flex flex-1 min-w-0 flex-wrap items-center gap-x-2 gap-y-1 list-none m-0 p-0";
const FOOTER_LINK_BASE: &str = "no-underline text-ui leading-snug min-h-[44px] min-w-[44px] inline-flex items-center px-2.5 py-2 rounded-xl hover:underline hover:bg-current/8 focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 font-medium";

#[component]
pub fn Footer() -> Element {
    let locale = use_locale();
    rsx! {
        div {
            class: FOOTER_PAIR_CLASS,
            div {
                class: FOOTER_GROUP_CLASS,
                span {
                    class: "{FOOTER_BADGE_BASE} text-wd-compound",
                    "{t(locale, TextKey::FooterArchive)}"
                }
                ul {
                    class: FOOTER_UL_CLASS,
                    li {
                        a {
                            class: "{FOOTER_LINK_BASE} text-wd-compound",
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
            class: FOOTER_PAIR_CLASS,
            div {
                class: FOOTER_GROUP_CLASS,
                span {
                    class: "{FOOTER_BADGE_BASE} text-wd-taxon",
                    "{t(locale, TextKey::FooterCode)}"
                }
                ul {
                    class: FOOTER_UL_CLASS,
                    li {
                        a {
                            class: "{FOOTER_LINK_BASE} text-wd-taxon",
                            href: env!("CARGO_PKG_REPOSITORY"),
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "lotus-explore-rs"
                        }
                    }
                }
            }
            div {
                class: FOOTER_GROUP_CLASS,
                span {
                    class: "{FOOTER_BADGE_BASE} text-wd-taxon",
                    "{t(locale, TextKey::FooterData)}"
                }
                ul {
                    class: FOOTER_UL_CLASS,
                    li {
                        a {
                            class: "{FOOTER_LINK_BASE} text-wd-taxon",
                            href: "https://www.wikidata.org/wiki/Q104225190",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "LOTUS Initiative"
                        }
                    }
                    li {
                        a {
                            class: "{FOOTER_LINK_BASE} text-wd-taxon",
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
            class: FOOTER_PAIR_CLASS,
            div {
                class: FOOTER_GROUP_CLASS,
                span {
                    class: "{FOOTER_BADGE_BASE} text-wd-reference",
                    "{t(locale, TextKey::FooterPrograms)}"
                }
                ul {
                    class: FOOTER_UL_CLASS,
                    li {
                        a {
                            class: "{FOOTER_LINK_BASE} text-wd-reference",
                            href: "https://github.com/cdk/depict",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "CDK Depict"
                        }
                    }
                    li {
                        a {
                            class: "{FOOTER_LINK_BASE} text-wd-reference",
                            href: "https://citation.js.org",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "Citation.js"
                        }
                    }
                    li {
                        a {
                            class: "{FOOTER_LINK_BASE} text-wd-reference",
                            href: "https://lifescience.opensource.epam.com/ketcher",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "Ketcher"
                        }
                    }
                    li {
                        a {
                            class: "{FOOTER_LINK_BASE} text-wd-reference",
                            href: "https://qlever.dev/wikidata",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "QLever SPARQL"
                        }
                    }
                    li {
                        a {
                            class: "{FOOTER_LINK_BASE} text-wd-reference",
                            href: "https://www.rdkitjs.com",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "RDKit.js"
                        }
                    }
                    li {
                        a {
                            class: "{FOOTER_LINK_BASE} text-wd-reference",
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
            class: FOOTER_GROUP_CLASS,
            span {
                class: "{FOOTER_BADGE_BASE} text-wd-compound",
                "{t(locale, TextKey::FooterCitation)}"
            }
            ul {
                class: FOOTER_UL_CLASS,
                li {
                    a {
                        class: "{FOOTER_LINK_BASE} text-wd-compound",
                        href: "https://doi.org/10.7554/eLife.70780",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "LOTUS Article"
                    }
                }
                li {
                    a {
                        class: "{FOOTER_LINK_BASE} text-wd-compound",
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
            class: FOOTER_GROUP_CLASS,
            span {
                class: "{FOOTER_BADGE_BASE} text-wd-entries",
                "{t(locale, TextKey::FooterLicense)}"
            }
            ul {
                class: FOOTER_UL_CLASS,
                li {
                    class: "inline-flex flex-wrap items-center gap-x-1",
                    a {
                        class: "{FOOTER_LINK_BASE} text-wd-entries",
                        href: "https://creativecommons.org/publicdomain/zero/1.0/",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "CC0 1.0"
                    }
                    span {
                        class: "text-subtle text-ui",
                        "({t(locale, TextKey::FooterForData)})"
                    }
                }
                li {
                    class: "inline-flex flex-wrap items-center gap-x-1",
                    a {
                        class: "{FOOTER_LINK_BASE} text-wd-entries",
                        href: "https://www.gnu.org/licenses/agpl-3.0.html",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "AGPL-3.0"
                    }
                    span {
                        class: "text-subtle text-ui",
                        "({t(locale, TextKey::FooterForCode)})"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "footer/tests.rs"]
mod tests;
