// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! User-facing formatting for domain errors and warnings.

use crate::features::explore::{
    DomainError, ErrorKind, LookupNotice, ParseFault, QueryStage, ValidationFault,
};
#[cfg(target_arch = "wasm32")]
use crate::i18n::error_hint_memory;
use crate::i18n::{
    Locale, TextKey, err_api_not_configured, err_compound_not_found, err_element_count_too_high,
    err_invalid_search_input, err_mass_out_of_range, err_mass_range_invalid,
    err_query_stage_failed, err_reference_not_an_identifier, err_reference_not_found,
    err_similarity_threshold_invalid, err_structure_too_long, err_taxon_not_found,
    err_taxon_parse_failed, err_taxon_too_long, err_unsupported_format, err_year_out_of_range,
    err_year_range_invalid, t, warn_ambiguous_compound, warn_ambiguous_taxon,
    warn_compound_resolved, warn_input_standardized, warn_taxon_common_name, warn_unconstrained,
    warn_wdqs_fallback,
};
use crate::repositories::RepositoryError;

pub fn format_domain_error(locale: Locale, err: &DomainError) -> String {
    match err {
        DomainError::Validation(v) => format_validation_fault(locale, v),
        DomainError::Transport { stage, source } => format_transport_fault(locale, *stage, source),
        DomainError::Parse(p) => format_parse_fault(locale, p),
        #[cfg(target_arch = "wasm32")]
        DomainError::MemoryLimit { .. } => error_hint_memory(locale).into(),
    }
}

/// One string per notice, in the order they apply.
///
/// A list because a taxon name can raise two: `bacteria` is both spelled
/// differently from `Bacteria` and ambiguous, and both are true. Returning a
/// single string is what made one of them disappear, depending on which code
/// path produced it.
pub fn format_taxon_warnings(locale: Locale, warnings: &[LookupNotice]) -> Vec<String> {
    warnings
        .iter()
        .map(|w| format_taxon_warning(locale, w))
        .collect()
}

fn format_taxon_warning(locale: Locale, warning: &LookupNotice) -> String {
    match warning {
        LookupNotice::Standardized {
            original,
            standardized,
        } => warn_input_standardized(locale, original, standardized),
        LookupNotice::CommonName {
            chosen_name,
            chosen_qid,
        } => warn_taxon_common_name(locale, chosen_name, chosen_qid),
        LookupNotice::CompoundResolved {
            chosen_label,
            chosen_qid,
        } => warn_compound_resolved(locale, chosen_label, chosen_qid),
        LookupNotice::AmbiguousTaxon {
            chosen_name,
            chosen_qid,
            candidates,
        } => warn_ambiguous_taxon(locale, chosen_name, chosen_qid, &candidates.join(", ")),
        LookupNotice::AmbiguousCompound {
            chosen_name,
            chosen_qid,
            candidates,
        } => warn_ambiguous_compound(locale, chosen_name, chosen_qid, &candidates.join(", ")),
        LookupNotice::ApiMessage(msg) => msg.clone(),
        LookupNotice::WdqsFallback => warn_wdqs_fallback(locale),
        LookupNotice::Unconstrained => warn_unconstrained(locale),
    }
}

pub fn error_hint_text(locale: Locale, kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Validation => t(locale, TextKey::ErrorHintValidation),
        ErrorKind::Configuration => t(locale, TextKey::ErrorHintConfiguration),
        ErrorKind::BadRequest => t(locale, TextKey::ErrorHintBadRequest),
        ErrorKind::Network => t(locale, TextKey::ErrorHintNetwork),
        ErrorKind::RateLimit => t(locale, TextKey::ErrorHintRateLimit),
        ErrorKind::QueryTooExpensive => t(locale, TextKey::ErrorHintQueryTooExpensive),
        ErrorKind::Parse => t(locale, TextKey::ErrorHintParse),
        ErrorKind::Truncated => t(locale, TextKey::ErrorHintTruncated),
        #[cfg(target_arch = "wasm32")]
        ErrorKind::Memory => "",
        ErrorKind::Unknown => t(locale, TextKey::ErrorHintUnknown),
    }
}

fn format_transport_fault(locale: Locale, stage: QueryStage, source: &RepositoryError) -> String {
    let detail = transport_error_summary(locale, source);
    let stage_label = stage_display_label(locale, stage);
    err_query_stage_failed(locale, stage_label, &detail)
}

fn stage_display_label(locale: Locale, stage: QueryStage) -> &'static str {
    match stage {
        QueryStage::TaxonSearch => t(locale, TextKey::StageTaxonSearch),
        QueryStage::ResultsQuery => t(locale, TextKey::StageResultsQuery),
    }
}

fn transport_error_summary(locale: Locale, source: &RepositoryError) -> String {
    let raw = match source {
        RepositoryError::NotConfigured => return err_api_not_configured(locale),
        RepositoryError::Network(detail) | RepositoryError::Parse(detail) => detail.as_ref(),
        // Summarised as its own sentence rather than passed through: the raw
        // message leads with a byte count, which reads as a stray number to
        // anyone who has not just watched the transfer stall.
        RepositoryError::Truncated(_) => return t(locale, TextKey::ErrorHintTruncated).to_string(),
        RepositoryError::Http { status, body } => {
            let detail = if looks_like_html(body) {
                if *status == 429 {
                    "Too many requests from upstream service".into()
                } else {
                    "Upstream service returned an HTML error page".into()
                }
            } else {
                compact_error_text(body)
            };
            return format!("HTTP {status}: {detail}");
        }
    };
    compact_error_text(raw)
}

fn looks_like_html(msg: &str) -> bool {
    let head = msg.trim_start();
    if head.is_empty() {
        return false;
    }
    // Sample at most 256 bytes (ASCII-safe) to avoid scanning huge payloads
    let sample = &head[..head.len().min(256)];
    // Case-insensitive prefix check without heap allocation
    let lc: String = sample.to_ascii_lowercase();
    lc.starts_with("<!doctype html")
        || lc.starts_with("<html")
        || lc.contains("<html")
        || lc.contains("<body")
}

fn compact_error_text(msg: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(msg)
        && let Some(exception) = value.get("exception").and_then(|v| v.as_str())
    {
        return truncate_for_notice(exception);
    }

    let trimmed = msg.lines().next().unwrap_or(msg).trim();
    truncate_for_notice(trimmed)
}

fn truncate_for_notice(text: &str) -> String {
    const MAX_CHARS: usize = 220;
    let Some((end, _)) = text.char_indices().nth(MAX_CHARS) else {
        return text.to_string();
    };
    let mut result = String::with_capacity(end + 3);
    result.push_str(&text[..end]);
    result.push('…');
    result
}

fn format_validation_fault(locale: Locale, fault: &ValidationFault) -> String {
    match fault {
        ValidationFault::EmptyInput => err_invalid_search_input(locale),
        ValidationFault::TaxonTooLong => err_taxon_too_long(locale),
        ValidationFault::StructureTooLong => err_structure_too_long(locale),
        ValidationFault::MassOutOfRange => err_mass_out_of_range(locale),
        ValidationFault::MassRangeInvalid => err_mass_range_invalid(locale),
        ValidationFault::YearOutOfRange => err_year_out_of_range(locale),
        ValidationFault::YearRangeInvalid => err_year_range_invalid(locale),
        ValidationFault::ElementCountTooHigh => err_element_count_too_high(locale),
        ValidationFault::SimilarityThresholdInvalid => err_similarity_threshold_invalid(locale),
        ValidationFault::CompoundNotFound { input } => err_compound_not_found(locale, input),
        ValidationFault::TaxonNotFound { input } => err_taxon_not_found(locale, input),
        ValidationFault::ReferenceNotFound { input } => err_reference_not_found(locale, input),
        ValidationFault::ReferenceNotAnIdentifier { input } => {
            err_reference_not_an_identifier(locale, input)
        }
        ValidationFault::UnsupportedFormat { format } => err_unsupported_format(locale, format),
    }
}

fn format_parse_fault(locale: Locale, fault: &ParseFault) -> String {
    match fault {
        ParseFault::CompoundCsv { details } | ParseFault::TaxonPick { details } => {
            err_query_stage_failed(
                locale,
                stage_display_label(locale, QueryStage::TaxonSearch),
                &compact_error_text(details),
            )
        }
        ParseFault::TaxonCsv { details } => {
            err_taxon_parse_failed(locale, &compact_error_text(details))
        }
        ParseFault::ResultsCsv { details } => err_query_stage_failed(
            locale,
            stage_display_label(locale, QueryStage::ResultsQuery),
            &compact_error_text(details),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_lookup_notices_are_formatted_and_neither_is_dropped() {
        // `bacteria` is two true things at once: spelled differently from the
        // name it matched, and matched by more than one taxon. Formatting takes
        // a list precisely so neither can be the one that gets lost.
        let lines = format_taxon_warnings(
            Locale::En,
            &[
                LookupNotice::Standardized {
                    original: "bacteria".into(),
                    standardized: "Bacteria".into(),
                },
                LookupNotice::AmbiguousTaxon {
                    chosen_name: "Bacteria".into(),
                    chosen_qid: "Q10876".into(),
                    candidates: vec!["Bacteria (Q10876)".into(), "Bacteria (Q4034791)".into()],
                },
            ],
        );

        assert_eq!(lines.len(), 2, "one line per notice: {lines:?}");
        let joined = lines.join("\n");
        assert!(
            joined.contains("bacteria") && joined.contains("Bacteria"),
            "{joined}"
        );
        assert!(
            joined.contains("Q10876") && joined.contains("Q4034791"),
            "{joined}"
        );
    }

    #[test]
    fn a_compound_ambiguity_does_not_call_itself_a_taxon() {
        // The bug this pins: `c1ccccc1` resolves to several compounds, and the
        // notice said "Ambiguous taxon name" because both lookups shared one
        // variant and the presenter had the taxon wording hardcoded for it. The
        // reader was pointed at the field they did not type into.
        let lines = format_taxon_warnings(
            Locale::En,
            &[LookupNotice::AmbiguousCompound {
                chosen_name: "benzene".into(),
                chosen_qid: "Q2270".into(),
                candidates: vec![
                    "benzene (Q2270)".into(),
                    "deuterated benzene (Q1101314)".into(),
                ],
            }],
        );
        let joined = lines.join("\n");
        assert!(!joined.to_lowercase().contains("taxon"), "{joined}");
        assert!(joined.contains("compound"), "{joined}");
        assert!(joined.contains("Q2270"), "{joined}");
        assert!(joined.contains("deuterated benzene (Q1101314)"), "{joined}");
    }

    #[test]
    fn each_ambiguity_names_its_own_kind_of_thing_in_every_locale() {
        // The sentence is not the English one in the other three, so asserting
        // English phrasing there would pass for the wrong reason. What has to hold
        // everywhere is that each names a taxon or a compound, and not the other.
        for locale in [Locale::En, Locale::Fr, Locale::De, Locale::It] {
            let taxon = format_taxon_warnings(
                locale,
                &[LookupNotice::AmbiguousTaxon {
                    chosen_name: "Bacteria".into(),
                    chosen_qid: "Q10876".into(),
                    candidates: vec!["Bacteria (Q10876)".into()],
                }],
            )
            .join("\n");
            let compound = format_taxon_warnings(
                locale,
                &[LookupNotice::AmbiguousCompound {
                    chosen_name: "benzene".into(),
                    chosen_qid: "Q2270".into(),
                    candidates: vec!["benzene (Q2270)".into()],
                }],
            )
            .join("\n");

            assert_ne!(taxon, compound, "{locale:?}: the two read the same");
            assert!(
                !taxon.to_lowercase().contains("compound"),
                "{locale:?}: {taxon}"
            );
            assert!(
                !compound.to_lowercase().contains("taxon"),
                "{locale:?}: {compound}"
            );
        }
    }

    #[test]
    fn no_lookup_notices_formats_to_no_lines() {
        assert_eq!(format_taxon_warnings(Locale::En, &[]).len(), 0);
    }

    #[test]
    fn compact_error_text_uses_exception_from_json() {
        let payload = r#"{"exception":"Upstream service returned HTTP 500","query":"SELECT ..."}"#;
        assert_eq!(
            compact_error_text(payload),
            "Upstream service returned HTTP 500"
        );
    }

    #[test]
    fn transport_error_summary_truncates_long_network_message() {
        let long = "x".repeat(400);
        let summary = transport_error_summary(Locale::En, &RepositoryError::network(long));
        assert!(summary.chars().count() <= 221);
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn transport_error_summary_localizes_not_configured() {
        let en = transport_error_summary(Locale::En, &RepositoryError::NotConfigured);
        let fr = transport_error_summary(Locale::Fr, &RepositoryError::NotConfigured);
        // "configured" (EN) and "configurée" (FR) both share the ASCII stem "config".
        // `to_ascii_lowercase` does not strip accents, so checking for "configure" would
        // miss the French past-participle "configurée" whose 'é' is non-ASCII.
        assert!(en.to_ascii_lowercase().contains("config"));
        assert!(fr.to_ascii_lowercase().contains("config"));
    }

    #[test]
    fn format_domain_error_renders_new_validation_faults() {
        let err = DomainError::Validation(ValidationFault::YearRangeInvalid);
        let rendered = format_domain_error(Locale::En, &err);
        assert!(rendered.contains("Year"));
    }

    #[test]
    fn error_hint_for_http_4xx_is_bad_request_not_network() {
        let err = DomainError::transport(
            QueryStage::ResultsQuery,
            RepositoryError::Http {
                status: 400,
                body: "Invalid SPARQL query".to_string(),
            },
        );

        assert_eq!(
            error_hint_text(Locale::En, err.kind()),
            "The server rejected the request. Check your search parameters."
        );
    }

    #[test]
    fn error_hint_for_transport_parse_uses_parse_hint() {
        let err = DomainError::transport(
            QueryStage::ResultsQuery,
            RepositoryError::parse("csv parse failed"),
        );

        assert_eq!(
            error_hint_text(Locale::En, err.kind()),
            t(Locale::En, TextKey::ErrorHintParse)
        );
    }

    #[test]
    fn error_hint_for_not_configured_uses_configuration_hint() {
        let err = DomainError::transport(QueryStage::ResultsQuery, RepositoryError::NotConfigured);

        assert_eq!(
            error_hint_text(Locale::En, err.kind()),
            t(Locale::En, TextKey::ErrorHintConfiguration)
        );
    }

    #[test]
    fn transport_error_summary_replaces_html_payloads_with_readable_text() {
        let summary = transport_error_summary(
            Locale::En,
            &RepositoryError::Http {
                status: 503,
                body: "<html><body>Service unavailable</body></html>".to_string(),
            },
        );

        assert_eq!(
            summary,
            "HTTP 503: Upstream service returned an HTML error page"
        );
    }

    #[test]
    fn error_hint_for_rate_limit_uses_rate_limit_hint() {
        let err = DomainError::transport(
            QueryStage::ResultsQuery,
            RepositoryError::Http {
                status: 429,
                body: "<html><body>Too many requests</body></html>".to_string(),
            },
        );

        assert_eq!(
            error_hint_text(Locale::En, err.kind()),
            t(Locale::En, TextKey::ErrorHintRateLimit)
        );
    }

    #[test]
    fn a_cancelled_query_is_advised_to_narrow_not_to_retry() {
        // The hint is the user-facing half of the change: every other failure in
        // this file ends by suggesting a retry, and for this one a retry is
        // precisely the wrong advice. The endpoint's budget was spent once
        // already; a second click spends it again.
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::Http {
                status: 429,
                body: "Operation timed out. Last operation: Sort (internal order) on ?r"
                    .to_string(),
            },
        };

        assert_eq!(err.kind(), ErrorKind::QueryTooExpensive);
        let hint = error_hint_text(Locale::En, err.kind());
        assert_eq!(hint, t(Locale::En, TextKey::ErrorHintQueryTooExpensive));
        assert!(
            !hint.to_lowercase().contains("retry"),
            "the hint must not suggest a retry: {hint}"
        );
    }
}
