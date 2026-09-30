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

/// Write `content` to a file in the user's download directory.
///
/// Returns the path written, so the caller can tell the user where the file
/// went. A window has no download shelf and no browser notification, so a file
/// appearing in `~/Downloads` with nothing on screen is indistinguishable from a
/// button that did nothing.
///
/// # Errors
/// Returns a message if the download directory cannot be determined or the file
/// cannot be written.
pub fn download_text(content: &str, filename: &str) -> Result<PathBuf, String> {
    let dir = download_dir().ok_or_else(|| {
        "no download directory could be determined; set LOTUS_DOWNLOAD_DIR".to_string()
    })?;
    download_text_in(&dir, content, filename)
}

/// [`download_text`], writing into a directory the caller chose.
///
/// Split out so the write path can be tested without touching the process
/// environment, which is shared state and unsafe to mutate from a parallel test.
pub fn download_text_in(dir: &Path, content: &str, filename: &str) -> Result<PathBuf, String> {
    // `sanitize_filename` strips path separators, so the name cannot escape `dir`.
    let name = sanitize_filename(filename);
    let name = if name.is_empty() {
        "lotus-export".to_string()
    } else {
        name
    };

    let path = unique_path(dir, &name);
    std::fs::write(&path, content)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    log::info!(
        "event=download phase=write state=success path={}",
        path.display()
    );
    Ok(path)
}

/// Where an export should be written.
///
/// `LOTUS_DOWNLOAD_DIR` wins, so a scripted run can be pointed at a specific
/// directory. Otherwise the platform convention: `~/Downloads`, with the XDG
/// variable honoured first on Linux.
///
/// Spelled out rather than taken from `dirs`, which is only enabled for the
/// `desktop` feature while this function compiles for every native target.
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
pub fn open_externally(url: &str) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("refusing to open a non-http URL: {url}"));
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
        .map_err(|e| format!("failed to run {program}: {e}"))
}

#[cfg(test)]
mod tests {
    // A test that fails to make its temp dir is reporting, not panicking.
    #![allow(clippy::expect_used)]

    use std::path::PathBuf;

    use super::{download_text_in, unique_path};

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lotus-local-file-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// A second export of the same file must not overwrite the first, and the
    /// extension has to survive the counter.
    #[test]
    fn a_repeated_export_does_not_overwrite_the_first() {
        let dir = scratch("repeat");
        assert_eq!(unique_path(&dir, "results.csv"), dir.join("results.csv"));

        std::fs::write(dir.join("results.csv"), b"first").expect("write");
        assert_eq!(
            unique_path(&dir, "results.csv"),
            dir.join("results (2).csv")
        );

        std::fs::write(dir.join("results (2).csv"), b"second").expect("write");
        assert_eq!(
            unique_path(&dir, "results.csv"),
            dir.join("results (3).csv")
        );

        // A name that is already a counter is still checked, not assumed free.
        std::fs::write(dir.join("results (3).csv"), b"third").expect("write");
        assert_eq!(
            unique_path(&dir, "results (3).csv"),
            dir.join("results (3) (2).csv"),
        );

        // No extension: the counter goes after the whole name.
        std::fs::write(dir.join("export"), b"x").expect("write");
        assert_eq!(unique_path(&dir, "export"), dir.join("export (2)"));

        // A dotfile has no stem, so it is treated as a bare name.
        assert_eq!(unique_path(&dir, ".gitignore"), dir.join(".gitignore"));

        assert_eq!(
            std::fs::read(dir.join("results.csv")).expect("read"),
            b"first",
            "the first export must be left intact"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The end-to-end path: a written file exists, holds the content, and the
    /// caller is told where it went.
    #[test]
    fn a_written_export_reports_its_path_and_keeps_its_content() {
        let dir = scratch("write");

        let path = download_text_in(&dir, "id,name\nQ153,Ethanol\n", "results.csv").expect("write");
        assert_eq!(path, dir.join("results.csv"), "the path is reported back");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "id,name\nQ153,Ethanol\n"
        );

        let again = download_text_in(&dir, "second", "results.csv").expect("write");
        assert_eq!(again, dir.join("results (2).csv"), "the first is kept");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A name is chosen by the server, and a name containing a path separator
    /// must not be able to write outside the download directory.
    #[test]
    fn a_traversal_in_the_filename_cannot_escape_the_download_directory() {
        let dir = scratch("traversal");

        let path = download_text_in(&dir, "x", "../../escaped.csv").expect("write");
        assert_eq!(
            path.parent().expect("a parent"),
            dir,
            "{path:?} escaped the download directory"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A non-http URL would be handed to an external program, so it is refused.
    #[test]
    fn only_web_urls_are_handed_to_the_system_opener() {
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,<script>",
            "",
        ] {
            let err = super::open_externally(bad).expect_err("should refuse");
            assert!(err.contains("non-http"), "{bad:?} was not refused: {err}");
        }
    }
}
