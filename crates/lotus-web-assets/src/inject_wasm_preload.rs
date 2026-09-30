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
mod tests {
    // The panic lints keep library code from panicking on bad input. A test that
    // fails on a bad fixture is reporting a failure, not panicking on input.
    #![allow(
        clippy::panic,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing
    )]

    use super::{
        DEFAULT_WEB_PUBLIC_DIR, asset_prefix, find_wasm_module, insert_after_anchor,
        module_script_name,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A throwaway directory, removed on drop so a failing test does not leave
    /// one behind in the temp dir.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "lotus-web-assets-{tag}-{}-{unique}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("assets"))
                .unwrap_or_else(|e| panic!("cannot create {}: {e}", path.display()));
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        /// Write a file under `assets/`, as dx would leave it.
        fn asset(&self, name: &str, contents: &str) {
            fs::write(self.0.join("assets").join(name), contents)
                .unwrap_or_else(|e| panic!("cannot write {name}: {e}"));
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A bundle where the glue names exactly one hashed module that exists.
    fn bundle_naming(glue: &str) -> TempDir {
        let dir = TempDir::new("one");
        dir.asset("index-abc123.js", glue);
        dir
    }

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

    // ── The module script index.html actually loads ───────────────────────────

    #[test]
    fn the_module_name_comes_from_the_module_script() {
        assert_eq!(
            module_script_name(GLUE).as_deref(),
            Some("lotus-explore-rs-dxh1e.js"),
            "the base path is stripped: the name is resolved against assets/"
        );
    }

    #[test]
    fn the_module_name_is_none_without_a_module_script() {
        assert!(module_script_name("  <script src=\"/a.js\"></script>").is_none());
    }

    #[test]
    fn a_module_script_with_no_src_has_no_module_name() {
        // The search for `src=` is not bounded by the tag, so a module script
        // with its source inlined followed by any other tag with a `src` used to
        // report *that* file as the wasm module. It would then preload the wrong
        // bytes, or fail to find a wasm that was there all along.
        let html = concat!(
            "  <script type=\"module\">",
            "    import { init } from \"./index-abc123.js\";",
            "    init();",
            "  </script>\n",
            "  <script src=\"/analytics.js\"></script>\n",
        );
        assert_eq!(
            module_script_name(html),
            None,
            "an inlined module script loads no external file to name"
        );
    }

    // ── Finding the one wasm the glue will fetch ──────────────────────────────

    #[test]
    fn the_single_hashed_module_is_found() {
        let dir = bundle_naming("export default \"lotus-explore-rs-d1g2h3i4.wasm\";");
        dir.asset("lotus-explore-rs-d1g2h3i4.wasm", "\0asm");
        assert_eq!(
            find_wasm_module(dir.path(), "index-abc123.js"),
            Ok("lotus-explore-rs-d1g2h3i4.wasm".to_owned())
        );
    }

    #[test]
    fn the_pre_rewrite_fallback_is_not_mistaken_for_the_module() {
        // The glue carries both the module it fetches and the `_bg.wasm` name
        // left over from before the rewrite. `_bg` has an underscore, not a
        // hash, so the `-`-split filter rejects it.
        let dir = bundle_naming(
            "fetch(\"lotus-explore-rs-d1g2h3i4_bg.wasm\"); fetch(\"lotus-explore-rs-d1g2h3i4.wasm\");",
        );
        dir.asset("lotus-explore-rs-d1g2h3i4_bg.wasm", "\0asm");
        dir.asset("lotus-explore-rs-d1g2h3i4.wasm", "\0asm");
        assert_eq!(
            find_wasm_module(dir.path(), "index-abc123.js"),
            Ok("lotus-explore-rs-d1g2h3i4.wasm".to_owned()),
            "the hashed name wins over the _bg fallback that sits beside it"
        );
    }

    #[test]
    fn punctuation_in_a_token_does_not_split_it() {
        // The tokenizer has to keep `.`, `-` and `_`, or `app-d1g2h3i4.wasm`
        // is read as three tokens and the hash is lost.
        let dir = bundle_naming("import(\"app_v2-d1g2h3i4.wasm\")");
        dir.asset("app_v2-d1g2h3i4.wasm", "\0asm");
        assert_eq!(
            find_wasm_module(dir.path(), "index-abc123.js"),
            Ok("app_v2-d1g2h3i4.wasm".to_owned())
        );
    }

    #[test]
    fn a_hash_with_a_non_alphanumeric_character_is_not_a_hash() {
        // `-d1g2.wasm` hashes to `d1g2`, but a token like `x-y_z.wasm` splits to
        // `z` -- and `x-y_z` is a name, not a hashed bundle output. Anything that
        // is not purely alphanumeric after the last dash is rejected.
        let dir = bundle_naming("import(\"x-y_z.wasm\")");
        dir.asset("x-y_z.wasm", "\0asm");
        let err = find_wasm_module(dir.path(), "index-abc123.js");
        assert!(
            matches!(err, Err(ref e) if e.contains("names no .wasm")),
            "an unrecognised name is not a module, got {err:?}"
        );
    }

    #[test]
    fn a_trailing_dash_is_not_a_hash() {
        let dir = bundle_naming("import(\"app-.wasm\")");
        dir.asset("app-.wasm", "\0asm");
        let err = find_wasm_module(dir.path(), "index-abc123.js");
        assert!(
            matches!(err, Err(ref e) if e.contains("names no .wasm")),
            "an empty hash is not a hash, got {err:?}"
        );
    }

    #[test]
    fn a_wasm_named_but_absent_does_not_count() {
        // The filter requires the file to exist, so a name the glue carries for a
        // variant that was not built into this bundle is ignored.
        let dir = bundle_naming("import(\"lotus-d1g2h3i4.wasm\")");
        assert!(
            find_wasm_module(dir.path(), "index-abc123.js").is_err(),
            "a named but missing module is not found"
        );
    }

    #[test]
    fn an_ambiguous_bundle_is_refused_rather_than_guessed() {
        // Two real modules means the glue was not rewritten for this bundle.
        // Preloading the wrong one is worse than failing, and the message names
        // both so the cause is obvious.
        let dir =
            bundle_naming("import(\"lotus-d1g2h3i4.wasm\"); import(\"lotus-aaaa1111.wasm\");");
        dir.asset("lotus-d1g2h3i4.wasm", "\0asm");
        dir.asset("lotus-aaaa1111.wasm", "\0asm");
        let err = find_wasm_module(dir.path(), "index-abc123.js");
        match err {
            Err(e) => {
                assert!(e.contains("refusing to guess"), "unexpected message: {e}");
                assert!(
                    e.contains("lotus-aaaa1111.wasm"),
                    "the message names the candidates: {e}"
                );
                assert!(
                    e.contains("lotus-d1g2h3i4.wasm"),
                    "the message names the candidates: {e}"
                );
            }
            Ok(name) => panic!("two modules should be refused, got {name}"),
        }
    }

    // ── Where the preload link lands ──────────────────────────────────────────

    #[test]
    fn the_link_goes_on_the_line_after_the_anchor() {
        let out =
            insert_after_anchor(GLUE, "  <link rel=\"preload\" as=\"fetch\">").unwrap_or_default();
        let lines: Vec<&str> = out.lines().collect();
        let anchor = lines
            .iter()
            .position(|line| line.contains("rel=\"preload\" as=\"script\""))
            .unwrap_or_else(|| panic!("the anchor line went missing:\n{out}"));
        assert_eq!(lines[anchor + 1], "  <link rel=\"preload\" as=\"fetch\">");
        // Everything else is untouched and still in order.
        assert_eq!(
            out.lines()
                .filter(|line| line.contains("as=\"fetch\""))
                .count(),
            1
        );
        assert!(
            lines[anchor + 2..]
                .iter()
                .any(|line| line.contains("type=\"module\"")),
            "the module script is still there:\n{out}"
        );
    }

    #[test]
    fn a_document_with_no_newline_gets_the_link_appended() {
        let html = r#"<script type="module" src="/a.js"></script>"#;
        let out = insert_after_anchor(html, "LINK").unwrap_or_default();
        assert_eq!(out, format!("{html}\nLINK\n"));
    }

    #[test]
    fn the_anchor_may_be_the_module_script_when_there_is_no_preload() {
        let html = "  <script type=\"module\" src=\"/a.js\"></script>\n";
        let out = insert_after_anchor(html, "LINK").unwrap_or_default();
        assert_eq!(
            out,
            format!("  <script type=\"module\" src=\"/a.js\"></script>\nLINK\n")
        );
    }

    #[test]
    fn no_anchor_is_an_error_rather_than_a_silent_append() {
        let err = insert_after_anchor("  <p>nothing here</p>\n", "LINK");
        assert!(
            matches!(err, Err(ref e) if e.contains("no <link")),
            "got {err:?}"
        );
    }

    #[test]
    fn missing_glue_is_an_error_not_an_empty_module() {
        let dir = TempDir::new("noglue");
        let err = find_wasm_module(dir.path(), "index-abc123.js");
        assert!(
            matches!(err, Err(ref e) if e.contains("cannot read the JS glue")),
            "got {err:?}"
        );
    }
}
