// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Handing a file or a URL to the user, on a native build.
//!
//! The browser asks the browser: a blob URL and an anchor click, and the browser
//! shows its own download UI. A desktop window has neither, so this module does
//! the two things a window can do -- write a file, and hand a URL to the system
//! browser.
//!
//! It lives beside `native.rs` rather than in `upload.rs` because it is the write
//! side. `upload.rs` is the read side: taking a file the user picked apart.

use std::path::{Path, PathBuf};

use super::super::upload::sanitize_filename;
use super::DownloadError;

/// Write `content` to a file in the user's download directory.
///
/// Returns the path written, so the caller can tell the user where the file
/// went. A window has no download shelf and no browser notification, so a file
/// appearing in `~/Downloads` with nothing on screen is indistinguishable from a
/// button that did nothing.
///
/// # Errors
/// Returns a [`DownloadError`] (`Io`, `InvalidInput`, etc.; `Cancelled` is wasm-only)
/// if the download directory cannot be determined or the file cannot be written.
pub fn download_text(content: &str, filename: &str) -> Result<PathBuf, DownloadError> {
    download_bytes(content.as_bytes(), filename)
}

/// Write raw `bytes` to a file in the user's download directory.
///
/// # Errors
/// Returns a [`DownloadError`] (`Io`, `InvalidInput`, etc.; `Cancelled` is wasm-only)
/// if the download directory cannot be determined or the file cannot be written.
pub fn download_bytes(bytes: &[u8], filename: &str) -> Result<PathBuf, DownloadError> {
    let dir = download_dir().ok_or_else(|| {
        DownloadError::Io(
            "no download directory could be determined; set LOTUS_DOWNLOAD_DIR".to_string(),
        )
    })?;
    download_bytes_in(&dir, bytes, filename)
}

/// Decode a `data:image/png;base64,...` URL and write the PNG into the user's
/// download directory.
///
/// # Errors
/// Returns a [`DownloadError`] if the data URL is malformed or the file cannot be
/// written (`Cancelled` is wasm-only).
pub fn save_png_data_url(data_url: &str, filename: &str) -> Result<PathBuf, DownloadError> {
    let encoded = data_url
        .strip_prefix("data:image/png;base64,")
        .ok_or_else(|| DownloadError::InvalidInput("expected a base64 PNG data URL".to_string()))?;
    let bytes = decode_base64(encoded)?;
    download_bytes(&bytes, filename)
}

/// [`download_text`], writing into a directory the caller chose.
///
/// Split out so the write path can be tested without touching the process
/// environment, which is shared state and unsafe to mutate from a parallel test.
pub fn download_text_in(
    dir: &Path,
    content: &str,
    filename: &str,
) -> Result<PathBuf, DownloadError> {
    download_bytes_in(dir, content.as_bytes(), filename)
}

/// [`download_bytes`], writing into a directory the caller chose.
///
/// # Errors
/// Returns a [`DownloadError`] if writing fails (`Cancelled` is wasm-only).
pub fn download_bytes_in(
    dir: &Path,
    bytes: &[u8],
    filename: &str,
) -> Result<PathBuf, DownloadError> {
    // `sanitize_filename` strips path separators, so the name cannot escape `dir`.
    let name = sanitize_filename(filename);
    let name = if name.is_empty() {
        "lotus-export".to_string()
    } else {
        name
    };

    let path = unique_path(dir, &name);
    std::fs::write(&path, bytes)
        .map_err(|e| DownloadError::Io(format!("failed to write {}: {e}", path.display())))?;
    log::info!(
        "event=download phase=write state=success path={}",
        path.display()
    );
    Ok(path)
}

fn decode_base64(input: &str) -> Result<Vec<u8>, DownloadError> {
    let mut out = Vec::with_capacity((input.len() / 4) * 3);
    let mut acc = 0u32;
    let mut bits = 0u8;
    for byte in input.bytes() {
        if byte.is_ascii_whitespace() || byte == b'=' {
            continue;
        }
        let val = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => {
                return Err(DownloadError::InvalidInput(format!(
                    "invalid base64 byte: {byte}"
                )));
            }
        };
        acc = (acc << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).unwrap_or(0));
        }
    }
    Ok(out)
}

/// Where an export should be written.
///
/// `LOTUS_DOWNLOAD_DIR` wins, so a scripted run can be pointed at a specific
/// directory. Otherwise the platform convention: `~/Downloads`, with the XDG
/// variable honoured first on Linux.
///
/// Resolved via standard environment variables without pulling in an external
/// directory-lookup dependency.
fn download_dir() -> Option<PathBuf> {
    let non_empty = |v: Option<std::ffi::OsString>| v.filter(|v| !v.is_empty()).map(PathBuf::from);

    if let Some(dir) = non_empty(std::env::var_os("LOTUS_DOWNLOAD_DIR")) {
        return Some(dir);
    }
    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
        return Some(PathBuf::from(home).join("Downloads"));
    }
    if let Some(dir) = non_empty(std::env::var_os("XDG_DOWNLOAD_DIR")) {
        return Some(dir);
    }
    Some(PathBuf::from(std::env::var_os("HOME")?).join("Downloads"))
}

/// First unused path in `dir` for `name`, appending ` (2)`, ` (3)`, ...
///
/// An export never overwrites an earlier one. Silently replacing a file the user
/// meant to keep is worse than the collision being visible in the name.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (name.to_string(), String::new()),
    };
    let mut candidate = dir.join(name);
    let mut n = 2_u32;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    candidate
}

/// Open `url` in the user's default browser.
///
/// This is what the toolbar's `QLever` button does in a window. That button
/// used to carry only a `wasm32` branch, so in a desktop build it did nothing.
///
/// # Errors
/// Returns a message if the URL is not http(s) or the opener fails. Only http
/// and https are accepted, because the URL is handed to an external program.
pub fn open_externally(url: &str) -> Result<(), DownloadError> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(DownloadError::InvalidInput(format!(
            "refusing to open a non-http URL: {url}"
        )));
    }

    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("open", &[url])
    } else if cfg!(target_os = "windows") {
        // `start` is a shell builtin, so this needs a shell. The empty string is
        // the window title, which `start` would otherwise take from the URL.
        ("cmd", &["/c", "start", "", url])
    } else {
        ("xdg-open", &[url])
    };

    std::process::Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| DownloadError::Io(format!("failed to run {program}: {e}")))
}

#[cfg(test)]
#[path = "local_file/tests.rs"]
mod tests;
