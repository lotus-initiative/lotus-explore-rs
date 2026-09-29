// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Running a query, and falling back when the endpoint is gone.
//!
//! Everything here is generic over [`Http`], so the retry and fallback policy is
//! testable with a scripted client and no network.

use crate::client::{Http, HttpResponse};
use crate::error::{FetchError, ResponseFormat};
use crate::{QLEVER_WIKIDATA, WDQS_SCHOLARLY, WDQS_WIKIDATA};
use std::fmt::Write as _;
use std::time::Duration;

/// How many times a request is sent before giving up. One retry covers a
/// dropped connection; more would just multiply load on an endpoint that is
/// already struggling.
const MAX_ATTEMPTS: u32 = 2;

/// How long to wait between attempts.
const RETRY_BACKOFF: Duration = Duration::from_millis(400);

/// Which endpoint answered, and therefore what the provenance should say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Endpoint {
    /// The default, and much the fastest.
    #[default]
    Qlever,
    /// The main Wikidata Query Service.
    Wdqs,
    /// The scholarly subgraph, which serves the reference properties.
    Scholarly,
}

impl Endpoint {
    /// The service URL to POST to.
    #[must_use]
    pub const fn url(self) -> &'static str {
        match self {
            Self::Qlever => QLEVER_WIKIDATA,
            Self::Wdqs => WDQS_WIKIDATA,
            Self::Scholarly => WDQS_SCHOLARLY,
        }
    }

    /// How to name the endpoint in provenance metadata.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Qlever => "QLever",
            Self::Wdqs => "Wikidata Query Service",
            Self::Scholarly => "Wikidata Query Service (scholarly subgraph)",
        }
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A response, and where it came from.
#[derive(Debug, Clone)]
pub struct Answer {
    /// Where it came from, which the provenance should record.
    pub endpoint: Endpoint,
    /// The raw body, in the format that was asked for.
    pub body: Vec<u8>,
}

impl Answer {
    /// The body as UTF-8.
    ///
    /// # Errors
    /// Returns [`FetchError::Parse`] if the body is not UTF-8, which a SPARQL
    /// response is always meant to be.
    pub fn text(&self) -> Result<String, FetchError> {
        String::from_utf8(self.body.clone())
            .map_err(|e| FetchError::Parse(format!("the response was not UTF-8: {e}")))
    }
}

/// POST `query` to `endpoint` in `format`, retrying only what a retry can fix.
///
/// # Errors
/// Returns [`FetchError`] if the request could not be sent, if the endpoint
/// answered with a non-2xx status, or if it answered with an empty body. A
/// failure that a retry cannot fix is returned on the first attempt rather than
/// after the backoff.
pub async fn execute<H: Http>(
    http: &H,
    endpoint: Endpoint,
    query: &str,
    format: ResponseFormat,
) -> Result<Answer, FetchError> {
    let mut last = None;

    for attempt in 1..=MAX_ATTEMPTS {
        match send(http, endpoint.url(), query, format).await {
            Ok(body) => return Ok(Answer { endpoint, body }),
            Err(err) => {
                let worth_retrying = err.is_retryable() && attempt < MAX_ATTEMPTS;
                if !worth_retrying {
                    return Err(err);
                }
                last = Some(err);
                backoff(RETRY_BACKOFF).await;
            }
        }
    }

    Err(last.unwrap_or(FetchError::Empty))
}

/// Run `query` against `QLever`, falling back to WDQS when the endpoint is
/// unreachable.
///
/// A 4xx is *not* a reason to fall back: WDQS would reject the same query, and
/// trying would only double the load on a query that is wrong.
///
/// # Errors
/// Returns [`FetchError`] from either endpoint, and the WDQS failure if
/// `QLever` was unreachable and WDQS then failed too.
pub async fn execute_with_fallback<H: Http>(
    http: &H,
    query: &str,
    format: ResponseFormat,
) -> Result<Answer, FetchError> {
    match execute(http, Endpoint::Qlever, query, format).await {
        Ok(answer) => Ok(answer),
        Err(err) if err.is_endpoint_unavailable() => {
            let (endpoint, rewritten) = crate::wdqs_fallback(query);
            let endpoint = if endpoint == WDQS_SCHOLARLY {
                Endpoint::Scholarly
            } else {
                Endpoint::Wdqs
            };
            execute(http, endpoint, &rewritten, format).await
        }
        Err(err) => Err(err),
    }
}

/// GET `url`, for a prepared export URL that already carries its format.
///
/// # Errors
/// Returns [`FetchError`] if the URL could not be fetched or the server answered
/// with a non-2xx status. There is no retry: a prepared export URL is
/// idempotent, and the caller can decide.
pub async fn fetch_url<H: Http>(
    http: &H,
    url: &str,
    format: ResponseFormat,
) -> Result<Vec<u8>, FetchError> {
    let response = http.get(url, format.accept()).await?;
    let status = response.status();
    let body = response.bytes().await?;

    if !is_success(status) {
        return Err(FetchError::Http {
            status,
            message: compact(&body),
        });
    }
    Ok(body.to_vec())
}

async fn send<H: Http>(
    http: &H,
    endpoint: &str,
    query: &str,
    format: ResponseFormat,
) -> Result<Vec<u8>, FetchError> {
    let body = format!("query={}", urlencode(query));
    let response = http.post(endpoint, format.accept(), body).await?;

    let status = response.status();
    let response = if is_success(status) {
        response
    } else {
        // Read the body before reporting, because it carries the endpoint's
        // explanation and a gateway's is an HTML page.
        let response = response.bytes().await.map_err(|_| FetchError::Empty);
        match response {
            Ok(bytes) => {
                return Err(FetchError::Http {
                    status,
                    message: compact(&bytes),
                });
            }
            Err(_) => {
                return Err(FetchError::Http {
                    status,
                    message: String::new(),
                });
            }
        }
    };

    let bytes = response.bytes().await?;
    if bytes.is_empty() {
        return Err(FetchError::Empty);
    }
    Ok(bytes.to_vec())
}

const fn is_success(status: u16) -> bool {
    status >= 200 && status < 300
}

/// Reduce an error body to one line, preferring the endpoint's own `exception`
/// field over an HTML gateway page.
fn compact(body: &[u8]) -> String {
    const MAX: usize = 240;
    let text = String::from_utf8_lossy(body);
    let line = json_exception(&text)
        .or_else(|| html_title(&text))
        .or_else(|| {
            text.lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !matches!(*l, "{" | "}"))
        })
        .unwrap_or(&text)
        .trim_matches(',');

    if line.chars().count() <= MAX {
        return line.to_string();
    }
    let truncated: String = line.chars().take(MAX).collect();
    format!("{truncated}…")
}

/// The `<title>` of an error page, which is where a gateway puts the reason
/// (`502 Bad Gateway`). The markup around it says nothing.
fn html_title(text: &str) -> Option<&str> {
    if !text.contains('<') {
        return None;
    }
    let rest = text.split_once("<title>")?.1;
    let end = rest.find("</title>")?;
    let title = rest[..end].trim();
    (!title.is_empty()).then_some(title)
}

/// Pull `"exception": "…"` out of `QLever`'s error format. Hand-rolled, because
/// the alternative is a JSON dependency for one field of one error path.
fn json_exception(text: &str) -> Option<&str> {
    let rest = text.split_once("\"exception\"")?.1.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;

    let mut end = 0;
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '"' => return Some(&rest[..end.max(i)]),
            _ => end = i + c.len_utf8(),
        }
    }
    None
}

/// Percent-encode for `application/x-www-form-urlencoded`.
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*byte));
            }
            b' ' => out.push('+'),
            other => {
                out.push('%');
                let _ = write!(out, "{other:02X}");
            }
        }
    }
    out
}

/// Wait before the next attempt.
///
/// On native this is a tokio timer, which does not block the runtime. On wasm
/// there is no reactor, so it blocks the single browser thread — acceptable
/// because the backoff is short and a wasm client has nothing else to do.
async fn backoff(duration: Duration) {
    #[cfg(not(target_arch = "wasm32"))]
    tokio::time::sleep(duration).await;

    #[cfg(target_arch = "wasm32")]
    std::thread::sleep(duration);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_body_becomes_one_readable_line() {
        let body = br#"{"exception":"Variable ?s was not declared","bindings":[]}"#;
        let compacted = compact(body);
        assert_eq!(compacted, "Variable ?s was not declared");
    }

    #[test]
    fn a_gateway_pages_title_is_the_message_not_its_markup() {
        let body = b"<html>\n<head><title>502 Bad Gateway</title></head>\n</html>";
        assert_eq!(compact(body), "502 Bad Gateway");
    }

    #[test]
    fn a_plain_text_error_becomes_one_line() {
        let body = b"\n  Something went wrong.  \n";
        assert_eq!(compact(body), "Something went wrong.");
    }

    #[test]
    fn a_long_message_is_truncated_with_an_ellipsis() {
        let body = "x".repeat(500);
        let compacted = compact(body.as_bytes());
        assert!(compacted.chars().count() <= 241);
        assert!(compacted.ends_with('…'));
    }

    #[test]
    fn a_query_is_form_encoded() {
        assert_eq!(urlencode("a b"), "a+b");
        assert_eq!(urlencode("?s"), "%3Fs");
        assert_eq!(urlencode("a=b&c"), "a%3Db%26c");
        assert_eq!(
            urlencode("SELECT-1._~"),
            "SELECT-1._~",
            "unreserved is untouched"
        );
    }

    #[test]
    fn the_endpoints_are_the_three_public_services() {
        assert!(Endpoint::Qlever.url().contains("qlever"));
        assert!(Endpoint::Wdqs.url().contains("query.wikidata.org"));
        assert!(Endpoint::Scholarly.url().contains("query-scholarly"));
    }
}
