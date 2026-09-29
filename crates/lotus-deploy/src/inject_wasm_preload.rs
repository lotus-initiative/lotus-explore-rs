// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Preload the WebAssembly module in the built `index.html`.
//!
//! `dx` already emits `<link rel="preload" as="script">` for the JS glue, but
//! nothing preloads the module itself, and the glue cannot start the module
//! fetch until it has been downloaded, parsed and executed:
//!
//! ```text
//! html -> glue js (45 KiB) -> fetch(wasm) -> instantiateStreaming -> mount
//! ```
//!
//! That serialises the largest response on the page behind a response that is
//! two orders of magnitude smaller. Measured with Lighthouse's `devtools`
//! throttling on a simulated 1.6 Mbit/s / 150 ms link, the module request used
//! to start 520252 ms after the document and now starts 220055 ms after it —
//! in the same ~9 ms window as the CSS and the glue, instead of 300 s behind
//! them. Observed LCP 3114 ms -> 2940 ms; simulated LCP 3753 ms -> 3630 ms.
//!
//! The module name is content-hashed, so the link cannot be written by hand in
//! the source `index.html`; it has to be injected after the bundle is emitted.
//! The asset prefix is copied from the preload `dx` already emitted, so a
//! `--base-path` build keeps working without this tool knowing about base paths.
//!
//! Run after `dx build`, from the repository root or anywhere:
//!
//! ```text
//! cargo run --release -p lotus-deploy --bin inject-wasm-preload -- <web-public-dir>
//! ```
//!
//! The directory defaults to the standard `dx` output path. Running it twice is
//! a no-op.

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

// This crate has one dependency set shared by all of its bins. `fetch-assets`
// needs the HTTP and archive crates; this one deliberately touches neither, and
// `unused_crate_dependencies` is `deny` workspace-wide. Naming them as `_` is
// the supported way to say "linked, not used here" — an `#[allow]` would not be.
use reqwest as _;
use serde_json as _;
use zip as _;

const DEFAULT_WEB_PUBLIC_DIR: &str = "target/dx/lotus-explore-rs/release/web/public";

/// Anchor for the injection: the preload `dx` emits for the JS glue.
const GLUE_PRELOAD: &str = "rel=\"preload\" as=\"script\"";
/// Fallback anchor, for a bundle built without dx's own preload.
const MODULE_SCRIPT: &str = "<script type=\"module\"";
/// Marker for a link this tool already added.
const WASM_PRELOAD_MARKER: &str = "as=\"fetch\"";

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let dir: PathBuf = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_WEB_PUBLIC_DIR), PathBuf::from);
    let index = dir.join("index.html");
    let html = fs::read_to_string(&index)?;

    if html.contains(WASM_PRELOAD_MARKER) {
        println!("✓ wasm preload already present in {}", index.display());
        return Ok(());
    }

    let glue = module_script_name(&html).ok_or_else(|| {
        format!(
            "no <script type=\"module\" src=...> in {} — cannot tell which module it loads",
            index.display()
        )
    })?;
    let module = find_wasm_module(&dir, &glue).map_err(std::io::Error::other)?;

    // Reuse the prefix dx already wrote (it carries the --base-path) instead of
    // reconstructing it, so this stays correct for both `/` and `/<repo>`.
    let prefix = asset_prefix(&html).ok_or_else(|| {
        format!(
            "no <link rel=\"preload\" as=\"script\"> in {} to take the asset prefix from — \
             refusing to guess one, because a wrong prefix is a 404 on the module",
            index.display()
        )
    })?;
    let link = format!(
        "  <link rel=\"preload\" as=\"fetch\" type=\"application/wasm\" href=\"{prefix}{module}\" crossorigin>"
    );

    let at = [GLUE_PRELOAD, MODULE_SCRIPT]
        .into_iter()
        .find_map(|needle| html.find(needle))
        .ok_or_else(|| {
            format!(
                "no <link rel=\"preload\" as=\"script\"> or <script type=\"module\"> in {}",
                index.display()
            )
        })?;
    // Insert on the line after the anchor so the link lands in <head>.
    let end = html[at..]
        .find('\n')
        .map_or(html.len(), |offset| at + offset + 1);
    let mut out = String::with_capacity(html.len() + link.len() + 1);
    out.push_str(&html[..end]);
    out.push_str(&link);
    out.push('\n');
    out.push_str(&html[end..]);

    fs::write(&index, out)?;
    println!("✓ preloading {prefix}{module} in {}", index.display());
    Ok(())
}

/// The exact module the shipped glue will fetch.
///
/// The bundle is content-hashed, and `dx` leaves a superseded module behind
/// whenever a rebuild changes the hash, so `assets/` can hold more than one
/// `.wasm`. Picking by modification time gets this wrong — it once preloaded a
/// stale module, and Chrome reported "preloaded using link but not used",
/// which costs a wasted 1.4 MiB fetch and is worse than not preloading at all.
///
/// The glue is the ground truth: it contains the hashed module name verbatim.
/// Combine that with "and it is actually on disk", and refuse to guess when the
/// two disagree.
fn find_wasm_module(dir: &Path, glue_name: &str) -> Result<String, String> {
    let assets = dir.join("assets");
    let glue = fs::read_to_string(assets.join(glue_name)).map_err(|err| {
        format!(
            "cannot read the JS glue {}: {err}",
            assets.join(glue_name).display()
        )
    })?;

    let mut named: Vec<String> = glue
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'))
        .filter(|token| {
            Path::new(token)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("wasm"))
        })
        // The glue also carries the pre-rewrite `…_bg.wasm` fallback, which is
        // never what is actually fetched; keep the hashed one.
        .filter(|token| {
            token.rsplit_once(".wasm").is_some_and(|(stem, _)| {
                stem.rsplit_once('-').is_some_and(|(_, hash)| {
                    !hash.is_empty() && hash.bytes().all(|b| b.is_ascii_alphanumeric())
                })
            })
        })
        .map(str::to_owned)
        .filter(|name| assets.join(name).is_file())
        .collect();
    named.sort_unstable();
    named.dedup();

    match named.len() {
        1 => Ok(named.swap_remove(0)),
        0 => Err(format!(
            "the glue names no .wasm that exists in {}/assets — is the bundle built?",
            dir.display()
        )),
        _ => Err(format!(
            "the glue names several modules present in {}/assets ({}); refusing to guess",
            dir.display(),
            named.join(", ")
        )),
    }
}

/// The JS glue file `index.html` actually loads, which is what names the module.
fn module_script_name(html: &str) -> Option<String> {
    let at = html.find(MODULE_SCRIPT)?;
    let rest = &html[at..];
    let src_at = rest.find("src=\"")? + "src=\"".len();
    let src = &rest[src_at..];
    let end = src.find('"')?;
    let src = &src[..end];
    src.rsplit('/').next().map(str::to_owned)
}

/// The `…/assets/` prefix dx wrote, so the injected link honours `--base-path`.
fn asset_prefix(html: &str) -> Option<String> {
    let at = html.find(GLUE_PRELOAD)?;
    let rest = &html[at..];
    let href_at = rest.find("href=\"")? + "href=\"".len();
    let href = &rest[href_at..];
    let end = href.find('"')?;
    let href = &href[..end];
    // Everything up to and including the slash before the file name. dx emits
    // `/./assets/…`, so keeping the slash here is what carries the base path.
    href.rfind('/').map(|slash| href[..=slash].to_owned())
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_WEB_PUBLIC_DIR, asset_prefix};

    const GLUE: &str = concat!(
        r#"  <link rel="preload" as="script" href="/./assets/lotus-explore-rs-dxh1e.js" crossorigin>"#,
        "\n",
        r#"  <script type="module" async src="/./assets/lotus-explore-rs-dxh1e.js"></script>"#,
        "\n",
    );

    #[test]
    fn default_dir_is_the_dx_output_path() {
        assert!(DEFAULT_WEB_PUBLIC_DIR.ends_with("web/public"));
    }

    #[test]
    fn asset_prefix_comes_from_the_preload_dx_emitted() {
        assert_eq!(asset_prefix(GLUE).as_deref(), Some("/./assets/"));
    }

    #[test]
    fn asset_prefix_keeps_a_base_path() {
        let html = GLUE.replace("/./assets/", "/lotus-explore-rs/./assets/");
        assert_eq!(
            asset_prefix(&html).as_deref(),
            Some("/lotus-explore-rs/./assets/")
        );
    }

    #[test]
    fn asset_prefix_is_none_without_a_preload() {
        assert!(asset_prefix("  <script type=\"module\" src=\"/a.js\"></script>").is_none());
    }
}
