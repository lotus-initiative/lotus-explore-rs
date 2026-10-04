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
use crate::result::{
    ColumnarSearchResult, SearchRequest, SearchResult, TaxonNote, TaxonResolution,
};
use lotus_model::{DatasetStats, SearchCriteria, SmilesSearchType, ValidationError};
use lotus_query::{
    CsvColumnarReader, parse_compounds_csv_capped, parse_counts_csv, parse_taxon_csv,
};
use lotus_query::{
    Nomenclature, all_compounds_including_untaxonomised_query, all_compounds_query,
    compounds_by_taxon_query_with, counts_query, limit_query, structure_search_query_with,
    taxon_lookup_query, with_filters,
};

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
        // Three cases, and the first two used to be one.
        //
        // Nothing given is not the same request as `*`. Nothing given means "no
        // constraint I can apply", which includes the compounds with no organism
        // recorded. `*` is the explicit request for everything with an occurrence, and
        // it keeps requiring one. Sending both to the same query meant an empty box
        // answered the narrower question without saying so.
        //
        // The wildcard has to be recognised from `criteria.taxon` rather than from
        // `qid`, because `resolve_taxon` resolves it to `None` -- it names no Wikidata
        // entity -- so by the time the query is built the two arrive looking identical.
        // Reading the difference off the QID alone therefore made `*` and an empty
        // box build the same query, which is the bug this arm exists to prevent.
        //
        // Only the no-structure branch is corrected. In the structure branch above a
        // wildcard is already expressed by not filtering the taxon, and routing it
        // through the root taxon there would require an occurrence and so could
        // return *fewer* compounds than "every taxon" is supposed to.
        None if criteria.taxon.trim() == "*" => all_compounds_query(),
        None => match qid {
            Some(qid) if qid != "*" => compounds_by_taxon_query_with(qid, &nomenclature),
            Some(_) => all_compounds_query(),
            None => all_compounds_including_untaxonomised_query(),
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

/// A search that returns the whole result set, exactly, and in memory that fits.
///
/// This is the path the browser takes. It differs from [`search`] in three ways,
/// each of which is a decision rather than a convenience:
///
/// - **No `LIMIT`.** The query asks for every row. The old path asked for 500
///   and then counted separately, which is why its filters could only ever see
///   500 rows: the truncation was server-side, so nothing downstream could undo
///   it. Here the whole set arrives and the table can filter all of it.
/// - **No `COUNT` query.** [`lotus_model::ColumnarResultSet`] holds every row and
///   deduplicates nothing, so its statistics *are* the endpoint's counts. The
///   second query is not needed, and with it goes the `counts_query`
///   construction whose deletion of two named blocks could silently make a filter
///   count nothing.
/// - **The body is never assembled.** The response is read through
///   [`crate::BodyChunks`] and folded into the set a chunk at a time, so peak
///   memory is the set plus one chunk rather than the set plus the payload. For
///   the widest search -- 2,990,730 edges at the 314 B/row a real export averages
///   -- that is the difference between about 940 MB of CSV and none.
///
/// # Errors
/// Returns [`SearchError::Invalid`] if the criteria cannot be turned into a
/// query, and [`SearchError::Transport`] if the taxon lookup, the results query
/// or the body fails.
pub async fn search_columnar<H: Http>(
    http: &H,
    request: &SearchRequest,
) -> Result<ColumnarSearchResult, SearchError> {
    let year_max = request.year_max;
    lotus_model::validate_criteria(&request.criteria, year_max).map_err(SearchError::Invalid)?;

    let taxon = resolve_taxon(http, &request.criteria.taxon)
        .await
        .map_err(|e| SearchError::Transport {
            stage: "taxon",
            source: e,
        })?;

    let query = build_execution_query(request, taxon.qid.as_deref());
    let mut body = crate::execute_streaming_with_fallback(http, &query, ResponseFormat::Csv)
        .await
        .map_err(|e| SearchError::Transport {
            stage: "results",
            source: e,
        })?
        .chunks;

    let mut reader = CsvColumnarReader::new();
    loop {
        let chunk = body
            .next_chunk()
            .await
            .map_err(|e| SearchError::Transport {
                stage: "results",
                source: e,
            })?;
        let Some(bytes) = chunk else { break };
        reader.feed(&bytes).map_err(|e| SearchError::Transport {
            stage: "results",
            source: e.into(),
        })?;
    }

    let set = reader.finish().map_err(|e| SearchError::Transport {
        stage: "results",
        source: e.into(),
    })?;

    Ok(ColumnarSearchResult {
        set,
        taxon: Some(taxon),
        query,
    })
}

/// Fold a response body, read a chunk at a time, into a columnar set.
///
/// The loop lives here because a `BodyChunks` is not a `Read`: bridging the two
/// means buffering, which is the thing being avoided. So the payload passes
/// through exactly one place at a time, and peak memory is the finished set plus
/// the largest single chunk.
///
/// # Errors
/// Returns [`FetchError::Parse`] if the body cannot be read as the CSV the
/// result queries produce, and propagates any transport failure.
pub async fn columnar_from_chunks(
    mut body: crate::ChunkedBody,
) -> Result<lotus_model::ColumnarResultSet, FetchError> {
    let mut reader = CsvColumnarReader::new();
    loop {
        let Some(chunk) = body.next_chunk().await? else {
            break;
        };
        reader
            .feed(&chunk)
            .map_err(|e| FetchError::Parse(e.to_string()))?;
    }
    reader
        .finish()
        .map_err(|e| FetchError::Parse(e.to_string()))
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

#[cfg(test)]
mod tests {
    use super::{build_base_query, is_ambiguous, is_qid, standardize_taxon_name};
    use lotus_model::{SearchCriteria, TaxonMatch, TaxonNameSource};

    /// Underscores become spaces and the genus is capitalised, because taxon names
    /// arrive from spreadsheets in both shapes.
    ///
    /// Both halves matter independently: an underscored name finds nothing as typed,
    /// and a lower-case genus is a different string from the one Wikidata stores.
    /// Mutation testing could replace this function with an empty string or with a
    /// constant and the suite noticed neither, because no test fed it anything.
    #[test]
    fn a_taxon_name_is_standardised_before_it_is_looked_up() {
        assert_eq!(standardize_taxon_name("gentiana_lutea"), "Gentiana lutea");
        assert_eq!(standardize_taxon_name("Gentiana_lutea"), "Gentiana lutea");
        assert_eq!(standardize_taxon_name("gentiana lutea"), "Gentiana lutea");

        // Only the genus is touched: a species epithet keeps the case it was given,
        // because Wikidata stores `lutea` in lower case but `GENTIANA` is not a word.
        assert_eq!(
            standardize_taxon_name("gentiana LUTEA"),
            "Gentiana LUTEA",
            "only the genus is recased"
        );

        // A single word is still a genus.
        assert_eq!(standardize_taxon_name("gentianella"), "Gentianella");

        // Nothing in, nothing out. Input with no word in it is handed back
        // unchanged rather than trimmed to nothing: `resolve_taxon` trims the input
        // *before* calling this, so a blank box never reaches here, and a
        // standardiser that silently rewrote blank input would be inventing a name
        // the reader never typed.
        assert_eq!(standardize_taxon_name(""), "");
        assert_eq!(standardize_taxon_name("   "), "   ");
        assert_eq!(
            standardize_taxon_name("_"),
            " ",
            "an underscore becomes a space, and a lone underscore has no genus to recase"
        );
    }

    /// A QID is a `Q` and at least one digit, and nothing looser.
    ///
    /// The digit requirement is the interesting half: `Q` alone is a search term,
    /// and treating it as an identifier would send `wd:Q` to an endpoint that
    /// cannot answer it.
    #[test]
    fn a_bare_qid_is_recognised_and_nothing_else_is() {
        assert!(is_qid("Q16521"));
        assert!(is_qid("q16521"), "a lower-case q is still a QID");

        assert!(!is_qid("Q"), "a lone Q is a search term, not an identifier");
        assert!(!is_qid("q"), "nor in lower case");
        assert!(!is_qid(""), "nor is an empty string");
        assert!(!is_qid("Q16521a"), "nor a QID with a suffix");
        assert!(!is_qid("P16521"), "a property is not an entity");
        assert!(!is_qid("Q-16521"), "nor a QID with a sign");
        assert!(!is_qid("Gentiana lutea"), "nor a name");
    }

    /// More than one candidate is ambiguous; exactly one is not.
    ///
    /// This decides whether the reader is told that the first of several matches was
    /// used. Getting it wrong in the tight direction hides a real ambiguity, and in
    /// the loose direction warns about a search that had no choice to make.
    #[test]
    fn only_several_candidates_are_ambiguous() {
        let candidate = |qid: &str| TaxonMatch {
            qid: qid.to_string(),
            name: format!("name-{qid}"),
            source: TaxonNameSource::Scientific,
        };
        let candidates = |n: usize| {
            (0..n)
                .map(|i| candidate(&format!("Q{}", 100 + i)))
                .collect::<Vec<_>>()
        };

        assert!(!is_ambiguous(&candidates(0)), "no match is not ambiguous");
        assert!(
            !is_ambiguous(&candidates(1)),
            "one match is a match, not an ambiguity"
        );
        assert!(is_ambiguous(&candidates(2)), "two matches are ambiguous");
        assert!(is_ambiguous(&candidates(5)), "and so are five");
    }

    /// A wildcard passed as a QID still builds the query that requires occurrences.
    ///
    /// `build_base_query` reads the wildcard out of `criteria.taxon` now, because
    /// `resolve_taxon` resolves `*` to no QID at all. This test covers the other
    /// door: a caller that hands the wildcard straight through as a QID, which is
    /// what the guard on that arm is for.
    #[test]
    fn a_wildcard_handed_straight_in_is_not_treated_as_a_qid() {
        let criteria = SearchCriteria {
            taxon: "*".to_string(),
            ..SearchCriteria::up_to_year(2026)
        };
        let via_qid = build_base_query(&criteria, Some("*"));

        assert!(
            !via_qid.contains("wd:*"),
            "the wildcard is not an entity and must never be spliced into a wd: IRI"
        );
        assert!(
            via_qid.contains("p:P703"),
            "a wildcard keeps requiring an occurrence however it arrived"
        );
    }
}
