// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Numeric/identifier cells for results-table rows (mass, formula, publication year).

use dioxus::prelude::*;

pub fn mass_cell(mass: Option<f64>) -> Element {
    rsx! {
        td { class: "px-3 py-2.5 align-top text-right",
            if let Some(m) = mass {
                span { "property": "wdt:P2062", class: "text-ui font-medium tabular-nums text-wd-compound", "{format_mass_value(m)}" }
            } else {
                span { class: "text-subtle", "-" }
            }
        }
    }
}

pub fn formula_cell(formula: Option<&str>) -> Element {
    rsx! {
        td { class: "px-3 py-2.5 align-top text-right",
            if let Some(f) = formula {
                span { "property": "wdt:P274", class: "text-ui font-medium font-mono whitespace-nowrap text-wd-compound", "{f}" }
            } else {
                span { class: "text-subtle", "-" }
            }
        }
    }
}

pub fn year_cell(pub_year: Option<i16>) -> Element {
    rsx! {
        td { class: "min-w-[6ch] px-3 py-2.5 align-top text-right whitespace-nowrap",
            if let Some(y) = pub_year {
                time { datetime: "{y}", "property": "datePublished", class: "text-ui font-medium text-wd-reference", "{y}" }
            } else {
                span { class: "text-subtle", "-" }
            }
        }
    }
}

fn format_mass_value(mass: f64) -> String {
    format!("{mass:.4}")
}

#[cfg(test)]
#[path = "numeric/tests.rs"]
mod tests;
