// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Streaming columnar search (`search_columnar`) and enrichment chunk feeders.

use lotus_query::{CsvColumnarReader, dictionary_queries, occurrences_query};

use super::execute_search::SearchError;
use super::query_plan::prepare_query;
use crate::client::Http;
use crate::error::{FetchError, ResponseFormat};
use crate::result::{ColumnarSearchResult, SearchRequest};

/// A search that returns the whole result set, exactly, and in memory that fits.
///
/// The browser's path. Three differences from [`search`](super::search), each a decision:
///
/// - **No `LIMIT`.** The query asks for every row. Asking for 500 and counting separately
///   left the filters able to see only those 500, since the truncation was server-side.
/// - **No `COUNT` query.** [`lotus_model::ColumnarResultSet`] holds every row and
///   deduplicates nothing, so its statistics *are* the endpoint's counts. With the second
///   query goes the `counts_query` construction whose deletion of two named blocks could
///   silently make a filter count nothing.
/// - **The body is never assembled.** The response is read through [`crate::BodyChunks`]
///   and folded in a chunk at a time, so peak memory is the set plus one chunk rather than
///   the set plus the payload.
///
/// # Errors
/// Returns [`SearchError::Invalid`] if the criteria cannot be turned into a
/// query, and [`SearchError::Transport`] if the taxon lookup, the results query
/// or the body fails.
pub async fn search_columnar<H: Http>(
    http: &H,
    request: &SearchRequest,
) -> Result<ColumnarSearchResult, SearchError> {
    let (query, taxon) = prepare_query(http, request).await?;
    let occ_query = occurrences_query(&query);
    let mut body = crate::execute_streaming_with_fallback(http, &occ_query, ResponseFormat::Csv)
        .await
        .map_err(|e| SearchError::Transport {
            stage: "results",
            source: e,
        })?
        .chunks;

    let mut reader = CsvColumnarReader::new();
    let mut noop = |_| {};
    feed_chunks_reporting(&mut reader, &mut body, &mut noop)
        .await
        .map_err(|e| SearchError::Transport {
            stage: "results",
            source: e,
        })?;
    enrich_columnar_reader(
        &mut reader,
        |dict_query| async move {
            Ok(
                crate::execute_streaming_with_fallback(http, &dict_query, ResponseFormat::Csv)
                    .await?
                    .chunks,
            )
        },
        &mut noop,
    )
    .await
    .map_err(|e| SearchError::Transport {
        stage: "results",
        source: e,
    })?;

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
    body: crate::ChunkedBody,
) -> Result<lotus_model::ColumnarResultSet, FetchError> {
    columnar_from_chunks_reporting(body, &mut |_| {}).await
}

/// How much of a result set has arrived so far.
///
/// Counts, not a fraction: the endpoint does not say how many rows the query will
/// produce before it has produced them, so there is no denominator to divide by.
/// A percentage here would be invented, and the number that matters -- how much is
/// still to come -- is exactly what is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StreamProgress {
    /// Data rows folded into the set so far.
    pub rows: usize,
    /// Decompressed body bytes read so far.
    pub bytes: usize,
}

/// Feed a chunked response stream into an existing [`CsvColumnarReader`],
/// reporting progress after each chunk.
///
/// # Errors
/// Propagates any transport failure, and returns [`FetchError::Parse`] if a
/// chunk cannot be decoded by the reader.
pub async fn feed_chunks_reporting<F>(
    reader: &mut CsvColumnarReader,
    body: &mut crate::ChunkedBody,
    on_progress: &mut F,
) -> Result<(), FetchError>
where
    F: FnMut(StreamProgress) + ?Sized,
{
    loop {
        let Some(chunk) = body.next_chunk().await? else {
            break;
        };
        reader
            .feed(&chunk)
            .map_err(|e| FetchError::Parse(e.to_string()))?;
        on_progress(StreamProgress {
            rows: reader.row_count(),
            bytes: reader.byte_count(),
        });
    }
    Ok(())
}

/// Enrich an occurrence-only [`CsvColumnarReader`] with compound and reference
/// dictionary streams when the initial stream omitted metadata columns.
///
/// When `reader.needs_enrichment()` is `false` (for example, when a single
/// 15-column fixture or WDQS fallback stream already supplied metadata), this
/// returns immediately without issuing additional requests.
///
/// # Errors
/// Propagates any transport or parse error from the dictionary streams.
pub async fn enrich_columnar_reader<F, S, Fut>(
    reader: &mut CsvColumnarReader,
    mut open_stream: S,
    on_progress: &mut F,
) -> Result<(), FetchError>
where
    F: FnMut(StreamProgress) + ?Sized,
    S: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<crate::ChunkedBody, FetchError>>,
{
    if reader.needs_enrichment() {
        let compound_qids: Vec<u32> = reader.compound_qids().collect();
        let reference_qids: Vec<u32> = reader.reference_qids().collect();

        for dict_query in dictionary_queries(&compound_qids, &reference_qids) {
            reader
                .next_enrichment_stream()
                .map_err(|e| FetchError::Parse(e.to_string()))?;
            let mut body = open_stream(dict_query).await?;
            feed_chunks_reporting(reader, &mut body, on_progress).await?;
        }
    }
    Ok(())
}

/// Fold a body into a columnar set, reporting progress after each chunk.
///
/// The callback runs **once per chunk**, tens of thousands of times on a wide search, so a
/// caller re-rendering each time spends more rendering than parsing and has to throttle.
/// [`StreamProgress`] exists so it can.
///
/// Generic over the callback rather than `&mut dyn FnMut`: a `dyn FnMut` is not `Send`, so
/// a function taking one has a non-`Send` future and [`columnar_from_chunks`] delegates
/// here. Native and server paths need `Send`; the browser needs a callback capturing a
/// `Signal`, emphatically not `Send`. Monomorphising lets both hold at once.
///
/// # Errors
/// Propagates any transport failure, and returns [`FetchError::Parse`] if the
/// body is not the CSV the result queries produce.
pub async fn columnar_from_chunks_reporting<F>(
    mut body: crate::ChunkedBody,
    on_progress: &mut F,
) -> Result<lotus_model::ColumnarResultSet, FetchError>
where
    F: FnMut(StreamProgress) + ?Sized,
{
    let mut reader = CsvColumnarReader::new();
    feed_chunks_reporting(&mut reader, &mut body, on_progress).await?;
    reader
        .finish()
        .map_err(|e| FetchError::Parse(e.to_string()))
}
