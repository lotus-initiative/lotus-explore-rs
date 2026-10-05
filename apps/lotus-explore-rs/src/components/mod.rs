// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The Dioxus components, grouped by which page or surface they belong to.
//!
//! Grouped by surface rather than by widget: `results_table` is a thing a reader
//! uses, `form_inputs` is a set of things, and a reader looking for "where does the
//! row height come from" wants the first and would not look in the second.
//!
//! # Component length
//!
//! Components here run past the 40-line mark that applies elsewhere, which is the
//! shape of Dioxus: the `rsx!` block is one expression, so a `Footer` of 156 lines of
//! markup is not 156 lines of logic, and splitting it would add three props and three
//! files to say the same thing.
//!
//! Logic inside the markup is not forgiven. A component that fetches, formats and
//! renders is three: the fetch goes to `features/`, the formatting to a pure function
//! beside the file, and the component keeps only the state and the `rsx!`. The line
//! count is the symptom, so the job is what gets checked.
pub mod copy_button;
pub mod curation_results_table;
pub mod data_curation_page;
pub mod faq;
pub mod form_sections;
pub mod landing;
pub mod layout;
pub mod loading;
pub mod results_table;
pub mod results_viewport;
pub mod search_panel;
pub mod welcome;

pub mod form_inputs;
pub mod ui;
