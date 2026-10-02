// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::Locale;

mod de;
mod en;
mod fr;
mod it;

/// Generates a locale-dispatch wrapper that forwards to each per-locale
/// submodule function.  Handles the four arity/return-type combinations used
/// by the curation i18n tables.
///
/// The per-locale functions are `pub(super)` and reachable only through the
/// wrapper this macro generates, which is the shape that would normally need a
/// `dead_code` suppression -- the call site is in a different module, so the lint
/// cannot see the use.
///
/// It does not need one. This file carried `#[allow(dead_code)]` on all four arms
/// and the lint turned out not to fire on any of them, on either target or in any
/// build configuration. The suppression had outlived whatever it was for, and the
/// reason it survived is that an `allow` reports nothing when it is unnecessary.
macro_rules! dispatch {
    // no args → &'static str
    ($name:ident) => {
        pub fn $name(locale: Locale) -> &'static str {
            match locale {
                Locale::En => en::$name(),
                Locale::Fr => fr::$name(),
                Locale::De => de::$name(),
                Locale::It => it::$name(),
            }
        }
    };
    // no args → String
    ($name:ident => String) => {
        pub fn $name(locale: Locale) -> String {
            match locale {
                Locale::En => en::$name(),
                Locale::Fr => fr::$name(),
                Locale::De => de::$name(),
                Locale::It => it::$name(),
            }
        }
    };
    // one &str arg → String
    ($name:ident, $arg:ident: &str -> String) => {
        pub fn $name(locale: Locale, $arg: &str) -> String {
            match locale {
                Locale::En => en::$name($arg),
                Locale::Fr => fr::$name($arg),
                Locale::De => de::$name($arg),
                Locale::It => it::$name($arg),
            }
        }
    };
    // one &str arg → &'static str
    ($name:ident, $arg:ident: &str -> &'static str) => {
        pub fn $name(locale: Locale, $arg: &str) -> &'static str {
            match locale {
                Locale::En => en::$name($arg),
                Locale::Fr => fr::$name($arg),
                Locale::De => de::$name($arg),
                Locale::It => it::$name($arg),
            }
        }
    };
    // one usize arg → String
    ($name:ident, $arg:ident: usize -> String) => {
        pub fn $name(locale: Locale, $arg: usize) -> String {
            match locale {
                Locale::En => en::$name($arg),
                Locale::Fr => fr::$name($arg),
                Locale::De => de::$name($arg),
                Locale::It => it::$name($arg),
            }
        }
    };
    // two usize args → String
    ($name:ident, $a:ident: usize, $b:ident: usize -> String) => {
        pub fn $name(locale: Locale, $a: usize, $b: usize) -> String {
            match locale {
                Locale::En => en::$name($a, $b),
                Locale::Fr => fr::$name($a, $b),
                Locale::De => de::$name($a, $b),
                Locale::It => it::$name($a, $b),
            }
        }
    };
}

dispatch!(heading_add_one_row);
dispatch!(heading_tsv_import);
dispatch!(heading_queued_rows);
dispatch!(heading_results);
dispatch!(heading_quickstatements);
dispatch!(heading_quickstatements_dependencies);
dispatch!(placeholder_molecule_name);
dispatch!(placeholder_taxon_optional);
dispatch!(placeholder_doi_optional);
dispatch!(button_add_row);
dispatch!(button_load_example_rows);
dispatch!(button_append_tsv_rows);
dispatch!(button_generate_quickstatements);
dispatch!(button_generating);
dispatch!(button_remove);
dispatch!(col_name);
dispatch!(col_action);
dispatch!(col_original_smiles);
dispatch!(col_canonical_smiles);
dispatch!(col_exact_mass);
dispatch!(col_status);
dispatch!(label_new_item);
dispatch!(hint_expected_tsv_headers);
dispatch!(hint_scroll_curation_results);
dispatch!(msg_two_step_hint);
dispatch!(button_second_pass);
dispatch!(msg_second_pass_running);
dispatch!(msg_second_pass_done);
dispatch!(msg_curation_rate_limited);
dispatch!(curation_badge_prerequisite_pending);
dispatch!(curation_badge_mass_missing);
dispatch!(curation_badge_second_pass_required);
dispatch!(curation_mass_warning_title);
dispatch!(msg_delay_advice);
dispatch!(curation_qs_dev_label);
dispatch!(curation_qs_dev_prereq_hint);
dispatch!(curation_qs_dev_main_hint);
dispatch!(curation_note_existing_complete);
dispatch!(curation_note_existing_updates);
dispatch!(curation_note_new_compound);
dispatch!(curation_note_dependencies_pending);
dispatch!(view_switch_aria);
dispatch!(view_label_explorer);
dispatch!(view_label_curation_explorer);
dispatch!(view_label_draw);

dispatch!(msg_name_smiles_required => String);
dispatch!(msg_duplicate_row_skipped => String);
dispatch!(msg_no_valid_tsv_rows => String);
dispatch!(msg_tsv_missing_column, column: &str -> String);
dispatch!(msg_add_row_before_generate => String);
dispatch!(msg_running_checks => String);
dispatch!(msg_done_review_copy => String);
dispatch!(msg_prerequisites_pending, count: usize -> String);
dispatch!(msg_second_pass_still_pending_count, count: usize -> String);
dispatch!(curation_pending_taxon, taxon: &str -> String);
dispatch!(curation_pending_reference, doi: &str -> String);
dispatch!(msg_curation_failed, detail: &str -> String);
dispatch!(msg_tsv_import_complete, added: usize, skipped: usize -> String);
dispatch!(msg_examples_loaded, added: usize, skipped: usize -> String);
dispatch!(curation_status_label, status_key: &str -> &'static str);
