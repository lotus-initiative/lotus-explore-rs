// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! What an export stream does when its upstream body stops part way.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "`HttpResponse` is an AFIT: the `async fn` spelling is fixed by the trait"
)]

use super::*;
use lotus_search::{FetchError, HttpResponse, ResponseBody};

/// A 200 whose body arrives in `pieces` and then fails, as a connection dropped
/// mid-transfer does.
struct CutShort {
    pieces: std::collections::VecDeque<&'static str>,
}

impl HttpResponse for CutShort {
    fn status(&self) -> u16 {
        200
    }

    async fn bytes(self) -> Result<ResponseBody, FetchError> {
        Err(FetchError::Network("read through chunk".into()))
    }

    async fn text(self) -> Result<String, FetchError> {
        Err(FetchError::Network("read through chunk".into()))
    }

    async fn chunk(&mut self) -> Result<Option<ResponseBody>, FetchError> {
        self.pieces.pop_front().map_or_else(
            || Err(FetchError::Network("connection reset".into())),
            |piece| Ok(Some(ResponseBody::copy_from_slice(piece.as_bytes()))),
        )
    }
}

/// Everything the pump sent, in order.
async fn run(format: ExportFormat, compress: bool) -> Vec<Result<Bytes, std::io::Error>> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let upstream = CutShort {
        pieces: ["compound,taxon\n", "Q1,Q2\n"].into(),
    };
    pump_export(upstream, tx, format, compress).await;
    let mut sent = Vec::new();
    while let Some(item) = rx.recv().await {
        sent.push(item);
    }
    sent
}

/// A cut-short export used to arrive as a complete file: a 200, a valid gzip
/// of part of the CSV, or an empty one for JSON and Turtle. The stream must end
/// in an error instead, so the client sees a failed download and not a small
/// one.
#[tokio::test]
async fn an_upstream_that_stops_part_way_fails_the_download() {
    for (format, compress) in [
        (ExportFormat::Csv, true),
        (ExportFormat::Csv, false),
        (ExportFormat::Json, true),
        (ExportFormat::Rdf, false),
    ] {
        let sent = run(format, compress).await;
        assert!(
            sent.last().is_some_and(Result::is_err),
            "{format:?} compress={compress}: the stream ends in an error, got {} items, last ok={:?}",
            sent.len(),
            sent.last().map(Result::is_ok)
        );
    }
}
