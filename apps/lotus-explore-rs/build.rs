// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Build script for lotus-explore-rs.
//! Generates site metadata files (llms.txt, robots.txt, sitemap.xml, etc.)
//! from the site-metadata.json configuration.

// No `pub` items are exposed outside this binary/build-script root; the
// crate-level allows for the `buildrs` test target live in build_test.rs.
#[allow(
    unreachable_pub,
    reason = "build.rs compiles both as a build script and as a module inside build_test.rs, with no external crate consumers"
)]
#[path = "build/mod.rs"]
mod modules;

use std::{error::Error, fs, path::PathBuf};

use ai::{build_agent_skills, build_ai_catalog, build_llms_txt};
use csp::inline_script_hashes;
use headers::{build_headers_txt, build_redirects_txt};
use metadata::{Metadata, hostname};
use modules::{ai, csp, headers, metadata, site_files};
use site_files::{build_humans_txt, build_robots_txt, build_security_txt, build_sitemap_xml};

#[cfg(test)]
use csp::{base64_standard, inline_script_texts, sha256};
#[cfg(test)]
use metadata::{host, lotus_home};

/// The files this build script generates into `public/`, relative to it.
/// `clean_dx_output` removes exactly these from `dx`'s output, and nothing else.
const GENERATED_METADATA: [&str; 10] = [
    "llms.txt",
    "humans.txt",
    "robots.txt",
    "sitemap.xml",
    "site.webmanifest",
    "_headers",
    "_redirects",
    ".well-known/ai-catalog.json",
    ".well-known/agent-skills.json",
    ".well-known/security.txt",
];

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let metadata_path = manifest_dir.join("metadata/site-metadata.json");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build");
    println!("cargo:rerun-if-changed=public");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=index.html");
    println!("cargo:rerun-if-changed=../../crates/lotus/src");
    println!("cargo:rerun-if-changed={}", metadata_path.display());

    let raw = fs::read_to_string(&metadata_path)?;
    let mut metadata: Metadata = serde_json::from_str(&raw)?;

    // The live host, for code that must name it (the analytics badge) without
    // repeating it: a CNAME change then reaches it through this file.
    println!("cargo:rustc-env=LOTUS_SITE_HOST={}", hostname(&metadata));

    // Sync site metadata directly to WebManifest identity
    metadata.manifest.name = metadata.site.name.clone();
    metadata.manifest.short_name = metadata.site.short_name.clone();
    metadata.manifest.description = metadata.site.description.clone();

    let public_dir = manifest_dir.join("public");
    let well_known_dir = public_dir.join(".well-known");

    // Read for the CSP digest below, so the hash and the script it describes
    // cannot come from different revisions of this file.
    let index_html = fs::read_to_string(manifest_dir.join("index.html"))?;

    write_if_changed(public_dir.join("llms.txt"), build_llms_txt(&metadata))?;
    write_if_changed(
        well_known_dir.join("ai-catalog.json"),
        build_ai_catalog(&metadata)?,
    )?;
    write_if_changed(
        well_known_dir.join("agent-skills.json"),
        build_agent_skills(&metadata)?,
    )?;
    write_if_changed(public_dir.join("humans.txt"), build_humans_txt(&metadata))?;
    write_if_changed(public_dir.join("robots.txt"), build_robots_txt(&metadata))?;
    write_if_changed(public_dir.join("sitemap.xml"), build_sitemap_xml(&metadata))?;
    write_if_changed(
        well_known_dir.join("security.txt"),
        build_security_txt(&metadata),
    )?;
    write_if_changed(
        public_dir.join("_headers"),
        build_headers_txt(&metadata, &inline_script_hashes(&index_html)),
    )?;
    write_if_changed(
        public_dir.join("_redirects"),
        build_redirects_txt(&metadata),
    )?;
    write_if_changed(
        public_dir.join("site.webmanifest"),
        format!("{}\n", serde_json::to_string_pretty(&metadata.manifest)?),
    )?;

    clean_dx_output()?;

    Ok(())
}

/// Remove the previously-generated metadata files from `dx`'s output tree.
/// This runs on every wasm compile of the app, including `cargo check`, so
/// generated assets stay in sync before bundling.
fn clean_dx_output() -> Result<(), Box<dyn Error>> {
    let Ok(target) = std::env::var("TARGET") else {
        return Ok(());
    };
    if !target.starts_with("wasm32") {
        return Ok(());
    }
    let Some(target_dir) = std::env::var("OUT_DIR").ok().and_then(|out| {
        PathBuf::from(out)
            .ancestors()
            .find(|path| path.file_name().is_some_and(|name| name == "target"))
            .map(PathBuf::from)
    }) else {
        return Ok(());
    };
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "release".into());
    let profile = profile.strip_prefix("wasm-").unwrap_or(profile.as_str());
    let output = target_dir
        .join("dx")
        .join("lotus-explore-rs")
        .join(profile)
        .join("web")
        .join("public");
    if !output.is_dir() {
        return Ok(());
    }
    for name in GENERATED_METADATA {
        let path = output.join(name);
        if path.is_file() {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn write_if_changed(path: PathBuf, contents: String) -> Result<(), Box<dyn Error>> {
    let should_write = fs::read_to_string(&path).map_or(true, |current| current != contents);
    if should_write {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "build/tests.rs"]
mod tests;
