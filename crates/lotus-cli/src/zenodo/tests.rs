// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit and loopback-HTTP integration tests for the direct Zenodo client.

#![allow(clippy::expect_used)]

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;

#[test]
fn prepare_uploads_rejects_empty_traversal_and_duplicates() {
    let path = Path::new("out/file.txt");
    let valid = prepare_uploads([("a.tar.gz".to_string(), path), ("b.json".to_string(), path)])
        .expect("distinct valid names");
    assert_eq!(valid.len(), 2);
    assert_eq!(valid.first().map(|u| u.filename.as_str()), Some("a.tar.gz"));

    for bad in ["", ".", "..", "sub/file", "sub\\file"] {
        let err = prepare_uploads([(bad.to_string(), path)]).expect_err("invalid name");
        assert!(
            err.to_string().contains("invalid upload filename"),
            "got: {err}"
        );
    }

    let dup = prepare_uploads([("same".to_string(), path), ("same".to_string(), path)])
        .expect_err("duplicate name");
    assert_eq!(dup.to_string(), "duplicate upload filename: same");
}

#[test]
fn debug_redacts_bearer_token() {
    let client =
        ZenodoClient::with_base_url("http://127.0.0.1:9999/api", "super-secret-token-value")
            .expect("valid client");
    let rendered = format!("{client:?}");
    assert!(!rendered.contains("super-secret-token-value"), "{rendered}");
    assert!(rendered.contains("<redacted>"), "{rendered}");
}

#[test]
fn with_base_url_rejects_invalid_url_and_blank_token() {
    assert!(ZenodoClient::with_base_url("ftp://zenodo.org/api", "tok").is_err());
    assert!(ZenodoClient::with_base_url("https://zenodo.org/api", "   ").is_err());
}

#[test]
fn parse_record_id_extracts_numeric_id_from_record_urls() {
    assert_eq!(
        parse_record_id_from_url("https://zenodo.org/api/records/123456"),
        Some(123_456)
    );
    assert_eq!(
        parse_record_id_from_url("https://zenodo.org/api/records/99?draft=1"),
        Some(99)
    );
    assert_eq!(
        parse_record_id_from_url("https://zenodo.org/api/deposit/depositions/99"),
        None
    );
}

#[test]
fn deposit_metadata_serializes_zenodo_description_key_and_fields() {
    let meta = DepositMetadata {
        title: "LOTUS Archive".to_string(),
        upload_type: "dataset",
        publication_date: "2026-01-15".to_string(),
        description_html: "<p>Desc</p>".to_string(),
        creators: vec![Creator {
            name: "The LOTUS Initiative".to_string(),
        }],
        access_right: "open",
        license: "cc-by-4.0",
        keywords: vec!["LOTUS".to_string()],
        related_identifiers: vec![RelatedIdentifier {
            identifier: "https://github.com/lotus-initiative/lotus-explore-rs/commit/0123456789abcdef0123456789abcdef01234567".to_string(),
            relation: "isDerivedFrom",
            scheme: "url",
            resource_type: "software",
        }],
        communities: vec![Community {
            identifier: "the-lotus-initiative",
        }],
    };
    let json = serde_json::to_value(&meta).expect("serialize metadata");
    assert_eq!(
        json.get("description").and_then(serde_json::Value::as_str),
        Some("<p>Desc</p>")
    );
    assert_eq!(
        json.get("upload_type").and_then(serde_json::Value::as_str),
        Some("dataset")
    );
    assert_eq!(
        json.get("access_right").and_then(serde_json::Value::as_str),
        Some("open")
    );
    assert_eq!(
        json.get("publication_date")
            .and_then(serde_json::Value::as_str),
        Some("2026-01-15")
    );
    assert_eq!(
        json.get("creators")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("The LOTUS Initiative")
    );
    assert_eq!(
        json.get("communities")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("identifier"))
            .and_then(serde_json::Value::as_str),
        Some("the-lotus-initiative")
    );
}

#[derive(Debug)]
struct RecordedRequest {
    method: String,
    path: String,
    auth: Option<String>,
    body: String,
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> RecordedRequest {
    let mut buf = Vec::new();
    let mut tmp = [0_u8; 4096];
    let header_end;
    loop {
        let n = stream.read(&mut tmp).await.expect("read socket");
        assert!(n > 0, "unexpected EOF reading HTTP headers");
        buf.extend_from_slice(tmp.get(..n).expect("slice"));
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }
    let header_text = String::from_utf8_lossy(buf.get(..header_end).expect("headers"));
    let mut lines = header_text.lines();
    let request_line = lines.next().expect("request line");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().expect("method").to_string();
    let path = parts.next().expect("path").to_string();

    let mut content_len = 0_usize;
    let mut auth = None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                content_len = v.trim().parse().expect("content-length");
            } else if k.eq_ignore_ascii_case("authorization") {
                auth = Some(v.trim().to_string());
            }
        }
    }
    while buf.len() < header_end + content_len {
        let n = stream.read(&mut tmp).await.expect("read body");
        if n == 0 {
            break;
        }
        buf.extend_from_slice(tmp.get(..n).expect("slice"));
    }
    let body = String::from_utf8_lossy(
        buf.get(header_end..header_end + content_len)
            .unwrap_or_default(),
    )
    .into_owned();
    RecordedRequest {
        method,
        path,
        auth,
        body,
    }
}

async fn spawn_mock_server<F>(handler: F) -> (String, Arc<Mutex<Vec<RecordedRequest>>>)
where
    F: Fn(&RecordedRequest, &str) -> (u16, String) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let base = format!("http://{addr}/api");
    let base_for_handler = base.clone();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let recorded_clone = Arc::clone(&recorded);
    let handler = Arc::new(handler);

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let req = read_http_request(&mut stream).await;
            let (status, body) = handler(&req, &base_for_handler);
            recorded_clone.lock().expect("lock").push(req);
            let reason = match status {
                200 => "OK",
                201 => "Created",
                204 => "No Content",
                401 => "Unauthorized",
                _ => "Error",
            };
            let reply = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes()).await;
        }
    });

    (base, recorded)
}

fn sample_metadata() -> DepositMetadata {
    DepositMetadata {
        title: "Test".to_string(),
        upload_type: "dataset",
        publication_date: "2026-01-15".to_string(),
        description_html: "<p>Test</p>".to_string(),
        creators: vec![Creator {
            name: "LOTUS".to_string(),
        }],
        access_right: "open",
        license: "cc-by-4.0",
        keywords: vec!["LOTUS".to_string()],
        related_identifiers: Vec::new(),
        communities: vec![Community {
            identifier: "the-lotus-initiative",
        }],
    }
}

fn temp_upload_dir(label: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("lotus-zenodo-test-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[tokio::test]
async fn publish_dataset_creates_uploads_and_publishes_initial_record() {
    let (base, log) = spawn_mock_server(|req, api| match (req.method.as_str(), req.path.as_str()) {
        ("POST", "/api/deposit/depositions") => (
            201,
            format!(
                r#"{{"id":101,"submitted":false,"links":{{"bucket":"{api}/files/bucket-101"}}}}"#
            ),
        ),
        ("PUT", "/api/deposit/depositions/101") => (
            200,
            format!(
                r#"{{"id":101,"submitted":false,"files":[],"links":{{"bucket":"{api}/files/bucket-101"}}}}"#
            ),
        ),
        (
            "PUT",
            "/api/files/bucket-101/archive.tar.gz" | "/api/files/bucket-101/manifest.json",
        ) => (200, r#"{"key":"ok"}"#.to_string()),
        ("POST", "/api/deposit/depositions/101/actions/publish") => {
            (202, r#"{"id":101,"submitted":true,"record_id":777}"#.to_string())
        }
        other => (400, format!("unexpected request: {other:?}")),
    })
    .await;

    let dir = temp_upload_dir("initial");
    let archive = dir.join("archive.tar.gz");
    let manifest = dir.join("manifest.json");
    std::fs::write(&archive, b"tgz-bytes").expect("write archive");
    std::fs::write(&manifest, b"{}").expect("write manifest");
    let uploads = prepare_uploads([
        ("archive.tar.gz".to_string(), archive.as_path()),
        ("manifest.json".to_string(), manifest.as_path()),
    ])
    .expect("uploads");

    let client = ZenodoClient::with_base_url(base, "secret-tok")
        .expect("client")
        .with_fast_timings();
    let published = client
        .publish_dataset(None, &sample_metadata(), &uploads)
        .await
        .expect("publish succeeds");
    assert_eq!(published.record_id, DepositionId(777));

    let calls: Vec<(Option<String>, String)> = log
        .lock()
        .expect("lock")
        .iter()
        .map(|call| (call.auth.clone(), call.body.clone()))
        .collect();
    assert_eq!(calls.len(), 5);
    for (auth, _) in &calls {
        assert_eq!(auth.as_deref(), Some("Bearer secret-tok"));
    }
    assert!(
        calls
            .get(1)
            .expect("metadata call")
            .1
            .contains("\"upload_type\":\"dataset\"")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn publish_dataset_versions_published_root_and_clears_old_files() {
    let (base, log) = spawn_mock_server(|req, api| match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/api/deposit/depositions/42") => (
            200,
            format!(
                r#"{{"id":42,"submitted":true,"links":{{"self":"{api}/deposit/depositions/42","latest":"{api}/records/50"}}}}"#
            ),
        ),
        ("GET", "/api/deposit/depositions/50") => (
            200,
            format!(
                r#"{{"id":50,"submitted":true,"links":{{"self":"{api}/deposit/depositions/50"}}}}"#
            ),
        ),
        ("POST", "/api/deposit/depositions/50/actions/newversion") => (
            201,
            format!(r#"{{"id":50,"links":{{"latest_draft":"{api}/deposit/depositions/51"}}}}"#),
        ),
        ("GET" | "PUT", "/api/deposit/depositions/51") => (
            200,
            format!(
                r#"{{"id":51,"submitted":false,"files":[{{"id":"old-1"}}],"links":{{"bucket":"{api}/files/bucket-51"}}}}"#
            ),
        ),
        ("DELETE", "/api/deposit/depositions/51/files/old-1") => (204, String::new()),
        ("PUT", "/api/files/bucket-51/data.tar.gz") => (200, r#"{"key":"data.tar.gz"}"#.to_string()),
        ("POST", "/api/deposit/depositions/51/actions/publish") => {
            (202, r#"{"id":51,"submitted":true,"record_id":51}"#.to_string())
        }
        other => (400, format!("unexpected request: {other:?}")),
    })
    .await;

    let dir = temp_upload_dir("newversion");
    let data = dir.join("data.tar.gz");
    std::fs::write(&data, b"payload").expect("write data");
    let uploads = prepare_uploads([("data.tar.gz".to_string(), data.as_path())]).expect("uploads");

    let client = ZenodoClient::with_base_url(base, "secret-tok")
        .expect("client")
        .with_fast_timings();
    let published = client
        .publish_dataset(Some(DepositionId(42)), &sample_metadata(), &uploads)
        .await
        .expect("newversion publish succeeds");
    assert_eq!(published.record_id, DepositionId(51));

    let paths: Vec<String> = log
        .lock()
        .expect("lock")
        .iter()
        .map(|r| format!("{} {}", r.method, r.path))
        .collect();
    assert!(paths.contains(&"DELETE /api/deposit/depositions/51/files/old-1".to_string()));
    assert!(paths.contains(&"POST /api/deposit/depositions/50/actions/newversion".to_string()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn publish_dataset_reuses_unpublished_draft_and_surfaces_http_errors() {
    let (base, log) = spawn_mock_server(|req, api| match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/api/deposit/depositions/12") => (
            200,
            format!(
                r#"{{"id":12,"submitted":false,"files":[],"links":{{"bucket":"{api}/files/b12"}}}}"#
            ),
        ),
        ("PUT", "/api/deposit/depositions/12") => (401, r#"{"message":"bad token"}"#.to_string()),
        other => (400, format!("unexpected request: {other:?}")),
    })
    .await;

    let client = ZenodoClient::with_base_url(base, "bad-tok")
        .expect("client")
        .with_fast_timings();
    let err = client
        .publish_dataset(Some(DepositionId(12)), &sample_metadata(), &[])
        .await
        .expect_err("401 should fail");
    let msg = format!("{err:#}");
    assert!(msg.contains("401"), "got: {msg}");

    assert!(
        !log.lock()
            .expect("lock")
            .iter()
            .any(|r| r.path.contains("newversion")),
        "unpublished draft must not call newversion"
    );
}

#[test]
fn from_env_with_selects_sandbox_then_production_and_rejects_blank_or_invalid() {
    let mut missing = |_: &str| Err(std::env::VarError::NotPresent);
    let err = ZenodoClient::from_env_with(&mut missing).expect_err("missing tokens must fail");
    assert!(err.to_string().contains("ZENODO_TOKEN"), "{err}");

    let (sandbox_client, sandbox_label) = ZenodoClient::from_env_with(|name| match name {
        "ZENODO_SANDBOX_TOKEN" => Ok("sb-tok".to_string()),
        _ => Err(std::env::VarError::NotPresent),
    })
    .expect("sandbox client");
    assert_eq!(sandbox_client.base_url, SANDBOX_API_URL);
    assert_eq!(sandbox_label, "Zenodo sandbox (ZENODO_SANDBOX_TOKEN)");

    let (prod_client, prod_label) = ZenodoClient::from_env_with(|name| match name {
        "ZENODO_SANDBOX_TOKEN" => Ok("   ".to_string()),
        "ZENODO_TOKEN" => Ok("prod-tok".to_string()),
        _ => Err(std::env::VarError::NotPresent),
    })
    .expect("blank sandbox token falls back to production");
    assert_eq!(prod_client.base_url, PRODUCTION_API_URL);
    assert_eq!(prod_label, "Zenodo production (ZENODO_TOKEN)");

    let invalid_err = ZenodoClient::from_env_with(|name| match name {
        "ZENODO_SANDBOX_TOKEN" => Err(std::env::VarError::NotUnicode(std::ffi::OsString::from(
            "bad",
        ))),
        _ => Err(std::env::VarError::NotPresent),
    })
    .expect_err("non-unicode env var must error");
    assert!(
        invalid_err
            .to_string()
            .contains("cannot read ZENODO_SANDBOX_TOKEN"),
        "{invalid_err}"
    );
}

#[tokio::test]
async fn send_with_retries_recovers_from_transient_429_and_5xx_responses() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_srv = Arc::clone(&attempts);
    let (base, _) = spawn_mock_server(move |_, api| {
        let n = attempts_srv.fetch_add(1, Ordering::SeqCst) + 1;
        match n {
            1 => (429, "rate limited".to_string()),
            2 => (503, "busy".to_string()),
            _ => (
                200,
                format!(r#"{{"id":7,"submitted":false,"files":[],"links":{{"bucket":"{api}/files/b7"}}}}"#),
            ),
        }
    })
    .await;

    let client = ZenodoClient::with_base_url(base, "tok")
        .expect("client")
        .with_fast_timings();
    let dep = client
        .get_deposition(DepositionId(7))
        .await
        .expect("recovers on third attempt");
    assert_eq!(dep.id, 7);
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn send_with_retries_stops_and_surfaces_http_status_after_max_attempts() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_srv = Arc::clone(&attempts);
    let (base, _) = spawn_mock_server(move |_, _| {
        attempts_srv.fetch_add(1, Ordering::SeqCst);
        (503, r#"{"message":"still down"}"#.to_string())
    })
    .await;

    let client = ZenodoClient::with_base_url(base, "tok")
        .expect("client")
        .with_fast_timings();
    let err = client
        .get_deposition(DepositionId(7))
        .await
        .expect_err("all 503 attempts must fail");
    let msg = format!("{err:#}");
    assert!(msg.contains("503"), "expected 503 error, got: {msg}");
    assert_eq!(attempts.load(Ordering::SeqCst), MAX_TRANSIENT_ATTEMPTS);
}

#[tokio::test]
async fn send_with_retries_retries_dropped_socket_and_errors_when_unreachable() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let base = format!("http://{addr}/api");
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_srv = Arc::clone(&attempts);

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let n = attempts_srv.fetch_add(1, Ordering::SeqCst) + 1;
            if n == 1 {
                // Drop connection immediately to simulate transient transport failure.
                drop(stream);
                continue;
            }
            let _ = read_http_request(&mut stream).await;
            let body = r#"{"id":8,"submitted":false,"files":[],"links":{}}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes()).await;
        }
    });

    let client = ZenodoClient::with_base_url(base, "tok")
        .expect("client")
        .with_fast_timings();
    let dep = client
        .get_deposition(DepositionId(8))
        .await
        .expect("recovers after initial dropped connection");
    assert_eq!(dep.id, 8);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);

    // Closed port: exhausts MAX_TRANSIENT_ATTEMPTS and returns the transport error.
    let closed_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind closed listener");
    let closed_addr = closed_listener.local_addr().expect("closed addr");
    drop(closed_listener);
    let closed_client = ZenodoClient::with_base_url(format!("http://{closed_addr}/api"), "tok")
        .expect("client")
        .with_fast_timings();
    let err = closed_client
        .get_deposition(DepositionId(9))
        .await
        .expect_err("unreachable endpoint must fail");
    assert!(format!("{err:#}").contains("failed"), "{err:#}");
}
