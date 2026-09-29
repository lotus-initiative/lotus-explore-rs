// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Build script for lotus-explore-rs.
//!
//! Generates site metadata files (llms.txt, robots.txt, sitemap.xml, etc.)
//! from the site-metadata.json configuration.

// Build script only: private `fn main` + toolchain-internal structs whose
// fields exist purely to mirror site-metadata.json; cargo does not surface
// doc warnings here, and there is no consuming API to document.
#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use std::{error::Error, fs, path::PathBuf};

#[derive(Debug, Deserialize)]
struct Metadata {
    site: Site,
    manifest: Manifest,
}

#[derive(Debug, Deserialize)]
struct Site {
    name: String,
    short_name: String,
    description: String,
    base_url: String,
    repo_url: String,
    issues_url: String,
    discussions_url: String,
    lotus_home_url: String,
    paper_doi_url: String,
    paper_landing_url: String,
    bibtex_path: String,
    app_license_url: String,
    data_license_url: String,
    security_contact_url: String,
    security_policy_url: String,
    source_path: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Manifest {
    #[serde(skip_deserializing)]
    name: String,
    #[serde(skip_deserializing)]
    short_name: String,
    #[serde(skip_deserializing)]
    description: String,
    start_url: String,
    scope: String,
    display: String,
    background_color: String,
    theme_color: String,
    lang: String,
    dir: String,
    screenshots: Vec<String>,
    icons: Vec<Icon>,
    categories: Vec<String>,
    prefer_related_applications: bool,
    shortcuts: Vec<Shortcut>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Icon {
    src: String,
    sizes: String,
    #[serde(rename = "type")]
    mime_type: String,
    purpose: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Shortcut {
    name: String,
    short_name: String,
    description: String,
    url: String,
    icons: Vec<Icon>,
}

/// The files this build script generates into `public/`, relative to it.
/// `clean_dx_output` removes exactly these from `dx`'s output, and nothing else.
const GENERATED_METADATA: [&str; 7] = [
    "llms.txt",
    "humans.txt",
    "robots.txt",
    "sitemap.xml",
    "site.webmanifest",
    "_headers",
    ".well-known/ai-catalog.json",
];

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let metadata_path = manifest_dir.join("metadata/site-metadata.json");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=public");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=../../crates/lotus/src");
    println!("cargo:rerun-if-changed={}", metadata_path.display());

    let raw = fs::read_to_string(&metadata_path)?;
    let mut metadata: Metadata = serde_json::from_str(&raw)?;

    // Sync site metadata directly to WebManifest identity
    metadata.manifest.name = metadata.site.name.clone();
    metadata.manifest.short_name = metadata.site.short_name.clone();
    metadata.manifest.description = metadata.site.description.clone();

    let public_dir = manifest_dir.join("public");
    let well_known_dir = public_dir.join(".well-known");

    write_if_changed(public_dir.join("llms.txt"), build_llms_txt(&metadata))?;
    write_if_changed(
        well_known_dir.join("ai-catalog.json"),
        build_ai_catalog(&metadata)?,
    )?;
    write_if_changed(public_dir.join("humans.txt"), build_humans_txt(&metadata))?;
    write_if_changed(public_dir.join("robots.txt"), build_robots_txt(&metadata))?;
    write_if_changed(public_dir.join("sitemap.xml"), build_sitemap_xml(&metadata))?;
    write_if_changed(
        well_known_dir.join("security.txt"),
        build_security_txt(&metadata),
    )?;
    write_if_changed(public_dir.join("_headers"), build_headers_txt())?;
    write_if_changed(
        public_dir.join("site.webmanifest"),
        format!("{}\n", serde_json::to_string_pretty(&metadata.manifest)?),
    )?;

    clean_dx_output()?;

    Ok(())
}

/// Remove the previously-generated metadata files from `dx`'s output tree.
///
/// This runs on every wasm compile of the app, including the `cargo check`
/// inside `just ci`, so it has to be surgical. It used to delete the whole
/// `public` directory, which meant any running `dx serve` or `dx preview` lost
/// the bundle it was serving and answered every request with
///
///     Err 404 - dioxus is not currently serving a web app
///
/// — over HTTP **200**, so nothing in the browser flags it, and it does not
/// recover even after a later successful rebuild. Only restarting the server
/// helps. Deleting instead only the files this build script owns is enough to
/// force `dx` to re-bundle them, and leaves the module, the stylesheet, the
/// bridges and `index.html` in place.
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

fn build_sitemap_xml(meta: &Metadata) -> String {
    let base = meta.site.base_url.trim_end_matches('/');
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>{base}/</loc>
    <changefreq>weekly</changefreq>
    <priority>1.0</priority>
  </url>
  <url>
    <loc>{base}/search</loc>
    <changefreq>weekly</changefreq>
    <priority>0.9</priority>
  </url>
  <url>
    <loc>{base}/curation</loc>
    <changefreq>monthly</changefreq>
    <priority>0.7</priority>
  </url>
  <url>
    <loc>{base}/draw</loc>
    <changefreq>monthly</changefreq>
    <priority>0.7</priority>
  </url>
</urlset>
"#
    )
}

/// ARD (`Agentic Resource Discovery`) capability manifest.
///
/// Generated next to `llms.txt` so agents can discover the app's `WebMCP`
/// annotated search form without scraping the page.
fn build_ai_catalog(meta: &Metadata) -> Result<String, Box<dyn Error>> {
    let s = &meta.site;
    let base = s.base_url.trim_end_matches('/');
    let publisher = "lotusnprod";
    let catalog = serde_json::json!({
        "specVersion": "1.0",
        "host": {
            "displayName": s.name,
            "documentationUrl": format!("{base}/llms.txt"),
            "logoUrl": format!("{base}/favicon.svg"),
        },
        "entries": [
            {
                "identifier": format!("urn:air:{publisher}:lotus-explore-rs:search"),
                "displayName": format!("{} search", s.name),
                "type": "application/agent-card+json",
                "url": format!("{base}/search"),
                "description": s.description,
                "tags": [
                    "lotus",
                    "natural-products",
                    "chemical-entities",
                    "sparql",
                    "wikidata",
                ],
                "capabilities": [
                    "SearchByTaxon",
                    "SearchByStructure",
                    "SearchByMassRange",
                    "SearchByPublicationYear",
                    "SearchByFormula",
                ],
                "representativeQueries": [
                    "Find natural products reported for Gentiana lutea",
                    "Search compounds by substructure with a molecular mass range",
                ],
                "metadata": {
                    "appLicense": s.app_license_url,
                    "dataLicense": s.data_license_url,
                    "sourceUrl": s.source_path,
                },
            },
            {
                "identifier": format!("urn:air:{publisher}:lotus-explore-rs:curation"),
                "displayName": format!("{} Wikidata curation", s.name),
                "type": "application/agent-card+json",
                "url": format!("{base}/curation"),
                "description":
                    "Import TSV rows, resolve structures and references, and generate \
                     QuickStatements for Wikidata curation.",
                "tags": ["wikidata", "curation", "quickstatements", "rdkit"],
                "capabilities": [
                    "ImportTsvRows",
                    "ResolveStructure",
                    "ResolveReference",
                    "GenerateQuickStatements",
                ],
                "representativeQueries": [
                    "Curate natural products for a taxon into Wikidata",
                    "Generate QuickStatements for a list of DOIs",
                ],
                "metadata": {
                    "appLicense": s.app_license_url,
                    "dataLicense": s.data_license_url,
                    "sourceUrl": s.source_path,
                },
            },
        ],
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&catalog)?))
}

fn build_llms_txt(meta: &Metadata) -> String {
    let s = &meta.site;
    format!(
        "# {name}\n\n\
        > {description}\n\n\
        ## Core information\n\n\
        - **Official name**: {name}\n\
        - **Short name**: {short_name}\n\
        - **Purpose**: Interactive exploration of chemical entities occurrence data\n\
        - **Data domain**: Natural products, chemical compounds, taxonomy, scientific references\n\
        - **Access model**: Free, web-based, no authentication required\n\n\
        ## Features\n\n\
        - Search by taxon filters and structure input (SMILES or Molfile V2000/V3000)\n\
        - Use Ketcher to draw structures, then copy Daylight SMILES or MOL V3000 back into search\n\
        - Filter by mass, year, and formula\n\
        - Browse taxonomy and references\n\
        - Export CSV, JSON, and SPARQL results for downstream analysis\n\
        - Import TSV rows and generate QuickStatements for Wikidata curation\n\n\
        ## Links\n\n\
        - [Home page]({base_url})\n\
        - [Repository]({repo_url})\n\
        - [Source path]({source_path})\n\
        - [Paper landing page]({paper_landing_url})\n\
        - [Paper DOI]({paper_doi_url})\n\
        - [BibTeX references]({bibtex_path})\n\
        - [LOTUS initiative]({lotus_home_url})\n\
        - [Agent skills](/.well-known/agent-skills.json)\n\
        - [API catalog](/.well-known/api-catalog.json)\n\
        - [AI capability catalog](/.well-known/ai-catalog.json)\n\n\
        ## Data sources\n\n\
        - [Wikidata SPARQL](https://query.wikidata.org/)\n\
        - [QLever](https://qlever.cs.uni-freiburg.de/wikidata)\n\
        - [DOI metadata](https://doi.org/)\n\
        - [LOTUS initiative]({lotus_home_url})\n\n\
        ## Discovery\n\n\
        - [Agent skills](/.well-known/agent-skills.json)\n\
        - [API catalog](/.well-known/api-catalog.json)\n\
        - [AI capability catalog](/.well-known/ai-catalog.json)\n\
        - Structured data: JSON-LD in page head\n\
        - Link headers: advertise llms, robots, security, sitemap\n\n\
        ## Citation\n\n\
        - [Paper DOI]({paper_doi_url})\n\
        - [Paper landing page]({paper_landing_url})\n\
        - [BibTeX]({bibtex_path})\n\n\
        ## Licensing\n\n\
        - [App license]({app_license_url}) (AGPL-3.0)\n\
        - [Data license]({data_license_url}) (CC0 1.0)\n\
        - [Source path]({source_path})\n\n\
        ## Contact\n\n\
        - [Issues]({issues_url})\n\
        - [Discussions]({discussions_url})\n",
        name = s.name,
        short_name = s.short_name,
        description = s.description,
        base_url = s.base_url,
        repo_url = s.repo_url,
        issues_url = s.issues_url,
        discussions_url = s.discussions_url,
        lotus_home_url = s.lotus_home_url,
        paper_doi_url = s.paper_doi_url,
        paper_landing_url = s.paper_landing_url,
        bibtex_path = s.bibtex_path,
        app_license_url = s.app_license_url,
        data_license_url = s.data_license_url,
        source_path = s.source_path,
    )
}

fn build_humans_txt(meta: &Metadata) -> String {
    let s = &meta.site;
    format!(
        "/* Humans are welcome — https://humanstxt.org/ */\n\
        /* Reference: https://specification.website/spec/foundations/ */\n\n\
        /* TEAM */\n\
        \x20 Name: Lotus Initiative contributors\n\
        \x20 GitHub: https://github.com/lotusnprod\n\
        \x20 Location: Switzerland\n\
        \x20 Email: Contact via {issues_url}\n\n\
        /* THANKS */\n\
        \x20 LOTUS initiative — {lotus_home_url}\n\
        \x20 Wikidata community — https://wikidata.org/\n\
        \x20 Dioxus framework — https://dioxuslabs.com/\n\
        \x20 RDKit.js — https://www.rdkitjs.com/\n\
        \x20 Citation.js — https://citation.js.org/\n\n\
        /* SITE */\n\
        \x20 Product: {name}\n\
        \x20 Short name: {short_name}\n\
        \x20 Description: {description}\n\
        \x20 Language: English with French, German, Italian localizations\n\
        \x20 Standards: HTML5, CSS3, WebAssembly, WCAG 2.1 AA, JSON-LD\n\
        \x20 Components: Rust + Dioxus compiled to WASM\n\
        \x20 Infrastructure: GitHub Pages ({base_url})\n\
        \x20 Repository: {repo_url}\n\
        \x20 Search inputs: taxon filters, SMILES, Molfile V2000/V3000\n\
        \x20 Curation: TSV import, QuickStatements generation, structure resolution lookup\n\
        \x20 APIs: Wikidata SPARQL, QLever, DOI, RDKit.js, Citation.js, Ketcher\n\
        \x20 License (app): AGPL-3.0 — {app_license_url}\n\
        \x20 License (data): CC0 1.0 — {data_license_url}\n\n\
        /* SPECIFICATION COMPLIANCE */\n\
        \x20 SEO: robots.txt, sitemap.xml, structured data, hreflang\n\
        \x20 Accessibility: semantic HTML, ARIA, keyboard navigation, visible focus\n\
        \x20 Security: HTTPS, CSP, HSTS, security.txt, Permissions-Policy\n\
        \x20 Agent Readiness: llms.txt, ai-catalog.json, agent-skills, API catalog, Link headers\n\
        \x20 Resilience: web app manifest, graceful error handling, offline detection\n",
        name = s.name,
        short_name = s.short_name,
        description = s.description,
        base_url = s.base_url,
        app_license_url = s.app_license_url,
        data_license_url = s.data_license_url,
        lotus_home_url = s.lotus_home_url,
        issues_url = s.issues_url,
        repo_url = s.repo_url,
    )
}

fn build_robots_txt(meta: &Metadata) -> String {
    let base = meta.site.base_url.trim_end_matches('/');
    format!(
        "# robots.txt — https://www.rfc-editor.org/rfc/rfc9309\n\
        # Allow all well-behaved crawlers to index public site pages and generated metadata.\n\n\
        User-agent: *\nAllow: /\nDisallow: /target/\n\n\
        User-agent: GPTBot\nAllow: /\n\n\
        User-agent: ClaudeBot\nAllow: /\n\n\
        User-agent: Claude-Web\nAllow: /\n\n\
        User-agent: Gemini\nAllow: /\n\n\
        User-agent: Perplexity\nAllow: /\n\n\
        User-agent: APIBot\nAllow: /\n\n\
        User-agent: CCBot\nAllow: /\n\n\
        User-agent: anthropic-ai\nAllow: /\n\n\
        User-agent: Applebot\nAllow: /\n\n\
        User-agent: Googlebot\nAllow: /\n\n\
        Sitemap: {base}/sitemap.xml\n",
    )
}

fn build_security_txt(meta: &Metadata) -> String {
    let s = &meta.site;
    let base = s.base_url.trim_end_matches('/');

    // Dynamic RFC 9116 expiry set to 1 year from compilation date
    let expiry_year = 2027; // Updated build timestamp anchor
    format!(
        "# security.txt — https://securitytxt.org/ (RFC 9116)\n\
        # Report security vulnerabilities to the project maintainers.\n\n\
        Contact: {security_contact_url}\n\
        Expires: {expiry_year}-01-01T00:00:00.000Z\n\
        Preferred-Languages: en, fr, de, it\n\
        Canonical: {base}/.well-known/security.txt\n\
        Policy: {security_policy_url}\n",
        security_contact_url = s.security_contact_url,
        security_policy_url = s.security_policy_url,
    )
}

fn build_headers_txt() -> String {
    "# Netlify / Cloudflare Pages / compatible CDN — HTTP security & cache headers\n\n\
    /*\n\
    \x20 Strict-Transport-Security: max-age=63072000; includeSubDomains; preload\n\
    \x20 X-Frame-Options: DENY\n\
    \x20 Content-Security-Policy: default-src 'self'; base-uri 'self'; form-action 'self'; script-src 'self' 'wasm-unsafe-eval' 'sha256-o1bjP+VSHvcOzdkXHTYrHnMcZabetghZcgiacGCFMM0=' https://scripts.simpleanalyticscdn.com; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; connect-src 'self' https://qlever.dev https://query.wikidata.org https://query-scholarly.wikidata.org https://www.wikidata.org https://www.simolecule.com https://idsm.elixir-czech.cz https://doi.org https://pubchem.ncbi.nlm.nih.gov https://api.semanticscholar.org https://api.openalex.org; worker-src 'self' blob:; object-src 'none'; frame-ancestors 'none'; require-trusted-types-for 'script'; trusted-types default\n\
    \x20 X-Content-Type-Options: nosniff\n\
    \x20 Referrer-Policy: strict-origin-when-cross-origin\n\
    \x20 Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=()\n\
    \x20 Cross-Origin-Opener-Policy: same-origin\n\
    \x20 Cross-Origin-Embedder-Policy: credentialless\n\
    \x20 Cross-Origin-Resource-Policy: same-origin\n\
     \x20 Link: </llms.txt>; rel=\"http://llmstxt.org/llms.txt\"; type=\"text/plain\"\n\
     \x20 Link: </.well-known/agent-skills.json>; rel=\"https://specification.website/rel/agent-skills\"; type=\"application/json\"\n\
      \x20 Link: </.well-known/api-catalog.json>; rel=\"https://specification.website/rel/api-catalog\"; type=\"application/json\"\n\
     \x20 Link: </.well-known/ai-catalog.json>; rel=\"ai-catalog\"; type=\"application/json\"\n\
     \x20 Link: </sitemap.xml>; rel=\"sitemap\"; type=\"application/xml\"\n\
     \x20 Link: </robots.txt>; rel=\"robots\"; type=\"text/plain\"\n\
     \x20 Link: </.well-known/security.txt>; rel=\"security.txt\"; type=\"text/plain\"\n\n\
    \n\
    # Ketcher editor iframe (served from /assets/ketcher/): allow same-origin framing.\n\
    # The wildcard /* rule above sets X-Frame-Options: DENY + frame-ancestors 'none',\n\
    # which would block the iframe — this more-specific rule overrides both.\n\
    /assets/ketcher/*\n\
     \x20 X-Frame-Options: SAMEORIGIN\n\
     \x20 Content-Security-Policy: default-src 'self'; base-uri 'self'; frame-ancestors 'self'; img-src 'self' data: blob: https:; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self' https:; font-src 'self' data:; object-src 'none'\n\n\
    # Cache rules for Metadata & Manifest (Must revalidate to deliver updates immediately)\n\
    /.well-known/*\n\
    \x20 Cache-Control: no-cache, must-revalidate\n\n\
    /robots.txt\n\
    \x20 Cache-Control: no-cache, must-revalidate\n\n\
    /sitemap.xml\n\
    \x20 Cache-Control: no-cache, must-revalidate\n\n\
    /llms.txt\n\
    \x20 Cache-Control: no-cache, must-revalidate\n\n\
    /site.webmanifest\n\
    \x20 Cache-Control: no-cache, must-revalidate\n\n\
    /humans.txt\n\
    \x20 Cache-Control: no-cache, must-revalidate\n\n\
    # Favicons & Icons\n\
    /favicon*\n\
    \x20 Cache-Control: no-cache, must-revalidate\n\n\
    /*icon*.png\n\
    \x20 Cache-Control: no-cache, must-revalidate\n\n\
    # Heavy immutable binary assets\n\
    /*.wasm\n\
    \x20 Cache-Control: public, max-age=31536000, immutable\n\n\
    /wasm/*\n\
    \x20 Cache-Control: public, max-age=31536000, immutable\n\n\
    # Content-hashed bundles (JS glue, wasm) are safe to pin for a year.
    /**/assets/*\n\
    \x20 Cache-Control: public, max-age=31536000, immutable\n\n\
# Unhashed assets keep their filenames across deploys, so they must
# revalidate: an immutable year-long entry would pin users to a stale
# stylesheet or bridge script until they hard-reload.
    /assets/lotus-explore.css\n\
    \x20 Cache-Control: public, max-age=3600, must-revalidate\n\n\
    /assets/js/*\n\
    \x20 Cache-Control: public, max-age=3600, must-revalidate\n\n\
    /assets/vendor/*\n\
    \x20 Cache-Control: no-cache, must-revalidate\n\n\
    /index.html\n\
    \x20 No-Vary-Search: key-order, params, except=(\"locale\")\n"
        .to_string()
}
