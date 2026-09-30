// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Fetching Ketcher and laying its release zip out on disk.
//!
//! Ketcher ships as a prebuilt zip rather than as a package, so this is an
//! extraction problem: decide which entries are worth writing, where they go
//! relative to the destination, and refuse the ones that would escape it.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, BufWriter, Cursor, Write};
use std::path::Path;

use reqwest::blocking::Client;
use zip::ZipArchive;

use super::http::{resolve_tagged_version, setting};

const DEFAULT_DIR: &str = "public/assets/ketcher";
const KETCHER_OWNER_REPO: &str = "epam/ketcher";
const KETCHER_LATEST_URL: &str = "https://github.com/epam/ketcher/releases/latest";

#[cfg(test)]
mod tests;

/// The release archive URL for a version, without its leading `v`.
fn release_url(version: &str) -> String {
    format!(
        "https://github.com/{KETCHER_OWNER_REPO}/releases/download/v{version}/ketcher-standalone-{version}.zip"
    )
}

/// macOS zip metadata that must never be extracted: the `__MACOSX/` tree and
/// `._`-prefixed resource forks.
///
/// The ketcher release zip ships these (it was archived on macOS), and the
/// asset pipeline copies whatever it finds.
fn is_macos_junk(name: &str) -> bool {
    if name == "__MACOSX" || name.starts_with("__MACOSX/") {
        return true;
    }
    let Some(file_name) = name.rsplit('/').next() else {
        return false;
    };
    file_name.starts_with("._")
}

/// Standalone entry bundles (and their licence files) that ketcher's `index.html`
/// never references -- only `main.<hash>.js` is loaded by the editor iframe.
fn is_unused_entry(name: &str) -> bool {
    let Some(file_name) = name.rsplit('/').next() else {
        return false;
    };
    let is_entry_bundle = file_name.starts_with("closable.")
        || file_name.starts_with("duo.")
        || file_name.starts_with("popup.");
    is_entry_bundle
        && (Path::new(file_name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("js"))
            || file_name.ends_with(".js.LICENSE.txt"))
}

/// Reject an archive entry that would be written outside the target directory.
///
/// This is the zip-slip guard. `fetch-assets` runs with the developer's
/// privileges, so a `..` segment in a downloaded release would otherwise write
/// anywhere on the machine.
fn is_unsafe_entry_path(relative: &str) -> bool {
    relative.is_empty()
        || relative.starts_with('/')
        // Per segment rather than `contains("..")`, which also rejects the
        // perfectly ordinary `a..b.js` and silently drops it from the vendor.
        || relative.split('/').any(|segment| segment == "..")
}

/// The single top-level directory every name in the archive shares, if there is
/// exactly one.
///
/// Ketcher's release zip nests its whole payload under `standalone/`, which would
/// otherwise put the editor two levels down. An archive with more than one
/// top-level entry gets nothing stripped: picking one of several is how a file
/// from the wrong place ends up on disk, so the layout is left as it is.
fn common_prefix<'a>(names: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut top_levels: BTreeSet<&str> = BTreeSet::new();
    for name in names {
        // `__MACOSX` and `._foo` count as top-level entries if they are not
        // filtered here, which reads as two top levels and strips nothing --
        // leaving the editor one level too deep, with no error anywhere.
        if is_macos_junk(name) {
            continue;
        }
        let Some(first) = name.split('/').next() else {
            continue;
        };
        if !first.is_empty() {
            top_levels.insert(first);
        }
    }
    (top_levels.len() == 1)
        .then(|| top_levels.into_iter().next())
        .flatten()
        .map(str::to_owned)
}

/// What the extraction did, for the one line it prints at the end.
///
/// A type rather than two loose counters because the two are only ever read
/// together, and the "did we skip anything" test was a bare `> 0` on a number
/// nothing else could set.
#[derive(Debug, Default)]
struct Tally {
    extracted: u64,
    skipped_bytes: u64,
}

impl Tally {
    const fn record_extracted(&mut self) {
        self.extracted += 1;
    }

    const fn record_skipped(&mut self, bytes: u64) {
        self.skipped_bytes += bytes;
    }

    fn summary(&self, dir: &Path) -> String {
        let mut out = format!(
            "  extracted {} file(s) to {}",
            self.extracted,
            dir.display()
        );
        if self.skipped_bytes > 0 {
            let _ = writeln!(
                out,
                "  skipped {} bytes of unused entry bundles (closable/duo/popup)",
                self.skipped_bytes
            );
        }
        out
    }
}

/// Where an archive entry lands under the destination, or `None` to skip it.
///
/// Kept separate from the writing so the decision is testable without a zip.
fn destination_for(name: &str, strip_prefix: Option<&str>) -> Option<String> {
    if is_macos_junk(name) || is_unused_entry(name) {
        return None;
    }
    let relative = strip_prefix
        .and_then(|prefix| name.strip_prefix(prefix))
        .unwrap_or(name);
    let relative = relative.trim_start_matches('/');
    (!is_unsafe_entry_path(relative)).then(|| relative.to_owned())
}

/// Write every worthwhile entry of `archive` into `dest`.
///
/// # Errors
/// Returns the first filesystem or archive failure.
fn extract(
    archive: &mut ZipArchive<Cursor<Vec<u8>>>,
    dest: &Path,
) -> Result<Tally, Box<dyn std::error::Error>> {
    let names: Result<Vec<String>, _> = (0..archive.len())
        .map(|index| archive.by_index(index).map(|file| file.name().to_owned()))
        .collect();
    let strip_prefix = common_prefix(names?.iter().map(String::as_str));
    let mut tally = Tally::default();

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if is_macos_junk(file.name()) || is_unused_entry(file.name()) {
            if is_unused_entry(file.name()) {
                tally.record_skipped(file.size());
            }
            continue;
        }
        let Some(relative) = destination_for(file.name(), strip_prefix.as_deref()) else {
            continue;
        };

        let out_path = dest.join(&relative);
        if file.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        let Some(parent) = out_path.parent() else {
            continue;
        };
        fs::create_dir_all(parent)?;
        let mut out = BufWriter::new(fs::File::create(&out_path)?);
        io::copy(&mut file, &mut out)?;
        out.flush()?;
        tally.record_extracted();
    }
    Ok(tally)
}

/// Where to put Ketcher, and which version of it.
#[derive(Clone, Debug)]
pub struct KetcherTarget {
    /// Directory the editor is laid out in.
    pub dir: std::path::PathBuf,
    /// The version wanted, or `latest` to resolve one.
    pub requested: String,
    /// The archive URL, or `None` to derive it from the resolved version.
    pub url: Option<String>,
}

impl KetcherTarget {
    /// Read the settings this binary is documented to honour.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            dir: std::path::PathBuf::from(setting("KETCHER_DIR", DEFAULT_DIR)),
            requested: setting("KETCHER_VERSION", "latest"),
            url: std::env::var("KETCHER_URL").ok(),
        }
    }
}

/// Whether the editor already in `target.dir` is the one that was asked for.
///
/// The version is read out of the editor's own `index.html` rather than a
/// separate marker file, so there is nothing to keep in step with it.
fn already_present(target: &KetcherTarget, version: &str) -> Result<bool, std::io::Error> {
    let index_html = target.dir.join("index.html");
    if !index_html.is_file() {
        return Ok(false);
    }
    Ok(fs::read_to_string(&index_html)?.contains(&format!("Ketcher v{version}")))
}

/// Download Ketcher into `target.dir` unless that version is already there.
///
/// # Errors
/// Returns the first version-resolution, transport, archive or filesystem
/// failure. A stale tree is removed before the download, so a failure leaves no
/// editor behind rather than a half-updated one.
pub fn fetch_ketcher(
    client: &Client,
    target: &KetcherTarget,
) -> Result<(), Box<dyn std::error::Error>> {
    let version = resolve_tagged_version(client, KETCHER_LATEST_URL, &target.requested)?;
    if already_present(target, &version)? {
        println!(
            "✓ Ketcher v{version} already present in {}",
            target.dir.display()
        );
        return Ok(());
    }
    if target.dir.join("index.html").is_file() {
        fs::remove_dir_all(&target.dir)?;
        println!("Updating Ketcher to v{version}");
    }

    let url = target.url.clone().unwrap_or_else(|| release_url(&version));
    println!("Downloading Ketcher v{version} from {url} ...");
    let response = client.get(&url).send()?;
    if !response.status().is_success() {
        return Err(format!("HTTP {} fetching {url}", response.status()).into());
    }
    let bytes = response.bytes()?;
    println!("  downloaded {} bytes", bytes.len());

    fs::create_dir_all(&target.dir)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec()))?;
    let tally = extract(&mut archive, &target.dir)?;
    println!("{}", tally.summary(&target.dir));
    Ok(())
}
