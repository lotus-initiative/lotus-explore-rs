// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! User-facing formatting for domain errors and warnings.

use crate::features::explore::truncation::{Reason, truncation_reason};
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
    err_taxon_parse_failed, err_taxon_too_long, err_truncated_by_endpoint, err_unsupported_format,
    err_year_out_of_range, err_year_range_invalid, t, warn_ambiguous_compound,
    warn_ambiguous_taxon, warn_compound_resolved, warn_input_standardized, warn_taxon_common_name,
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
/// differently from `Bacteria` and ambiguous, and both are true. Returning one
/// string is what made one disappear, depending on the code path.
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
    // A truncation is not a parse failure to a reader, and the detail is an
    // English sentence from `lotus-query` -- so a translated frame with an
    // English sentence inside it, and the wrong advice. Recognised here through
    // the same helper the classifier uses, because the two must not disagree.
    let truncated = match fault {
        ParseFault::CompoundCsv { details }
        | ParseFault::TaxonPick { details }
        | ParseFault::TaxonCsv { details }
        | ParseFault::ResultsCsv { details } => truncation_reason(details),
    };
    if let Some(reason) = truncated {
        return err_truncated_by_endpoint(locale, Reason::of(reason));
    }
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
#[path = "error_presenter/tests.rs"]
mod tests;
