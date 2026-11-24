// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! What an export stream does when its upstream body stops part way.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "`HttpResponse` is an AFIT: the `async fn` spelling is fixed by the trait"
)]

use super::*;
use lotus_search::{FetchError, HttpResponse, ResponseBody};

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

struct CompleteUpstream {
    pieces: std::collections::VecDeque<&'static str>,
}

impl HttpResponse for CompleteUpstream {
    fn status(&self) -> u16 {
        200
    }

    async fn bytes(self) -> Result<ResponseBody, FetchError> {
        Ok(ResponseBody::new())
    }

    async fn text(self) -> Result<String, FetchError> {
        Ok(String::new())
    }

    async fn chunk(&mut self) -> Result<Option<ResponseBody>, FetchError> {
        Ok(self
            .pieces
            .pop_front()
            .map(|piece| ResponseBody::copy_from_slice(piece.as_bytes())))
    }
}

#[tokio::test]
async fn complete_upstream_flushes_csv_json_and_gzip_trailers() {
    for (format, compress) in [
        (ExportFormat::Csv, false),
        (ExportFormat::Csv, true),
        (ExportFormat::Json, false),
        (ExportFormat::Json, true),
    ] {
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let upstream = CompleteUpstream {
            pieces: ["compound,taxon\n", "Q1,Q2\n"].into(),
        };
        pump_export(upstream, tx, format, compress).await;
        let mut bytes = Vec::new();
        while let Some(item) = rx.recv().await {
            assert!(item.is_ok(), "unexpected stream error");
            if let Ok(chunk) = item {
                bytes.extend_from_slice(&chunk);
            }
        }
        assert!(!bytes.is_empty(), "streamed output must be non-empty");
        if compress {
            use std::io::Read as _;
            assert_eq!(bytes.get(..2), Some(&[0x1f, 0x8b][..]));
            let mut plain_text = String::new();
            let mut decoder = flate2::read::GzDecoder::new(bytes.as_slice());
            assert!(
                decoder.read_to_string(&mut plain_text).is_ok(),
                "compressed export must carry a valid complete gzip trailer"
            );
            assert_ne!(plain_text, "");
        } else if format == ExportFormat::Csv {
            assert_eq!(bytes, b"compound,taxon\nQ1,Q2\n");
        } else {
            assert!(
                String::from_utf8(bytes).is_ok_and(|text| text.contains("\"Q1\"")),
                "expected valid UTF-8 JSON containing Q1"
            );
        }
    }
}

#[tokio::test]
async fn push_chunk_and_stream_helpers_handle_closed_channels_and_status() {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(1);
    drop(rx);
    let mut encoder = Some(GzEncoder::new(Vec::new(), Compression::default()));
    assert!(!push_chunk(&tx, &mut encoder, b"data").await);

    let (tx_open, rx_open) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(4);
    let mut empty_encoder = Some(GzEncoder::new(Vec::new(), Compression::default()));
    assert!(push_chunk(&tx_open, &mut empty_encoder, b"data").await);

    // After flushing header+data on an open channel, an empty write leaves `ready` empty,
    // which must return `true` even after the receiver closes (`ready.is_empty() || send`).
    drop(rx_open);
    assert!(push_chunk(&tx_open, &mut empty_encoder, b"").await);

    assert!(check_upstream_status(200).is_ok());
    assert!(check_upstream_status(502).is_err());
    assert_eq!(sanitized_attachment(Some("  ")), None);
    assert_eq!(
        sanitized_attachment(Some("export.csv")).as_deref(),
        Some("export.csv")
    );
}
