// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Turning a [`SearchCriteria`] into a [`SearchResult`].
//!
//! This is the sequence the web app, the CLI and the API server all need, and
//! which the repository previously implemented three times: resolve the taxon,
//! build the query, apply the filters, execute, parse, count.
//!
//! Each step is a separate function so a caller can stop early — the web app
//! needs the query on its own for the "download the query" button, and the
//! server needs the counts without the rows.

use crate::client::Http;
use crate::error::{FetchError, ResponseFormat};
use crate::result::{SearchRequest, SearchResult, TaxonNote, TaxonResolution};
use lotus_model::{DatasetStats, SearchCriteria, SmilesSearchType, ValidationError};
use lotus_query::{
    Nomenclature, all_compounds_query, compounds_by_taxon_query_with, counts_query, limit_query,
    structure_search_query_with, taxon_lookup_query, with_filters,
};
use lotus_query::{parse_compounds_csv_capped, parse_counts_csv, parse_taxon_csv};

/// How a structure search should be run, once the structure is known.
#[derive(Debug, Clone, PartialEq)]
pub struct StructurePlan {
    /// The normalised structure text.
    pub structure: String,
    /// The search to run, as asked for. A molfile takes either mode: the
    /// similarity service was measured accepting a multi-line CTAB literal and
    /// answering a cutoff search, so the format does not decide the mode.
    pub search: SmilesSearchType,
    /// Tanimoto cutoff, when `search` is a similarity search.
    pub threshold: f64,
    /// Whether the structure is a molfile rather than a SMILES string.
    pub is_molfile: bool,
}

impl StructurePlan {
    /// Whether the request is a structure search at all.
    #[must_use]
    pub fn for_request(criteria: &SearchCriteria) -> Option<Self> {
        let structure = normalize_structure(&criteria.structure);
        if structure.is_empty() {
            return None;
        }
        let is_molfile = lotus_model::classify_structure(&structure).is_molfile();
        Some(Self {
            structure,
            search: criteria.structure_search,
            threshold: criteria.structure_threshold,
            is_molfile,
        })
    }
}

/// Unify line endings, and trim anything that is not a molfile.
#[must_use]
pub fn normalize_structure(value: &str) -> String {
    let unified = if value.contains('\r') {
        value.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        value.to_string()
    };
    if lotus_model::classify_structure(&unified).is_molfile() {
        unified
    } else {
        unified.trim().to_string()
    }
}

/// The base query for a criteria and an already-resolved taxon.
///
/// `qid` is [`TaxonResolution::qid`], and the wildcard `*` means every taxon.
#[must_use]
pub fn build_base_query(criteria: &SearchCriteria, qid: Option<&str>) -> String {
    let nomenclature = Nomenclature::from(criteria);
    match StructurePlan::for_request(criteria) {
        Some(plan) => {
            let taxon = match qid {
                // The wildcard has no Wikidata entity of its own, so it filters
                // on the root taxon, from which `P171*` reaches everything.
                Some("*") => Some("Q2382443"),
                other => other,
            };
            structure_search_query_with(
                &plan.structure,
                plan.search,
                plan.threshold,
                taxon,
                &nomenclature,
            )
        }
        None => match qid {
            Some(qid) if qid != "*" => compounds_by_taxon_query_with(qid, &nomenclature),
            _ => all_compounds_query(),
        },
    }
}

/// The query that will actually be sent: the base query plus the active filters.
#[must_use]
pub fn build_execution_query(request: &SearchRequest, qid: Option<&str>) -> String {
    let base = build_base_query(&request.criteria, qid);
    with_filters(&base, &request.criteria, request.year_max)
}

/// Resolve a taxon's input to a QID.
///
/// An empty input means "every taxon", a `*` is the explicit form of that, and
/// anything shaped like a QID is used without a round trip.
///
/// # Errors
/// Returns [`FetchError`] if the lookup cannot be run or read, and
/// [`FetchError::Parse`] if nothing matched — which is a validation failure the
/// caller reports to the user, not a broken query.
pub async fn resolve_taxon<H: Http>(http: &H, input: &str) -> Result<TaxonResolution, FetchError> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return Ok(TaxonResolution {
            qid: None,
            looked_up: trimmed.to_string(),
            notes: Vec::new(),
        });
    }
    if is_qid(trimmed) {
        return Ok(TaxonResolution {
            qid: Some(trimmed.to_ascii_uppercase()),
            looked_up: trimmed.to_string(),
            notes: Vec::new(),
        });
    }

    // Underscores and a lower-case genus both arrive from spreadsheets, and a
    // lookup for the text as typed would find nothing.
    let looked_up = standardize_taxon_name(trimmed);
    let csv =
        crate::execute_with_fallback(http, &taxon_lookup_query(&looked_up), ResponseFormat::Csv)
            .await?
            .text()?;
    let candidates = parse_taxon_csv(csv.as_bytes())?;

    let Some(chosen) = pick_match(&looked_up, &candidates) else {
        return Err(FetchError::Parse(format!("no taxon matched {trimmed:?}")));
    };

    let mut notes = Vec::new();
    if looked_up != trimmed {
        notes.push(TaxonNote::Standardized {
            original: trimmed.to_string(),
            looked_up: looked_up.clone(),
        });
    }
    if is_ambiguous(&candidates) {
        notes.push(TaxonNote::Ambiguous {
            candidates: candidates
                .iter()
                .take(4)
                .map(|c| format!("{} ({})", c.name, c.qid))
                .collect(),
        });
    }

    Ok(TaxonResolution {
        qid: Some(chosen),
        looked_up,
        notes,
    })
}

/// A bare QID, uppercase or not. At least one digit, so a lone `Q` is a search
/// term rather than an identifier.
#[must_use]
pub fn is_qid(value: &str) -> bool {
    let mut chars = value.chars();
    // At least one digit, so a lone `Q` is treated as a search term: a query
    // for `wd:Q` is one the endpoint cannot answer.
    matches!(chars.next(), Some('Q' | 'q'))
        && chars.clone().count() > 0
        && chars.all(|c| c.is_ascii_digit())
}

/// The name to look up: underscores become spaces, and the genus is
/// capitalised, because taxon names arrive from spreadsheets in both shapes.
#[must_use]
pub fn standardize_taxon_name(input: &str) -> String {
    let replaced = input.replace('_', " ");
    let mut words = replaced.split_whitespace();
    let Some(genus) = words.next() else {
        return replaced;
    };
    let mut chars = genus.chars();
    let mut out = chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
    });
    for word in words {
        out.push(' ');
        out.push_str(word);
    }
    out
}

/// The QID to filter on: an exact name match if there is one, else the first
/// candidate the endpoint returned.
fn pick_match(wanted: &str, candidates: &[lotus_model::TaxonMatch]) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let chosen = candidates
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(wanted))
        .or_else(|| candidates.first())?;
    Some(chosen.qid.clone())
}

/// Whether the answer is ambiguous: several exact names, or several candidates
/// and none of them the name that was asked for.
const fn is_ambiguous(candidates: &[lotus_model::TaxonMatch]) -> bool {
    candidates.len() > 1
}

/// Run a search and return its rows, counts and the query that produced them.
///
/// # Errors
/// Returns [`FetchError`] if the endpoint cannot be reached, rejects the query,
/// or returns something that is not the CSV that was asked for. A failure to
/// read the counts is *not* an error: the rows are already in hand, and the
/// local count is the right answer to fall back on.
pub async fn search<H: Http>(
    http: &H,
    request: &SearchRequest,
) -> Result<SearchResult, SearchError> {
    let year_max = request.year_max;
    lotus_model::validate_criteria(&request.criteria, year_max).map_err(SearchError::Invalid)?;

    let taxon = resolve_taxon(http, &request.criteria.taxon)
        .await
        .map_err(|e| SearchError::Transport {
            stage: "taxon",
            source: e,
        })?;

    let query = build_execution_query(request, taxon.qid.as_deref());
    let limit = request.limit.unwrap_or(DEFAULT_ROW_LIMIT);
    let display_query = limit_query(&query, limit);

    let csv = crate::execute_with_fallback(http, &display_query, ResponseFormat::Csv)
        .await
        .map_err(|e| SearchError::Transport {
            stage: "results",
            source: e,
        })?
        .text()
        .map_err(|e| SearchError::Transport {
            stage: "results",
            source: e,
        })?;

    let (rows, stats, truncated) =
        parse_compounds_csv_capped(csv.as_bytes(), limit).map_err(|e| SearchError::Transport {
            stage: "results",
            source: e.into(),
        })?;

    // The count query is the expensive one, so it is the one that gets
    // rate-limited. Falling back to the row-derived count keeps the totals
    // usable rather than reporting nothing.
    let stats = counts(http, &query).await.unwrap_or(stats);

    Ok(SearchResult {
        rows,
        stats: Some(stats),
        taxon: Some(taxon),
        query,
        truncated,
    })
}

/// The endpoint's own counts for a query.
///
/// # Errors
/// Returns [`FetchError`] if the count query could not be run or read. A caller
/// that already has the rows should treat this as advisory.
pub async fn counts<H: Http>(http: &H, query: &str) -> Result<DatasetStats, FetchError> {
    let csv = crate::execute_with_fallback(http, &counts_query(query), ResponseFormat::Csv)
        .await?
        .text()?;
    Ok(parse_counts_csv(csv.as_bytes())?)
}

/// How many rows a search returns when the caller does not say.
pub const DEFAULT_ROW_LIMIT: usize = 500;

/// Why a search could not be completed.
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    /// The filters cannot be turned into a query.
    #[error(transparent)]
    Invalid(#[from] ValidationError),
    /// The endpoint failed. `stage` says which query was running.
    #[error("the {stage} query failed: {source}")]
    Transport {
        /// Which query was running: the taxon lookup or the results.
        stage: &'static str,
        /// What the endpoint said.
        #[source]
        source: FetchError,
    },
}
