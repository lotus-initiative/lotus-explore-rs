// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Preload the WebAssembly module in the built `index.html`.

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

    let out =
        insert_after_anchor(&html, &link).map_err(|e| format!("{e} in {}", index.display()))?;

    fs::write(&index, out)?;
    println!("✓ preloading {prefix}{module} in {}", index.display());
    Ok(())
}

/// The exact module the shipped glue will fetch.
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

/// Put `link` on its own line directly after the anchor, so it lands in `<head>`
/// next to the script it preloads rather than at the end of the document.
///
/// Split out of `main` because the offset arithmetic is where this goes wrong:
/// inserted at the anchor rather than after it, a `<link>` lands in the middle of
/// the tag it is meant to precede and the preload is silently dropped.
fn insert_after_anchor(html: &str, link: &str) -> Result<String, String> {
    let at = [GLUE_PRELOAD, MODULE_SCRIPT]
        .into_iter()
        .find_map(|needle| html.find(needle))
        .ok_or("no <link rel=\"preload\" as=\"script\"> or <script type=\"module\">")?;
    // The newline is included, so the link starts a line rather than continuing
    // the anchor's. A document with no newline after the anchor gets the link
    // appended, which is still after it.
    let end = html[at..]
        .find('\n')
        .map_or(html.len(), |offset| at + offset + 1);
    let mut out = String::with_capacity(html.len() + link.len() + 2);
    out.push_str(&html[..end]);
    // The offset above already consumed the anchor's newline, so this only fires
    // for a document that ends without one -- where the link would otherwise be
    // appended onto the anchor's own line.
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(link);
    out.push('\n');
    out.push_str(&html[end..]);
    Ok(out)
}

/// The JS glue file `index.html` actually loads, which is what names the module.
fn module_script_name(html: &str) -> Option<String> {
    let at = html.find(MODULE_SCRIPT)?;
    let rest = &html[at..];
    // Bounded by the end of the opening tag. Searching the rest of the document
    // instead means a module script with its source inlined -- which is what dx
    // emits for some bundles -- reports the `src` of whatever tag comes next, and
    // that file then gets the wasm preload.
    let tag_end = rest.find('>')?;
    let tag = &rest[..tag_end];
    let src_at = tag.find("src=\"")? + "src=\"".len();
    let src = &tag[src_at..];
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
#[path = "inject_wasm_preload/tests.rs"]
mod tests;
