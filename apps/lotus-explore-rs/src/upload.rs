// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Browser file extraction and streaming line reads.
//! Native stubs preserve signature parity; they are inert by design.

#![allow(clippy::unused_async)]
#![allow(clippy::unused_self)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::needless_pass_by_ref_mut)]
#![allow(clippy::doc_markdown)]

#[cfg(target_arch = "wasm32")]
use gloo_timers::future::TimeoutFuture;
#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Uint8Array};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::{HtmlAnchorElement, Url};

/// Browser `Blob`; `()` on native.
#[cfg(target_arch = "wasm32")]
pub type UploadBlob = web_sys::Blob;

/// Unused on native.
#[cfg(not(target_arch = "wasm32"))]
pub type UploadBlob = ();

/// A file extracted from a form-data or drag-drop event.
#[derive(Debug)]
pub struct ExtractedFile {
    /// Blob for streaming reads.
    pub blob: UploadBlob,
}

/// Blob read error.
#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    /// A JS call failed.
    #[error("blob read error: {0}")]
    UploadBlob(String),

    /// Browser-only operation, raised by the native stubs.
    #[cfg(not(target_arch = "wasm32"))]
    #[error("download is only available in the browser")]
    BrowserOnly,

    /// App-level validation error, raised by the native stubs.
    #[cfg(not(target_arch = "wasm32"))]
    #[error("{0}")]
    Other(String),
}

impl UploadError {
    /// Wrap a message.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

#[cfg(target_arch = "wasm32")]
impl From<wasm_bindgen::JsValue> for UploadError {
    fn from(value: wasm_bindgen::JsValue) -> Self {
        use wasm_bindgen::JsCast;
        Self::UploadBlob(value.dyn_ref::<js_sys::JsString>().map_or_else(
            || format!("{value:?}"),
            |s| s.as_string().unwrap_or_default(),
        ))
    }
}

/// Chunk size for blob reads.
const CHUNK_SIZE: usize = 16 * 1024 * 1024;

/// Chunked line reader over a blob; yields lines without trailing `\n`/`\r`.
#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
pub struct UploadBlobLines {
    blob: UploadBlob,
    /// The blob's size, which bounds the chunk reads. Not exposed: nothing
    /// outside this module reads it, and an accessor nobody calls is an
    /// accessor someone has to keep correct.
    total_bytes: u64,
    offset: u64,
    buffer: Vec<u8>,
    buf_start: usize,
}

#[cfg(target_arch = "wasm32")]
impl UploadBlobLines {
    /// New reader for `blob`.
    #[must_use]
    pub fn new(blob: &UploadBlob) -> Self {
        // `Blob::size()` is f64; exact well below 2^53 for any real upload.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let total_bytes = blob.size() as u64;
        Self {
            blob: blob.clone(),
            total_bytes,
            offset: 0,
            buffer: Vec::with_capacity(CHUNK_SIZE),
            buf_start: 0,
        }
    }

    /// Next line, or `Ok(None)` at end of stream.
    pub async fn next_line(&mut self) -> Result<Option<String>, UploadError> {
        loop {
            // Complete line already buffered?
            if let Some(line) = self.take_line_from_buffer() {
                return Ok(Some(line));
            }

            // Otherwise check for EOF.
            if self.offset >= self.total_bytes {
                if self.buf_start < self.buffer.len() {
                    // `buf_start <= buffer.len()` is an invariant.
                    let tail = self.buffer.get(self.buf_start..).unwrap_or_default();
                    let remaining = String::from_utf8_lossy(tail).into_owned();
                    self.buf_start = self.buffer.len();
                    return Ok(Some(remaining));
                }
                return Ok(None);
            }

            self.load_next_chunk().await?;
        }
    }

    fn take_line_from_buffer(&mut self) -> Option<String> {
        // `buf_start` and `pos` both come from this slice, so they are in bounds.
        let available = self.buffer.get(self.buf_start..).unwrap_or_default();
        if let Some(pos) = available.iter().position(|b| *b == b'\n') {
            let line_bytes = available.get(..pos).unwrap_or_default();
            let mut line = String::from_utf8_lossy(line_bytes).into_owned();
            self.buf_start += pos + 1;
            if line.ends_with('\r') {
                line.pop();
            }
            // Compact once mostly consumed.
            if self.buf_start > self.buffer.len() / 2 {
                self.buffer.drain(..self.buf_start);
                self.buf_start = 0;
            }
            Some(line)
        } else {
            None
        }
    }

    async fn load_next_chunk(&mut self) -> Result<(), UploadError> {
        let start = self.offset;
        let end = (self.offset + CHUNK_SIZE as u64).min(self.total_bytes);
        // The JS binding takes f64; offsets stay well below 2^53.
        #[allow(clippy::cast_precision_loss)]
        let (start_f64, end_f64) = (start as f64, end as f64);
        let slice = self
            .blob
            .slice_with_f64_and_f64(start_f64, end_f64)
            .map_err(UploadError::from)?;
        let bytes = JsFuture::from(slice.array_buffer()).await?;
        let array = Uint8Array::new(&bytes);
        let chunk_len = array.byte_length() as usize;
        let mut chunk_bytes = vec![0u8; chunk_len];
        array.copy_to(&mut chunk_bytes);
        self.buffer.extend_from_slice(&chunk_bytes);
        self.offset = end;
        // Yield so the UI stays responsive between chunks.
        TimeoutFuture::new(0).await;
        Ok(())
    }
}

/// Native stub.
#[cfg(not(target_arch = "wasm32"))]
pub struct UploadBlobLines;

#[cfg(not(target_arch = "wasm32"))]
impl UploadBlobLines {
    #[must_use]
    pub fn new(_blob: &UploadBlob) -> Self {
        Self
    }

    pub async fn next_line(&mut self) -> Result<Option<String>, UploadError> {
        Err(UploadError::BrowserOnly)
    }
}

/// Extract a blob from the first entry of `evt.data().files()`.
/// # Errors
/// Returns a message if the file is not a `Blob`.
#[allow(clippy::unnecessary_wraps)]
// The wasm path returns `Err` for unsupported types, so `Result` is genuinely
// used; the native branch only returns `Ok(None)`, which trips the lint.
pub fn extract_blob_from_file_data(
    files: &[dioxus::html::FileData],
) -> Result<Option<ExtractedFile>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        type WebFile = web_sys::File;

        let Some(file) = files.iter().next() else {
            return Ok(None);
        };

        let Some(web_file) = file.inner().downcast_ref::<WebFile>() else {
            return Err("This file type is not supported in the browser.".to_string());
        };

        let blob = web_file
            .clone()
            .dyn_into::<UploadBlob>()
            .map_err(|_| "Unable to read the selected file as a blob.".to_string())?;
        Ok(Some(ExtractedFile { blob }))
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = files;
        Ok(None)
    }
}

/// Stream a blob to a string; avoids loading the whole file at once.
/// # Errors
/// Returns an error if the blob cannot be read or contains invalid UTF-8.
#[cfg(target_arch = "wasm32")]
pub async fn read_blob_string(blob: &UploadBlob) -> Result<String, UploadError> {
    let mut reader = UploadBlobLines::new(blob);
    let mut out = String::new();
    while let Some(line) = reader.next_line().await? {
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

/// Native stub.
/// # Errors
/// Always returns `Err`.
#[cfg(not(target_arch = "wasm32"))]
pub async fn read_blob_string(_blob: &UploadBlob) -> Result<String, UploadError> {
    Err(UploadError::other(
        "read_blob_string only available on WASM targets",
    ))
}

/// Strip control characters; replace path separators and quotes with `_`.
#[must_use]
pub fn sanitize_filename(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.trim().chars() {
        if c.is_control() {
            continue;
        }
        match c {
            '/' | '\\' | '"' | '\'' | '\n' | '\r' => out.push('_'),
            _ => out.push(c),
        }
    }
    out.trim_matches('.').trim().to_string()
}

#[cfg(target_arch = "wasm32")]
fn blob_url_from_str(content: &str, mime: &str) -> Result<String, String> {
    let parts = Array::new();
    parts.push(&JsValue::from_str(content));

    let blob = {
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(mime);
        UploadBlob::new_with_str_sequence_and_options(&parts, &options)
            .or_else(|_| UploadBlob::new_with_str_sequence(&parts))
    };
    let blob = blob.map_err(|e| format!("failed to create blob: {e:?}"))?;
    Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("failed to create object URL: {e:?}"))
}

/// Download `content` as `{filename}{extension}` with `mime`.
/// # Errors
/// Returns a message if the download cannot be triggered.
#[cfg(target_arch = "wasm32")]
pub fn download_text_as_blob(
    content: &str,
    filename: &str,
    extension: &str,
    mime: &str,
) -> Result<(), String> {
    let safe_name = if extension.is_empty() {
        sanitize_filename(filename)
    } else {
        let name = sanitize_filename(filename);
        if name.ends_with(extension) {
            name
        } else {
            format!("{name}{extension}")
        }
    };
    let url = blob_url_from_str(content, mime)?;
    click_download_anchor(&url, &safe_name, false)
        .map(|_| ())
        .map_err(|e| format!("download failed: {e}"))
}

/// Triggers a browser download of a URL (e.g. a QLever export URL or a remote file).
/// Opens the URL in a new tab / triggers an anchor click.  Returns `false` if
/// the browser does not support programmatic clicks (extremely rare).
#[cfg(target_arch = "wasm32")]
pub fn download_url(url: &str, filename: &str) -> bool {
    let safe_name = sanitize_filename(filename);
    click_download_anchor(url, &safe_name, true).unwrap_or_else(|_| {
        web_sys::window()
            .and_then(|w| w.open_with_url(url).ok())
            .is_some()
    })
}

#[cfg(target_arch = "wasm32")]
fn click_download_anchor(href: &str, filename: &str, new_tab: bool) -> Result<bool, String> {
    let window = web_sys::window().ok_or("no window object")?;
    let document = window.document().ok_or("no document object")?;
    let anchor: HtmlAnchorElement = document
        .create_element("a")
        .map_err(|e| format!("failed to create anchor: {e:?}"))?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|e| format!("failed to cast anchor: {e:?}"))?;

    anchor.set_href(href);
    anchor.set_download(filename);
    anchor.set_rel("noopener noreferrer");
    if new_tab {
        anchor.set_target("_blank");
    }

    let body = document.body().ok_or("no document body")?;
    body.append_child(&anchor)
        .map_err(|e| format!("failed to append anchor: {e:?}"))?;
    anchor.click();
    let _ = body.remove_child(&anchor);

    Ok(true)
}

/// POST a hidden form to download; for payloads too large for GET.
/// # Errors
/// Returns a message if the form cannot be created or submitted.
#[cfg(target_arch = "wasm32")]
pub async fn submit_download_form(endpoint: &str, fields: &[(&str, &str)]) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window object")?;
    let document = window.document().ok_or("no document object")?;

    let form = document
        .create_element("form")
        .map_err(|e| format!("failed to create form: {e:?}"))?
        .dyn_into::<web_sys::HtmlFormElement>()
        .map_err(|e| format!("failed to cast form: {e:?}"))?;
    form.set_method("POST");
    form.set_action(endpoint);
    form.set_target("_blank");
    let _ = form.set_attribute("accept-charset", "UTF-8");
    let _ = form.set_attribute("enctype", "application/x-www-form-urlencoded");

    for (name, value) in fields {
        let input = document
            .create_element("input")
            .map_err(|e| format!("failed to create input {name}: {e:?}"))?
            .dyn_into::<web_sys::HtmlInputElement>()
            .map_err(|e| format!("failed to cast input {name}: {e:?}"))?;
        input.set_type("hidden");
        input.set_name(name);
        input.set_value(value);
        form.append_child(&input)
            .map_err(|e| format!("failed to append input {name}: {e:?}"))?;
    }

    let body = document.body().ok_or("no document body")?;
    body.append_child(&form)
        .map_err(|e| format!("failed to append form: {e:?}"))?;
    form.submit()
        .map_err(|e| format!("failed to submit form: {e:?}"))?;
    let _ = body.remove_child(&form);

    // Awaited only to advance the microtask queue so the submit takes effect.
    #[allow(clippy::ignored_unit_patterns)]
    let _ = TimeoutFuture::new(0).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    // The panic lints keep shipped code free of panics on external input. A test
    // that fails to create its temp dir is reporting, not panicking.
    #![allow(clippy::expect_used)]

    use super::sanitize_filename;

    #[test]
    fn sanitize_removes_path_separators() {
        assert_eq!(sanitize_filename("a/b\\c"), "a_b_c");
    }

    #[test]
    fn sanitize_strips_control_chars() {
        assert_eq!(sanitize_filename("file\x00name"), "filename");
    }

    #[test]
    fn sanitize_strips_leading_dots() {
        assert_eq!(sanitize_filename("...file.txt"), "file.txt");
    }

    #[test]
    fn sanitize_empty_input() {
        assert_eq!(sanitize_filename("   "), "");
        assert_eq!(sanitize_filename("."), "");
    }

    #[test]
    fn sanitize_replaces_quotes_with_underscore() {
        assert_eq!(sanitize_filename("file\"name"), "file_name");
        assert_eq!(sanitize_filename("file'name"), "file_name");
    }

    #[test]
    fn sanitize_preserves_safe_names() {
        assert_eq!(sanitize_filename("lotus_results.csv"), "lotus_results.csv");
        assert_eq!(sanitize_filename("my_file-01.json"), "my_file-01.json");
    }

    #[test]
    fn sanitize_strips_trailing_whitespace() {
        assert_eq!(sanitize_filename("file.txt "), "file.txt");
        assert_eq!(sanitize_filename(" file.txt"), "file.txt");
    }

    #[test]
    fn sanitize_unicode_passthrough() {
        assert_eq!(sanitize_filename("résultats.csv"), "résultats.csv");
        assert_eq!(sanitize_filename("α-β-γ.rdf"), "α-β-γ.rdf");
    }
}
