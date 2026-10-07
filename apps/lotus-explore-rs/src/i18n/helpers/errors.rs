// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::Locale;
use crate::features::explore::truncation::Reason;

mod de;
mod en;
mod fr;
mod it;

/// The per-locale functions are reachable only through the wrapper this macro
/// generates, normally the shape that wants a `dead_code` suppression. It needs
/// none: this file carried `#[allow(dead_code)]` on all five arms and the lint
/// fired on none of them on either target. An unnecessary `allow` reports nothing,
/// which is how a suppression outlives whatever it was for.
macro_rules! dispatch {
    // no args → String
    ($name:ident) => {
        pub fn $name(locale: Locale) -> String {
            match locale {
                Locale::En => en::$name(),
                Locale::Fr => fr::$name(),
                Locale::De => de::$name(),
                Locale::It => it::$name(),
            }
        }
    };
    // no args → String (with cfg)
    ($name:ident with_cfg $cfg:literal) => {
        #[$cfg]
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
    ($name:ident, $arg:ident: &str) => {
        pub fn $name(locale: Locale, $arg: &str) -> String {
            match locale {
                Locale::En => en::$name($arg),
                Locale::Fr => fr::$name($arg),
                Locale::De => de::$name($arg),
                Locale::It => it::$name($arg),
            }
        }
    };
    // one `Reason` arg → String
    ($name:ident, reason: Reason) => {
        pub fn $name(locale: Locale, reason: Reason) -> String {
            match locale {
                Locale::En => en::$name(reason),
                Locale::Fr => fr::$name(reason),
                Locale::De => de::$name(reason),
                Locale::It => it::$name(reason),
            }
        }
    };
    // two &str args → String
    ($name:ident, $a:ident: &str, $b:ident: &str) => {
        pub fn $name(locale: Locale, $a: &str, $b: &str) -> String {
            match locale {
                Locale::En => en::$name($a, $b),
                Locale::Fr => fr::$name($a, $b),
                Locale::De => de::$name($a, $b),
                Locale::It => it::$name($a, $b),
            }
        }
    };
    // three &str args → String
    ($name:ident, $a:ident: &str, $b:ident: &str, $c:ident: &str) => {
        pub fn $name(locale: Locale, $a: &str, $b: &str, $c: &str) -> String {
            match locale {
                Locale::En => en::$name($a, $b, $c),
                Locale::Fr => fr::$name($a, $b, $c),
                Locale::De => de::$name($a, $b, $c),
                Locale::It => it::$name($a, $b, $c),
            }
        }
    };
}

dispatch!(err_invalid_search_input);
dispatch!(err_api_not_configured);
dispatch!(err_taxon_too_long);
dispatch!(err_structure_too_long);
dispatch!(err_mass_out_of_range);
dispatch!(err_mass_range_invalid);
dispatch!(err_year_out_of_range);
dispatch!(err_year_range_invalid);
dispatch!(err_element_count_too_high);
dispatch!(err_similarity_threshold_invalid);
dispatch!(warn_wdqs_fallback);

dispatch!(err_unsupported_format, fmt: &str);
dispatch!(err_taxon_parse_failed, detail: &str);
dispatch!(err_compound_not_found, input: &str);
dispatch!(err_taxon_not_found, taxon: &str);
dispatch!(err_reference_not_found, input: &str);
dispatch!(err_reference_not_an_identifier, input: &str);

dispatch!(err_query_stage_failed, stage: &str, detail: &str);
dispatch!(err_truncated_by_endpoint, reason: Reason);

dispatch!(warn_input_standardized, original: &str, normalized: &str);
dispatch!(warn_taxon_common_name, name: &str, qid: &str);
dispatch!(warn_compound_resolved, label: &str, qid: &str);
dispatch!(warn_ambiguous_taxon, best_name: &str, best_qid: &str, names: &str);
dispatch!(warn_ambiguous_compound, best_name: &str, best_qid: &str, names: &str);

#[cfg(target_arch = "wasm32")]
pub fn error_hint_memory(locale: Locale) -> &'static str {
    match locale {
        Locale::En => en::error_hint_memory(),
        Locale::Fr => fr::error_hint_memory(),
        Locale::De => de::error_hint_memory(),
        Locale::It => it::error_hint_memory(),
    }
}
