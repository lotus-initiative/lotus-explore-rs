// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Fetch the external frontend assets used by the web client.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};

use reqwest::blocking::Client;
use serde_json::Value;
use zip::ZipArchive;

const DEFAULT_KETCHER_VERSION: &str = "latest";
const DEFAULT_RDKIT_VERSION: &str = "latest";
const DEFAULT_CITATION_JS_REF: &str = "main";
const KETCHER_OWNER_REPO: &str = "epam/ketcher";
const KETCHER_LATEST_URL: &str = "https://github.com/epam/ketcher/releases/latest";
const RDKIT_LATEST_METADATA: &str = "https://unpkg.com/@rdkit/rdkit@latest/?meta";
const RDKIT_LICENSE_URL: &str = "https://raw.githubusercontent.com/rdkit/rdkit/master/license.txt";
const SCHOLIA_REPO: &str = "WDscholia/scholia";
const SCHOLIA_RAW_URL: &str = "https://raw.githubusercontent.com/WDscholia/scholia";
const GITHUB_BASE_URL: &str = "https://github.com";
const CITATION_LICENSE_URL: &str =
    "https://raw.githubusercontent.com/citation-js/citation-js/main/LICENSE.md";
const DEFAULT_DIR: &str = "public/assets/ketcher";
const DEFAULT_CURATION_DIR: &str = "public/assets/vendor";
const DEFAULT_CURATION_STATE: &str = "target/lotus-assets-state";

fn setting(name: &str, default: &str) -> String {
    setting_or(env::var(name).ok(), default)
}

/// A blank value counts as unset.
///
/// Split from [`setting`] so it is reachable without mutating the process
/// environment, which is `unsafe` and global; the distinction it draws is the
/// one that matters, since an exported `KETCHER_VERSION=""` in CI would
/// otherwise be taken as a path of zero length.
fn setting_or(value: Option<String>, default: &str) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

/// Unused standalone "entry" bundles (and their license files) that ketcher's `index.html`
/// never references — only `main.<hash>.js` is loaded by the editor iframe.
#[must_use]
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

/// macOS zip metadata that must never be extracted: the `__MACOSX/` tree and `._`-prefixed
/// resource forks.
/// The ketcher release zip ships these (it was archived on macOS), and dioxus-cli's asset
#[must_use]
fn is_macos_junk(name: &str) -> bool {
    if name == "__MACOSX" || name.starts_with("__MACOSX/") {
        return true;
    }
    let Some(file_name) = name.rsplit('/').next() else {
        return false;
    };
    file_name.starts_with("._")
}

#[must_use]
fn release_url(version: &str) -> String {
    format!(
        "https://github.com/{KETCHER_OWNER_REPO}/releases/download/v{version}/ketcher-standalone-{version}.zip"
    )
}

fn normalize_version(version: &str) -> String {
    version.strip_prefix('v').unwrap_or(version).to_owned()
}

/// A response that is not 2xx is an error naming the URL.
///
/// Shared by every fetch here for the same reason: a 404 from a CDN is a
/// perfectly good body to write to disk, and the asset then looks present until
/// the deployed site 404s on it.
fn check_status(status: reqwest::StatusCode, url: &str) -> Result<(), Box<dyn std::error::Error>> {
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {status} fetching {url}").into())
    }
}

/// A full 40-character git object id.
///
/// Shared because it answers two different questions -- "did the feed give me a
/// commit id" and "was the caller pinning one" -- and the two had drifted into
/// separate copies of the same check.
fn is_commit_id(candidate: &str) -> bool {
    candidate.len() == 40 && candidate.chars().all(|c| c.is_ascii_hexdigit())
}

fn read_json(client: &Client, url: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let response = client.get(url).send()?;
    check_status(response.status(), url)?;
    Ok(serde_json::from_slice(&response.bytes()?)?)
}

fn fetch_file(
    client: &Client,
    url: &str,
    destination: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading {url} ...");
    let response = client.get(url).send()?;
    check_status(response.status(), url)?;
    let bytes = response.bytes()?;
    if bytes.is_empty() {
        return Err(format!("empty response fetching {url}").into());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(destination, bytes)?;
    Ok(())
}

fn resolve_ketcher_version(
    client: &Client,
    latest_url: &str,
    requested: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if requested != "latest" {
        return Ok(normalize_version(requested));
    }
    let response = client.get(latest_url).send()?;
    check_status(response.status(), latest_url)?;
    let Some(tag) = response
        .url()
        .path()
        .split_once("/tag/")
        // GitHub redirects to `/releases/tag/v2.3.4`, but a trailing slash
        // survives in the path and would be pasted into the download URL.
        .map(|(_, tag)| tag.trim_end_matches('/'))
    else {
        return Err(
            format!("Ketcher latest URL {latest_url} did not resolve to a release tag").into(),
        );
    };
    Ok(normalize_version(tag))
}

fn resolve_rdkit_version(
    client: &Client,
    latest_url: &str,
    requested: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if requested != "latest" {
        return Ok(requested.to_owned());
    }
    let metadata = read_json(client, latest_url)?;
    let Some(version) = metadata.get("version").and_then(Value::as_str) else {
        return Err("RDKit package metadata has no version".into());
    };
    Ok(version.to_owned())
}

fn github_commit(body: &str) -> Option<String> {
    let marker = "Grit::Commit/";
    let start = body.find(marker)? + marker.len();
    let commit = body[start..].chars().take(40).collect::<String>();
    is_commit_id(&commit).then_some(commit)
}

fn resolve_github_ref(
    client: &Client,
    base_url: &str,
    repository: &str,
    requested: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if is_commit_id(requested) {
        return Ok(requested.to_owned());
    }

    let url = format!("{base_url}/{repository}/commits/{requested}.atom");
    let response = client.get(&url).send()?;
    check_status(response.status(), &url)?;
    let bytes = response.bytes()?;
    let body = String::from_utf8_lossy(&bytes);
    github_commit(&body)
        .ok_or_else(|| format!("GitHub ref {repository}/{requested} has no commit id").into())
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

/// Reject an archive entry that would be written outside the target directory.
///
/// This is the zip-slip guard, and it is the reason the extraction does not join
/// a user-controlled name onto a path without checking it: `fetch-assets` runs
/// with the developer's privileges, so a `..` segment in a downloaded release
/// would otherwise write anywhere on the machine.
fn is_unsafe_entry_path(relative: &str) -> bool {
    relative.is_empty()
        || relative.starts_with('/')
        // Per segment rather than `contains("..")`, which also rejects the
        // perfectly ordinary `a..b.js` and silently drops it from the vendor.
        || relative.split('/').any(|segment| segment == "..")
}

/// One vendored family of third-party assets, checked, invalidated and logged
/// on its own.
#[derive(Debug)]
struct VendoredAsset {
    /// Human-readable name, for the log.
    name: &'static str,
    /// Key this asset owns in the state file.
    state_key: &'static str,
    /// Directory under the curation root that this asset owns, and the only one
    /// removed when this asset is stale.
    dir: &'static str,
    /// The resolved version or commit.
    version: String,
    /// `(url, path within `dir`)`. Licences are listed deliberately, so a
    /// missing one invalidates the asset rather than shipping unlicensed bytes.
    files: Vec<(String, String)>,
}

impl VendoredAsset {
    /// Every file this asset must have on disk to count as cached.
    fn paths(&self, root: &Path) -> Vec<PathBuf> {
        self.files
            .iter()
            .map(|(_, relative)| root.join(self.dir).join(relative))
            .collect()
    }
}

/// Whether one vendored asset can be reused as-is.
/// `recorded` is what the state file holds for this asset, if anything.
#[must_use]
fn asset_is_current(recorded: Option<&str>, version: &str, all_files_present: bool) -> bool {
    recorded == Some(version) && all_files_present
}

/// Parse the `key=value` state file. One entry per line; unparsable lines are
/// ignored rather than fatal, so a hand-edited file degrades to a re-fetch.
fn parse_state(contents: &str) -> BTreeMap<String, String> {
    contents
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            Some((key.to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// Render the state file, keys sorted so the output is stable.
fn render_state(state: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (key, value) in state {
        out.push_str(key);
        out.push('=');
        out.push_str(value);
        out.push('\n');
    }
    out
}

fn fetch_curation_assets(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(setting("CURATION_ASSET_DIR", DEFAULT_CURATION_DIR));
    let rdkit_version = resolve_rdkit_version(
        client,
        RDKIT_LATEST_METADATA,
        &setting("RDKIT_VERSION", DEFAULT_RDKIT_VERSION),
    )?;
    let citation_commit = resolve_github_ref(
        client,
        GITHUB_BASE_URL,
        SCHOLIA_REPO,
        &setting("CITATION_JS_REF", DEFAULT_CITATION_JS_REF),
    )?;
    let state_path = PathBuf::from(setting("CURATION_ASSET_STATE", DEFAULT_CURATION_STATE));
    let mut state =
        fs::read_to_string(&state_path).map_or_else(|_| BTreeMap::new(), |raw| parse_state(&raw));

    let rdkit_dist = format!("https://unpkg.com/@rdkit/rdkit@{rdkit_version}/dist");
    let scholia = format!("{SCHOLIA_RAW_URL}/{citation_commit}");
    let assets = [
        VendoredAsset {
            name: "RDKit",
            state_key: "rdkit",
            dir: "rdkit",
            version: rdkit_version,
            files: vec![
                (
                    format!("{rdkit_dist}/RDKit_minimal.js"),
                    "RDKit_minimal.js".to_owned(),
                ),
                (
                    format!("{rdkit_dist}/RDKit_minimal.wasm"),
                    "RDKit_minimal.wasm".to_owned(),
                ),
                (RDKIT_LICENSE_URL.to_owned(), "LICENSE.txt".to_owned()),
            ],
        },
        VendoredAsset {
            name: "Scholia Citation.js",
            state_key: "scholia",
            dir: "citation-js",
            version: citation_commit,
            files: vec![
                (
                    format!("{scholia}/scholia/app/static/js/citation.js"),
                    "citation.js".to_owned(),
                ),
                (
                    format!("{scholia}/LICENSE"),
                    "LICENSE.scholia.txt".to_owned(),
                ),
                (
                    CITATION_LICENSE_URL.to_owned(),
                    "LICENSE.citation-js.txt".to_owned(),
                ),
            ],
        },
    ];

    for asset in &assets {
        let all_present = asset.paths(&root).iter().all(|path| path.is_file());
        if asset_is_current(
            state.get(asset.state_key).map(String::as_str),
            &asset.version,
            all_present,
        ) {
            println!(
                "✓ {} {} already present ({} files in {}/{})",
                asset.name,
                asset.version,
                asset.files.len(),
                root.display(),
                asset.dir
            );
            continue;
        }

        // Only this asset's own directory goes, so a stale Citation.js never
        // costs a re-download of RDKit and vice versa.
        let dir = root.join(asset.dir);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        fs::create_dir_all(&dir)?;
        println!(
            "↓ {} {} is not cached — fetching {} files into {}/{}",
            asset.name,
            asset.version,
            asset.files.len(),
            root.display(),
            asset.dir
        );
        for (url, relative_path) in &asset.files {
            fetch_file(client, url, &dir.join(relative_path))?;
        }
        // Recorded only after every file landed, so an interrupted fetch is
        // retried next time instead of being trusted.
        state.insert(asset.state_key.to_owned(), asset.version.clone());
        if let Some(parent) = state_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&state_path, render_state(&state))?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder().build()?;
    fetch_curation_assets(&client)?;

    let version = resolve_ketcher_version(
        &client,
        KETCHER_LATEST_URL,
        &setting("KETCHER_VERSION", DEFAULT_KETCHER_VERSION),
    )?;
    let ketcher_dir = PathBuf::from(setting("KETCHER_DIR", DEFAULT_DIR));
    let index_html = ketcher_dir.join("index.html");

    if index_html.is_file() {
        let index = fs::read_to_string(&index_html)?;
        if index.contains(&format!("Ketcher v{version}")) {
            println!(
                "✓ Ketcher v{version} already present in {}",
                ketcher_dir.display()
            );
            return Ok(());
        }
        fs::remove_dir_all(&ketcher_dir)?;
        println!("Updating Ketcher to v{version}");
    }

    let url = env::var("KETCHER_URL").unwrap_or_else(|_| release_url(&version));
    println!("Downloading Ketcher v{version} from {url} ...");

    let response = client.get(&url).send()?;
    check_status(response.status(), &url)?;
    let bytes = response.bytes()?;
    let total = bytes.len();
    println!("  downloaded {total} bytes");

    fs::create_dir_all(&ketcher_dir)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec()))?;

    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).map(|f| f.name().to_string()))
        .collect::<Result<_, _>>()?;
    let strip_prefix = common_prefix(names.iter().map(String::as_str));

    let mut tally = Tally::default();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;

        if is_macos_junk(file.name()) {
            continue;
        }
        if is_unused_entry(file.name()) {
            tally.record_skipped(file.size());
            continue;
        }

        let rel = strip_prefix
            .as_deref()
            .and_then(|prefix| file.name().strip_prefix(prefix).map(str::to_string))
            .unwrap_or_else(|| file.name().to_string());
        let rel = rel.trim_start_matches('/');
        if is_unsafe_entry_path(rel) {
            continue;
        }

        let out_path = ketcher_dir.join(rel);
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

    println!("{}", tally.summary(&ketcher_dir));
    Ok(())
}

#[cfg(test)]
mod tests {
    // The panic lints keep library code from panicking on bad input. A test that
    // fails on a bad fixture is reporting, not panicking.
    #![allow(
        clippy::panic,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing
    )]

    use super::*;
    use std::io::Read as _;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    // ── A local HTTP server, because the fetch paths decide what ships ────────

    /// Answers one canned response per connection, in order, and records the
    /// request line of each. Enough HTTP/1.1 to exercise the status handling
    /// with no network and no dependency.
    ///
    /// `{SELF}` in a response is replaced with this server's own address, which
    /// is how a redirect test points a client back at the same server.
    struct MockServer {
        base: String,
        seen: std::sync::mpsc::Receiver<String>,
    }

    impl MockServer {
        fn start(responses: Vec<String>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0")
                .unwrap_or_else(|e| panic!("cannot bind a loopback port: {e}"));
            let port = listener.local_addr().map(|a| a.port()).unwrap_or_default();
            let base = format!("http://127.0.0.1:{port}");
            let (tx, seen) = std::sync::mpsc::channel();
            let self_url = base.clone();
            std::thread::spawn(move || {
                for response in responses {
                    let Ok((mut stream, _)) = listener.accept() else {
                        break;
                    };
                    let mut buf = [0u8; 2048];
                    let read = stream.read(&mut buf).unwrap_or(0);
                    let line = String::from_utf8_lossy(&buf[..read])
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_owned();
                    let _ = tx.send(line);
                    let _ = stream.write_all(response.replace("{SELF}", &self_url).as_bytes());
                    let _ = stream.flush();
                }
            });
            Self { base, seen }
        }

        fn url(&self, path: &str) -> String {
            format!("{}{path}", self.base)
        }

        /// The request line of the next request served, so a test can assert a
        /// pinned version really did short-circuit the network.
        fn next_request(&self) -> String {
            self.seen
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|e| panic!("no request was served: {e}"))
        }
    }

    fn http_ok(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn http_status(code: u16, reason: &str) -> String {
        format!("HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
    }

    fn http_redirect_to_self(location: &str) -> String {
        format!(
            "HTTP/1.1 302 Found\r\nLocation: {{SELF}}{location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
    }

    fn client(timeout_ms: u64) -> Client {
        Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .unwrap_or_else(|e| panic!("cannot build a client: {e}"))
    }

    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("lotus-fetch-{tag}-{}-{unique}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path)
            .unwrap_or_else(|e| panic!("cannot create {}: {e}", path.display()));
        path
    }

    /// The message of a `Result`'s error, for asserting on it whatever the `Ok`
    /// type happens to be.
    fn err<T>(result: Result<T, Box<dyn std::error::Error>>) -> String {
        match result {
            Ok(_) => panic!("expected an error, got Ok"),
            Err(error) => error.to_string(),
        }
    }

    /// 40 hex characters, the length every one of these tests needs.
    fn commit_id(seed: char) -> String {
        std::iter::repeat_n(seed, 40).collect()
    }

    // ── Settings ─────────────────────────────────────────────────────────────

    #[test]
    fn an_unset_setting_is_its_default() {
        // `setting_or` covers the filtering; this covers the lookup itself, which
        // is the half that would silently return an empty value.
        assert_eq!(
            setting("LOTUS_FETCH_ASSETS_NOT_SET_9f3a2b", "fallback"),
            "fallback"
        );
    }

    #[test]
    fn a_blank_setting_counts_as_unset() {
        assert_eq!(setting_or(None, "fallback"), "fallback");
        for blank in ["", " ", "\t", "  \n ", "\r\n"] {
            assert_eq!(
                setting_or(Some(blank.to_owned()), "fallback"),
                "fallback",
                "{blank:?} is not a value, it is an empty export"
            );
        }
        assert_eq!(setting_or(Some("real".to_owned()), "fallback"), "real");
        assert_eq!(
            setting_or(Some(" padded ".to_owned()), "fallback"),
            " padded "
        );
    }

    // ── Version resolution ───────────────────────────────────────────────────

    #[test]
    fn a_pinned_ketcher_version_never_asks_the_network() {
        // The server has no responses queued, so any request at all would hang
        // and then fail; the assertion is that it succeeds without one.
        {
            let requested = "v2.3.4";
            let server = MockServer::start(vec![]);
            assert_eq!(
                resolve_ketcher_version(&client(300), &server.url("/latest"), requested)
                    .unwrap_or_default(),
                "2.3.4",
                "the leading v is stripped so versions compare equal"
            );
        }
    }

    #[test]
    fn the_word_latest_resolves_through_the_redirect() {
        {
            let requested = "latest";
            // The tag is read off the URL the redirect *resolved to*, so the mock
            // redirects to itself and the follow-up is what carries `/tag/`.
            let server =
                MockServer::start(vec![http_redirect_to_self("/tag/v9.9.9/"), http_ok("")]);
            let resolved =
                resolve_ketcher_version(&client(3000), &server.url("/releases/latest"), requested);
            assert!(
                matches!(resolved, Ok(ref v) if v == "9.9.9"),
                "the tag comes off the resolved URL, got {resolved:?}"
            );
            assert!(
                server.next_request().contains("/releases/latest"),
                "the latest URL is what was asked for"
            );
        }
    }

    #[test]
    fn a_latest_lookup_that_404s_is_an_error() {
        {
            let requested = "latest";
            let server = MockServer::start(vec![http_status(404, "Not Found")]);
            let message = err(resolve_ketcher_version(
                &client(3000),
                &server.url("/releases/latest"),
                requested,
            ));
            assert!(message.contains("404"), "a 404 names itself, got {message}");
        }
    }

    #[test]
    fn a_latest_lookup_with_no_tag_in_the_url_is_an_error() {
        {
            let requested = "latest";
            // Redirects to the repository root, not to a release: the version
            // cannot be read, and guessing one would vendor the wrong bundle.
            let server = MockServer::start(vec![
                http_redirect_to_self("/WDscholia/ketcher"),
                http_ok(""),
            ]);
            let message = err(resolve_ketcher_version(
                &client(3000),
                &server.url("/releases/latest"),
                requested,
            ));
            assert!(
                message.contains("did not resolve to a release tag"),
                "got {message}"
            );
        }
    }

    #[test]
    fn a_pinned_rdkit_version_never_asks_the_network() {
        {
            let requested = "2026.3.6";
            let server = MockServer::start(vec![]);
            assert_eq!(
                resolve_rdkit_version(&client(300), &server.url("/meta"), requested)
                    .unwrap_or_default(),
                "2026.3.6"
            );
        }
    }

    #[test]
    fn the_latest_rdkit_version_comes_from_the_package_metadata() {
        {
            let requested = "latest";
            let server = MockServer::start(vec![http_ok(r#"{"version":"2026.9.9"}"#)]);
            assert_eq!(
                resolve_rdkit_version(&client(3000), &server.url("/meta"), requested)
                    .unwrap_or_default(),
                "2026.9.9"
            );
            let _ = server.next_request();
        }
    }

    #[test]
    fn rdkit_metadata_with_no_version_is_an_error() {
        {
            let requested = "latest";
            let server = MockServer::start(vec![http_ok(r#"{"name":"@rdkit/rdkit"}"#)]);
            let message = err(resolve_rdkit_version(
                &client(3000),
                &server.url("/meta"),
                requested,
            ));
            assert!(message.contains("no version"), "got {message}");
        }
    }

    // ── Commit ids ───────────────────────────────────────────────────────────

    #[test]
    fn a_commit_id_is_forty_hex_characters_and_nothing_else() {
        assert!(is_commit_id(&commit_id('a')));
        assert!(is_commit_id(&commit_id('F')));
        assert!(
            !is_commit_id(&commit_id('a')[..39]),
            "one short is a branch"
        );
        assert!(
            !is_commit_id(&format!("{}a", commit_id('a'))),
            "one long is not an id"
        );
        assert!(
            !is_commit_id(&commit_id('z')),
            "hexadecimal only: a ref with a non-hex character is not a commit"
        );
        assert!(!is_commit_id("main"));
        assert!(!is_commit_id(""));
    }

    #[test]
    fn a_commit_is_read_out_of_the_feed() {
        let body = format!("<entry>Grit::Commit/{}\n</entry>", commit_id('0'));
        assert_eq!(github_commit(&body).unwrap_or_default(), commit_id('0'));
    }

    #[test]
    fn a_feed_with_no_commit_id_yields_nothing() {
        assert!(github_commit("<entry>nothing here</entry>").is_none());
        assert!(github_commit(&format!("Grit::Commit/{}", &commit_id('a')[..39])).is_none());
        assert!(
            github_commit(&format!("Grit::Commit/{}", commit_id('z'))).is_none(),
            "a non-hex id is not a commit"
        );
    }

    #[test]
    fn a_pinned_ref_is_taken_as_given() {
        let server = MockServer::start(vec![]);
        let pinned = commit_id('0');
        assert_eq!(
            resolve_github_ref(&client(300), &server.url(""), "WDscholia/scholia", &pinned)
                .unwrap_or_default(),
            pinned,
            "a full commit id needs no feed lookup"
        );
    }

    #[test]
    fn a_branch_ref_resolves_through_the_feed() {
        let server = MockServer::start(vec![http_ok(&format!(
            "<entry>Grit::Commit/{}\n</entry>",
            commit_id('0')
        ))]);
        assert_eq!(
            resolve_github_ref(&client(3000), &server.url(""), "WDscholia/scholia", "main")
                .unwrap_or_default(),
            commit_id('0')
        );
        let request = server.next_request();
        assert!(
            request.contains("/WDscholia/scholia/commits/main.atom"),
            "got {request}"
        );
    }

    #[test]
    fn a_ref_the_feed_does_not_resolve_is_an_error() {
        let server = MockServer::start(vec![http_ok("<entry>no id here</entry>")]);
        let message = err(resolve_github_ref(
            &client(3000),
            &server.url(""),
            "WDscholia/scholia",
            "gone",
        ));
        assert!(message.contains("no commit id"), "got {message}");
    }

    #[test]
    fn a_ref_lookup_that_404s_is_an_error() {
        let server = MockServer::start(vec![http_status(404, "Not Found")]);
        let message = err(resolve_github_ref(
            &client(3000),
            &server.url(""),
            "WDscholia/scholia",
            "gone",
        ));
        assert!(message.contains("404"), "got {message}");
    }

    // ── Reading and writing bytes ────────────────────────────────────────────

    #[test]
    fn json_is_parsed_and_a_500_is_not() {
        let server = MockServer::start(vec![
            http_ok(r#"{"version":"1"}"#),
            http_status(500, "Server Error"),
        ]);
        let c = client(3000);
        assert_eq!(
            read_json(&c, &server.url("/a")).unwrap_or_default()["version"],
            Value::from("1")
        );
        let message = err(read_json(&c, &server.url("/b")));
        assert!(message.contains("500"), "got {message}");
    }

    #[test]
    fn a_file_is_written_and_an_error_page_is_not() {
        let dir = temp_dir("fetch");
        let server = MockServer::start(vec![
            http_ok("contents"),
            http_status(403, "Forbidden"),
            http_ok(""),
        ]);
        let c = client(3000);

        let good = dir.join("nested/good.txt");
        fetch_file(&c, &server.url("/a"), &good).unwrap_or_default();
        assert_eq!(fs::read_to_string(&good).unwrap_or_default(), "contents");
        assert!(
            good.parent().is_some_and(Path::exists),
            "the parent is created"
        );

        let denied = dir.join("denied.txt");
        let message = err(fetch_file(&c, &server.url("/b"), &denied));
        assert!(message.contains("403"), "got {message}");
        assert!(
            !denied.exists(),
            "an error page must not be written as the asset"
        );

        let empty = dir.join("empty.txt");
        let message = err(fetch_file(&c, &server.url("/c"), &empty));
        assert!(message.contains("empty response"), "got {message}");
        assert!(!empty.exists(), "an empty body is not an asset");

        let _ = fs::remove_dir_all(&dir);
    }

    // ── Laying an archive out on disk ────────────────────────────────────────

    #[test]
    fn macos_junk_does_not_hide_the_single_wrapper_directory() {
        // `__MACOSX` is a top-level entry. Counting it makes the archive look
        // like it has two top levels, so nothing is stripped and the editor ends
        // up at `standalone/index.html` instead of `index.html`.
        let names = [
            "__MACOSX/._standalone",
            "standalone/index.html",
            "standalone/static/js/main.js",
        ];
        assert_eq!(
            common_prefix(names.into_iter()),
            Some("standalone".to_owned())
        );
    }

    #[test]
    fn the_summary_reports_what_was_written_and_what_was_left_out() {
        let mut tally = Tally::default();
        assert_eq!(
            tally.summary(Path::new("public/assets/ketcher")),
            "  extracted 0 file(s) to public/assets/ketcher",
            "nothing skipped means nothing said about skipping"
        );

        tally.record_extracted();
        tally.record_extracted();
        assert!(
            tally
                .summary(Path::new("out"))
                .contains("extracted 2 file(s)")
        );

        tally.record_skipped(4096);
        let summary = tally.summary(Path::new("out"));
        assert!(summary.contains("skipped 4096 bytes"), "got {summary}");
        assert!(
            summary.contains("closable/duo/popup"),
            "names what was skipped: {summary}"
        );

        tally.record_skipped(1);
        assert!(
            tally
                .summary(Path::new("out"))
                .contains("skipped 4097 bytes"),
            "the bytes accumulate rather than being overwritten"
        );
    }

    #[test]
    fn one_top_level_directory_is_stripped_and_several_are_not() {
        let nested = ["standalone/index.html", "standalone/static/js/main.js"];
        assert_eq!(
            common_prefix(nested.into_iter()),
            Some("standalone".to_owned()),
            "a single wrapper directory is stripped so the editor sits at the root"
        );
        // Two top levels: no guess, because the wrong guess writes files to the
        // wrong place and the failure only shows up in the browser.
        assert_eq!(common_prefix(["a/x.js", "b/y.js"].into_iter()), None);
        // Empty and root-only names carry no directory.
        assert_eq!(common_prefix(["/x.js"].into_iter()), None);
        assert_eq!(common_prefix(["/x.js", "/y.js"].into_iter()), None);
        assert_eq!(common_prefix(std::iter::empty()), None);
    }

    #[test]
    fn an_archive_entry_cannot_escape_the_target_directory() {
        // The zip-slip cases. Each of these writes outside `ketcher_dir` if it is
        // joined onto the path unchecked.
        for unsafe_path in [
            "..",
            "../outside.js",
            "static/../../outside.js",
            "/etc/passwd",
            "",
        ] {
            assert!(
                is_unsafe_entry_path(unsafe_path),
                "{unsafe_path:?} must not be written"
            );
        }
        for safe in ["index.html", "static/js/main.js", "a..b.js", "./x.js"] {
            assert!(
                !is_unsafe_entry_path(safe),
                "{safe:?} is inside the directory and must be written"
            );
        }
    }

    // ── The paths an asset owns ──────────────────────────────────────────────

    #[test]
    fn an_asset_owns_its_licence_as_well_as_its_payload() {
        let asset = VendoredAsset {
            name: "RDKit",
            state_key: "rdkit",
            dir: "rdkit",
            version: "2026.3.6".to_owned(),
            files: vec![
                (
                    "https://example.invalid/a.js".to_owned(),
                    "RDKit_minimal.js".to_owned(),
                ),
                (
                    "https://example.invalid/licence".to_owned(),
                    "LICENSE".to_owned(),
                ),
            ],
        };
        assert_eq!(
            asset.paths(Path::new("public/assets/vendor")),
            vec![
                PathBuf::from("public/assets/vendor/rdkit/RDKit_minimal.js"),
                PathBuf::from("public/assets/vendor/rdkit/LICENSE"),
            ],
            "a missing licence has to invalidate the asset, so it is one of the paths"
        );
    }

    #[test]
    fn an_asset_with_no_files_owns_nothing() {
        let asset = VendoredAsset {
            name: "empty",
            state_key: "empty",
            dir: "empty",
            version: "0".to_owned(),
            files: vec![],
        };
        assert!(asset.paths(Path::new("root")).is_empty());
    }

    #[test]
    fn classifies_unused_entries() {
        assert!(is_unused_entry("standalone/static/js/closable.5cead650.js"));
        assert!(is_unused_entry("standalone/static/js/duo.546fbaab.js"));
        assert!(is_unused_entry("standalone/static/js/popup.ec23766a.js"));
        assert!(is_unused_entry(
            "standalone/static/js/closable.5cead650.js.LICENSE.txt"
        ));
        assert!(is_unused_entry(
            "standalone/static/js/duo.546fbaab.js.LICENSE.txt"
        ));
        assert!(!is_unused_entry("standalone/static/js/main.cb80d824.js"));
        assert!(!is_unused_entry(
            "standalone/static/js/157.7de4e426.chunk.js"
        ));
        assert!(!is_unused_entry(
            "standalone/static/js/622.ed91ac0.chunk.js.LICENSE.txt"
        ));
        assert!(!is_unused_entry("standalone/index.html"));
        assert!(!is_unused_entry("standalone/static/css/main.9cca8bc6.css"));
        assert!(!is_unused_entry("standalone/duo.html"));
        assert!(!is_unused_entry(
            "standalone/static/css/closable.9cca8bc6.css"
        ));
        assert!(!is_unused_entry("standalone/._duo.546fbaab.js"));
    }

    #[test]
    fn state_round_trips_both_assets() {
        let raw = "rdkit=2026.3.6\nscholia=1626b5e3\n";
        let parsed = parse_state(raw);
        assert_eq!(parsed.get("rdkit").map(String::as_str), Some("2026.3.6"));
        assert_eq!(parsed.get("scholia").map(String::as_str), Some("1626b5e3"));
        assert_eq!(
            render_state(&parsed),
            raw,
            "format is unchanged, so no re-fetch"
        );
    }

    #[test]
    fn state_ignores_junk_lines_instead_of_failing() {
        let parsed = parse_state("rdkit=1\nnot a pair\n=novalue\n\nscholia=2\n");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed.get("rdkit").map(String::as_str), Some("1"));
        assert_eq!(parsed.get("scholia").map(String::as_str), Some("2"));
    }

    #[test]
    fn a_version_match_alone_is_not_enough() {
        // A partial download leaves the recorded version in place but files
        // missing, so both halves of the check have to hold.
        assert!(!asset_is_current(Some("1.0"), "1.0", false));
        assert!(asset_is_current(Some("1.0"), "1.0", true));
    }

    #[test]
    fn one_stale_asset_leaves_the_other_current() {
        let mut state = parse_state("rdkit=1\nscholia=1\n");
        // Scholia moves; RDKit does not.
        state.insert("scholia".to_owned(), "2".to_owned());
        let rdkit = asset_is_current(state.get("rdkit").map(String::as_str), "1", true);
        let scholia = asset_is_current(state.get("scholia").map(String::as_str), "1", true);
        assert!(rdkit, "RDKit must stay cached when only Scholia changed");
        assert!(!scholia, "the moved Scholia ref must be re-fetched");
        assert_eq!(
            render_state(&state),
            "rdkit=1\nscholia=2\n",
            "only the moved key is rewritten"
        );
    }

    #[test]
    fn an_unrecorded_asset_is_always_fetched() {
        assert!(!asset_is_current(None, "1.0", true));
        assert!(!asset_is_current(None, "1.0", false));
    }

    #[test]
    fn classifies_macos_junk() {
        assert!(is_macos_junk("__MACOSX"));
        assert!(is_macos_junk("__MACOSX/standalone/._index.html"));
        assert!(is_macos_junk(
            "__MACOSX/standalone/static/js/._duo.546fbaab.js"
        ));
        assert!(is_macos_junk(
            "__MACOSX/standalone/static/js/._asset-manifest.json"
        ));
        assert!(!is_macos_junk("standalone/index.html"));
        assert!(!is_macos_junk("standalone/static/js/main.cb80d824.js"));
        assert!(!is_macos_junk("standalone/static/js/duo.546fbaab.js"));
    }

    #[test]
    fn release_url_points_at_github_releases() {
        assert_eq!(
            release_url("3.18.0"),
            "https://github.com/epam/ketcher/releases/download/v3.18.0/ketcher-standalone-3.18.0.zip"
        );
        assert_eq!(
            release_url("3.10.0"),
            "https://github.com/epam/ketcher/releases/download/v3.10.0/ketcher-standalone-3.10.0.zip"
        );
    }

    #[test]
    fn normalizes_version_prefixes() {
        assert_eq!(normalize_version("v3.18.0"), "3.18.0");
        assert_eq!(normalize_version("3.18.0"), "3.18.0");
    }

    #[test]
    fn extracts_github_commit_from_atom_feed() {
        let body =
            "<id>tag:github.com,2008:Grit::Commit/0123456789abcdef0123456789abcdef01234567</id>";
        assert_eq!(
            github_commit(body).as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
        assert!(github_commit("<id>not-a-commit</id>").is_none());
    }
}
