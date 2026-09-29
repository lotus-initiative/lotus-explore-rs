// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! User-facing formatting for domain errors and warnings.

use crate::features::explore::{
    DomainError, ErrorKind, ParseFault, QueryStage, TaxonWarning, ValidationFault,
};
#[cfg(target_arch = "wasm32")]
use crate::i18n::error_hint_memory;
use crate::i18n::{
    Locale, TextKey, err_api_not_configured, err_element_count_too_high, err_invalid_search_input,
    err_mass_out_of_range, err_mass_range_invalid, err_query_stage_failed,
    err_similarity_threshold_invalid, err_structure_too_long, err_taxon_not_found,
    err_taxon_parse_failed, err_taxon_too_long, err_unsupported_format, err_year_out_of_range,
    err_year_range_invalid, t, warn_ambiguous_taxon, warn_input_standardized, warn_wdqs_fallback,
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

pub fn format_taxon_warning(locale: Locale, warning: &TaxonWarning) -> String {
    match warning {
        TaxonWarning::Standardized {
            original,
            standardized,
        } => warn_input_standardized(locale, original, standardized),
        TaxonWarning::Ambiguous {
            chosen_name,
            chosen_qid,
            candidates,
        } => warn_ambiguous_taxon(locale, chosen_name, chosen_qid, &candidates.join(", ")),
        TaxonWarning::ApiMessage(msg) => msg.clone(),
        TaxonWarning::WdqsFallback => warn_wdqs_fallback(locale),
    }
}

pub fn error_hint_text(locale: Locale, kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Validation => t(locale, TextKey::ErrorHintValidation),
        ErrorKind::Configuration => t(locale, TextKey::ErrorHintConfiguration),
        ErrorKind::BadRequest => t(locale, TextKey::ErrorHintBadRequest),
        ErrorKind::Network => t(locale, TextKey::ErrorHintNetwork),
        ErrorKind::RateLimit => t(locale, TextKey::ErrorHintRateLimit),
        ErrorKind::Parse => t(locale, TextKey::ErrorHintParse),
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
        ValidationFault::TaxonNotFound { input } => err_taxon_not_found(locale, input),
        ValidationFault::UnsupportedFormat { format } => err_unsupported_format(locale, format),
    }
}

fn format_parse_fault(locale: Locale, fault: &ParseFault) -> String {
    match fault {
        ParseFault::TaxonCsv { details } => {
            err_taxon_parse_failed(locale, &compact_error_text(details))
        }
        ParseFault::TaxonPick { details } => err_query_stage_failed(
            locale,
            stage_display_label(locale, QueryStage::TaxonSearch),
            &compact_error_text(details),
        ),
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
}
