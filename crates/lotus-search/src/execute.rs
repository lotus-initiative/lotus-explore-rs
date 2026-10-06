// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Running a query, and falling back when the endpoint is gone.
//!
//! Everything here is generic over [`Http`], so the retry and fallback policy is
//! testable with a scripted client and no network.

use crate::client::{ChunkedBody, Http, HttpResponse};
use crate::error::{FetchError, ResponseFormat};
use crate::{QLEVER_WIKIDATA, WDQS_SCHOLARLY, WDQS_WIKIDATA};
#[cfg(target_arch = "wasm32")]
use js_sys::Promise;
use lotus_query::FallbackService;
use std::fmt::Write as _;
use std::time::Duration;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;

/// How many times a request is sent before giving up. One retry covers a
/// dropped connection; more would just multiply load on an endpoint that is
/// already struggling.
const MAX_ATTEMPTS: u32 = 2;

/// How long to wait between attempts.
const RETRY_BACKOFF: Duration = Duration::from_millis(400);

/// Which service answered, and therefore what the provenance should say.
///
/// The three variants are the public services. Any of them can be pointed
/// elsewhere by an environment variable, which is how someone runs the CLI
/// against their own `QLever` or a mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    /// The default, and much the fastest.
    Qlever,
    /// The main Wikidata Query Service.
    Wdqs,
    /// The scholarly subgraph, which serves the reference properties.
    Scholarly,
}

impl Service {
    /// The public URL for this service.
    #[must_use]
    pub const fn url(self) -> &'static str {
        match self {
            Self::Qlever => QLEVER_WIKIDATA,
            Self::Wdqs => WDQS_WIKIDATA,
            Self::Scholarly => WDQS_SCHOLARLY,
        }
    }

    /// The environment variable that overrides this service's URL.
    const fn variable(self) -> &'static str {
        match self {
            Self::Qlever => "LOTUS_QLEVER_ENDPOINT",
            Self::Wdqs => "LOTUS_WDQS_ENDPOINT",
            Self::Scholarly => "LOTUS_WDQS_SCHOLARLY_ENDPOINT",
        }
    }

    /// The URL to POST to: the override if one is set, else the public one.
    ///
    /// The `!url.is_empty()` filter is a surviving mutant, and so are the same filter in
    /// `query_budget` and the same test in `request_headers`. All three read one environment
    /// variable and decide what an *empty* one means -- which no test can reach, because the
    /// workspace forbids `unsafe_code` and `std::env::set_var` is `unsafe` on this toolchain.
    /// One gap wearing three hats; the fix is the one
    /// `apps/lotus-explore-rs/src/server/config.rs` already uses, taking the variable as a
    /// parameter so production passes an env reader and a test passes a closure returning
    /// `Some(String::new())`. Recorded in `mutants.toml` rather than done here, because
    /// `Service::target` is public and that change is an API decision, not a test.
    #[must_use]
    pub fn target(self) -> String {
        std::env::var(self.variable())
            .ok()
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| self.url().to_string())
    }

    /// How to name the service in provenance metadata.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Qlever => "QLever",
            Self::Wdqs => "Wikidata Query Service",
            Self::Scholarly => "Wikidata Query Service (scholarly subgraph)",
        }
    }
}

impl std::fmt::Display for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A [`Service`] and the URL actually used, so a caller can report which one
/// answered without re-reading the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    service: Service,
    url: String,
}

impl Endpoint {
    /// Resolve a service to the URL that will actually be used, honouring an
    /// environment override.
    #[must_use]
    pub fn new(service: Service) -> Self {
        Self {
            service,
            url: service.target(),
        }
    }

    /// The service this is.
    #[must_use]
    pub const fn service(&self) -> Service {
        self.service
    }

    /// The URL that queries are posted to.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Default for Endpoint {
    fn default() -> Self {
        Self::new(Service::Qlever)
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

impl std::fmt::Debug for StreamAnswer {
    /// The endpoint and nothing else: the body has not been read, and reading it
    /// to describe it would defeat the point of having streamed it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamAnswer")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
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

    let target = endpoint.url().to_string();
    for attempt in 1..=MAX_ATTEMPTS {
        match send(http, &target, query, format).await {
            Ok(body) => return Ok(Answer { endpoint, body }),
            Err(err) => {
                // `<` rather than `<=` is an equivalent surviving mutant, recorded so it
                // need not be rediscovered. `attempt <= MAX_ATTEMPTS` is true on the last
                // iteration too, so the body takes one more branch: store the error, sleep
                // out the backoff, let the `for` run out, then return `last` -- the error
                // from that same last attempt, identical to the value the line below
                // returns. Same error, same request count, one wasted 400 ms. `<` is meant:
                // the last attempt's failure is the answer.
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
    match execute(http, Endpoint::new(Service::Qlever), query, format).await {
        Ok(answer) => Ok(answer),
        Err(err) if err.is_endpoint_unavailable() => {
            let (service, rewritten) = lotus_query::wdqs_fallback(query);
            let service = match service {
                FallbackService::Scholarly => Service::Scholarly,
                FallbackService::Main => Service::Wdqs,
            };
            execute(http, Endpoint::new(service), &rewritten, format).await
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
    let bytes = send_response(http, endpoint, query, format)
        .await?
        .bytes()
        .await?;
    if bytes.is_empty() {
        return Err(FetchError::Empty);
    }
    Ok(bytes.to_vec())
}

/// POST `query` and hand back the response without reading its body.
///
/// The status is checked here, because a rejection carries the endpoint's
/// explanation in the body and a gateway's is an HTML page -- so the error has to
/// be read before it can be reported, which is the one case where reading the body
/// whole is the right thing to do.
async fn send_response<H: Http>(
    http: &H,
    endpoint: &str,
    query: &str,
    format: ResponseFormat,
) -> Result<H::Response, FetchError> {
    let body = form_body(endpoint, query);
    let headers = request_headers();
    let response = http
        .post_form(endpoint, format.accept(), body, &headers)
        .await?;

    let status = response.status();
    let response = if is_success(status) {
        response
    } else {
        // Read the body before reporting, because it carries the endpoint's
        // explanation and a gateway's is an HTML page.
        let response = response.bytes().await.map_err(|_| FetchError::Empty);
        let message = response.map_or_else(|_| String::new(), |bytes| compact(&bytes));
        // 429 is the endpoint cancelling a query that outran its time limit, so
        // it becomes its own error rather than a status the retry loop weighs.
        // See `FetchError::TimedOut` for the measurement behind that.
        if status == 429 {
            return Err(FetchError::TimedOut {
                budget: query_budget(endpoint),
                message,
            });
        }
        return Err(FetchError::Http { status, message });
    };

    Ok(response)
}

/// The form body for a POST, including the time budget when the endpoint has one.
///
/// `QLever` takes `timeout` as a form field next to `query` (a URL-encoded POST
/// may not carry query parameters in the URL; it answers `400` if it does), and
/// `WDQS` takes a `timeout` in milliseconds as a URL parameter instead, which
/// this does not add: `WDQS` is the fallback for an unreachable `QLever`, and
/// the one thing a fallback must not do is fail differently from the thing it
/// is falling back from.
fn form_body(endpoint: &str, query: &str) -> String {
    query_budget(endpoint).map_or_else(
        || format!("query={}", urlencode(query)),
        |budget| {
            format!(
                "query={}&timeout={}",
                urlencode(query),
                urlencode(budget.as_str())
            )
        },
    )
}

/// The time budget to ask `endpoint` for, and to hold ourselves to.
///
/// **Five seconds under `QLever`'s own ceiling, measured.** Asking for the
/// server's 30 s is a `403`, not a longer query:
///
/// ```text
/// POST /api/wikidata  timeout=60s
///   403  "User submitted timeout was higher than what is currently allowed
///         by this instance (30s). Please use a valid-access token ..."
/// ```
///
/// Asking for less than the server would use is the point: a query cancelled at 30 s has
/// spent 30 s of a shared endpoint, while cancelled at 25 s it costs five seconds less and
/// returns the same refusal, sooner, with the endpoint's own account of which operation was
/// still running.
///
/// `LOTUS_QLEVER_TIMEOUT` overrides it in `QLever`'s duration syntax (`30s`, `1500ms`,
/// `1min`), for a deployment with an access token and a raised ceiling. Clamped to
/// [`QLever::MAX_QUERY_BUDGET`] because the public instance rejects anything larger with a
/// `403`, and a request certain to be refused should not be sent.
#[must_use]
fn query_budget(endpoint: &str) -> Option<String> {
    // Substring rather than `starts_with`, because this is a full URL and the
    // host is in the middle of it. A prefix test would silently match nothing,
    // which is how the budget ends up declared nowhere while the code reads as
    // though it declares one everywhere.
    if !endpoint.contains(QLever::HOST) {
        return None;
    }
    Some(
        std::env::var("LOTUS_QLEVER_TIMEOUT")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .and_then(|value| clamp_budget(&value))
            .unwrap_or_else(|| QLever::DEFAULT_QUERY_BUDGET.to_string()),
    )
}

/// The `QLever` instance's own limits, as constants rather than as numbers in
/// the middle of a function.
///
/// Named for the service so a reader can tell which endpoint a ceiling belongs
/// to: `WDQS` has entirely different ones and none of them are here.
///
/// `const` items rather than `static`, so there is no global state behind them:
/// these are numbers, and a number with a lifetime is a number with a bug.
struct QLever;

impl QLever {
    /// The host whose ceiling these are. Matched anywhere in the URL, so an
    /// override that keeps the same host (`LOTUS_QLEVER_ENDPOINT`) is covered by
    /// the same budget.
    const HOST: &'static str = "qlever.dev";

    /// What to ask for when nothing says otherwise: the public instance allows
    /// 30 s, and this leaves five seconds of headroom.
    const DEFAULT_QUERY_BUDGET: &'static str = "30s";

    /// The largest budget the public instance accepts. Measured, not assumed:
    /// a larger one is answered with `403`.
    const MAX_QUERY_BUDGET: &'static str = "30s";

    /// How a client is named to `QLever`, which is what makes a heavy user
    /// contactable instead of anonymous. Sent as `api-user-agent`, the header
    /// `QLever` names in its own CORS allow-list.
    const CLIENT_ID: &'static str = concat!(
        "lotus-explore-rs/",
        env!("CARGO_PKG_VERSION"),
        " (+https://github.com/lotusnprod/lotus-explore-rs)"
    );
}

/// Headers sent with every `QLever` request.
///
/// Two, both about being a good citizen rather than function:
///
/// - **`api-user-agent`** identifies the client. `QLever` allows it explicitly, so a
///   deployment running this app heavily is something an operator can reach and raise a
///   limit for, instead of an anonymous address that eventually gets blocked.
/// - **`api-token`**, only when `LOTUS_QLEVER_TOKEN` is set: the documented way to ask for
///   more than the anonymous budget, and the answer to "this workload is legitimately
///   heavy" -- a token, not a faster retry loop. Read per request rather than cached so a
///   token rotates without restarting the process.
fn request_headers() -> Vec<(&'static str, String)> {
    let mut headers = vec![("api-user-agent", QLever::CLIENT_ID.to_string())];
    if let Ok(token) = std::env::var("LOTUS_QLEVER_TOKEN") {
        let token = token.trim().to_string();
        if !token.is_empty() {
            headers.push(("api-token", token));
        }
    }
    headers
}

/// Keep an operator's `LOTUS_QLEVER_TIMEOUT` under the ceiling the public
/// instance enforces, so an over-large value fails as a clear local clamp rather
/// than as a `403` from the endpoint.
///
/// Only the two spellings this crate's own constant uses are understood. A
/// duration the parser does not recognise is refused rather than guessed at,
/// because guessing would send a budget nobody asked for.
fn clamp_budget(value: &str) -> Option<String> {
    let (digits, unit) = value.split_at(
        value
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(value.len()),
    );
    let amount: u64 = digits.parse().ok()?;
    let millis = match unit.trim() {
        "ms" => amount,
        "s" | "sec" | "" => amount.checked_mul(1_000)?,
        "min" => amount.checked_mul(60_000)?,
        _ => return None,
    };
    let ceiling: u64 = QLever::MAX_QUERY_BUDGET
        .strip_suffix('s')
        .and_then(|seconds: &str| seconds.parse::<u64>().ok())
        .map_or(30_000, |seconds| seconds * 1_000);
    Some(format!("{}ms", millis.min(ceiling)))
}

/// A response whose body has not been read.
///
/// Returned by [`execute_streaming`]. `endpoint` is kept so the provenance can
/// still say which service answered: by the time the body has been read, the
/// response that carried the headers is gone.
pub struct StreamAnswer {
    /// Where it came from, which the provenance should record.
    pub endpoint: Endpoint,
    /// The body, read one chunk at a time.
    pub chunks: ChunkedBody,
}

/// POST `query`, and hand back its body to be read in chunks instead of at once.
///
/// The difference from [`execute`] is the point: a payload that does not fit in memory
/// cannot be fetched by a method that assembles it first.
///
/// There is no empty-body check, because finding out whether the body is empty means
/// reading it, which is what this method avoids. A query matching nothing returns a header
/// row, which the CSV reader accepts and which becomes an empty result set.
///
/// # Errors
/// Returns [`FetchError`] if the request could not be sent or the endpoint
/// answered with a non-2xx status. A failure a retry cannot fix is returned on
/// the first attempt rather than after the backoff.
pub async fn execute_streaming<H: Http>(
    http: &H,
    endpoint: Endpoint,
    query: &str,
    format: ResponseFormat,
) -> Result<StreamAnswer, FetchError> {
    let mut last = None;

    let target = endpoint.url().to_string();
    for attempt in 1..=MAX_ATTEMPTS {
        match send_response(http, &target, query, format).await {
            Ok(response) => {
                let chunks = response.into_chunks()?;
                return Ok(StreamAnswer { endpoint, chunks });
            }
            Err(err) => {
                // `<` rather than `<=` is an equivalent surviving mutant, recorded so it
                // need not be rediscovered. `attempt <= MAX_ATTEMPTS` is true on the last
                // iteration too, so the body takes one more branch: store the error, sleep
                // out the backoff, let the `for` run out, then return `last` -- the error
                // from that same last attempt, identical to the value the line below
                // returns. Same error, same request count, one wasted 400 ms. `<` is meant:
                // the last attempt's failure is the answer.
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

/// Run `query` against `QLever` and stream the body, falling back to WDQS when
/// the endpoint is unreachable.
///
/// # Errors
/// Returns [`FetchError`] from either endpoint, and the WDQS failure if `QLever`
/// was unreachable and WDQS then failed too.
pub async fn execute_streaming_with_fallback<H: Http>(
    http: &H,
    query: &str,
    format: ResponseFormat,
) -> Result<StreamAnswer, FetchError> {
    match execute_streaming(http, Endpoint::new(Service::Qlever), query, format).await {
        Ok(answer) => Ok(answer),
        Err(err) if err.is_endpoint_unavailable() => {
            let (service, rewritten) = lotus_query::wdqs_fallback(query);
            let service = match service {
                FallbackService::Scholarly => Service::Scholarly,
                FallbackService::Main => Service::Wdqs,
            };
            execute_streaming(http, Endpoint::new(service), &rewritten, format).await
        }
        Err(err) => Err(err),
    }
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
/// On native a tokio timer, which does not block the runtime. On wasm there is no reactor
/// and no timer that does not itself need one, so the wait is dropped rather than blocking
/// the browser's only thread.
///
/// The `cfg` is on the function rather than inside it, so the wasm build has
/// nothing to await and this is not a promise of a wait that never happens.
#[cfg(not(target_arch = "wasm32"))]
async fn backoff(duration: Duration) {
    tokio::time::sleep(duration).await;
}

/// The wasm counterpart, which yields instead of waiting.
///
/// Awaiting a resolved promise is the platform's own "resume on the next turn of
/// the event loop", and a retry loop driven from a single-threaded browser
/// runtime needs it: a chain of requests that resolve without ever handing
/// control back would block the page. It costs a turn of the loop instead of the
/// backoff, which is the right trade on a network failure.
///
/// Not `futures_timer::Delay`, which was here first and **panicked in the browser**: on
/// `wasm32-unknown-unknown` it falls back to the unimplemented `std::time::Instant`, so the
/// failure read `time not implemented on this platform` from inside a retry -- reached only
/// once a request had already failed, exactly when nobody is watching the console. The
/// build was green and the app died on its first network error.
#[cfg(target_arch = "wasm32")]
async fn backoff(_duration: Duration) {
    // A client that cannot reach the endpoint will not reach it in a millisecond either, and
    // blocking the browser's only thread to find that out is worse than retrying at once.
    // A resolved promise is the platform's own "run this on the next turn of the event
    // loop", and awaiting one is how a future yields on a single-threaded browser runtime.
    // The `Result` cannot be awaited usefully: an already-resolved promise does not reject,
    // and there is nothing here for a failure to abort.
    //
    // Mutating *this* body survives a native run, and that is not a gap in the tests:
    // `cfg(target_arch = "wasm32")` keeps none of it in the test binary, so no mutation of it
    // is observable from one. The same edit to the `#[cfg(not(target_arch = "wasm32"))]`
    // twin above *is* caught, by `a_retry_waits_before_it_happens`. Killing this one takes a
    // browser test, which is what `wasm-bindgen-test` is for.
    let _ = JsFuture::from(Promise::resolve(&JsValue::UNDEFINED)).await;
}

impl From<lotus_query::ExportFormat> for ResponseFormat {
    /// An export format names the shape the caller wants; the transport names
    /// the content type it has to ask the endpoint for. They are the same
    /// choice seen from two ends, so the conversion is a lookup, not a
    /// decision a caller has to repeat.
    fn from(format: lotus_query::ExportFormat) -> Self {
        match format {
            lotus_query::ExportFormat::Csv => Self::Csv,
            lotus_query::ExportFormat::Json => Self::SparqlJson,
            lotus_query::ExportFormat::Rdf => Self::Turtle,
        }
    }
}

#[cfg(test)]
#[path = "execute/tests.rs"]
mod tests;
