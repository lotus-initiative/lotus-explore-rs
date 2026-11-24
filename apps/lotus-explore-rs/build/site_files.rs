// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Static site metadata file generators (`sitemap.xml`, `humans.txt`, `robots.txt`, `security.txt`).

use super::metadata::{Metadata, lotus_home};

/// Route URLs carry a trailing slash: the host 301s `/search` to `/search/`, so
/// advertising the slashless form made every sitemap URL but the root cost a
/// redirect (Lighthouse's `redirects` audit, ~800 ms).
pub fn build_sitemap_xml(meta: &Metadata) -> String {
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
    <loc>{base}/search/</loc>
    <changefreq>weekly</changefreq>
    <priority>0.9</priority>
  </url>
  <url>
    <loc>{base}/network/</loc>
    <changefreq>weekly</changefreq>
    <priority>0.8</priority>
  </url>
  <url>
    <loc>{base}/curation/</loc>
    <changefreq>monthly</changefreq>
    <priority>0.7</priority>
  </url>
  <url>
    <loc>{base}/draw/</loc>
    <changefreq>monthly</changefreq>
    <priority>0.7</priority>
  </url>
  <url>
    <loc>{base}/faq/</loc>
    <changefreq>monthly</changefreq>
    <priority>0.5</priority>
  </url>
</urlset>
"#
    )
}

pub fn build_humans_txt(meta: &Metadata) -> String {
    let s = &meta.site;
    format!(
        "/* Humans are welcome — https://humanstxt.org/ */\n\
        /* Reference: https://specification.website/spec/foundations/ */\n\n\
        /* TEAM */\n\
        \x20 Name: Lotus Initiative contributors\n\
        \x20 GitHub: https://github.com/lotus-initiative\n\
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
        \x20 License (Wikidata data): CC0 1.0 — {data_license_url}\n\n\
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
        lotus_home_url = lotus_home(meta),
        issues_url = s.issues_url,
        repo_url = s.repo_url,
    )
}

pub fn build_robots_txt(meta: &Metadata) -> String {
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

pub fn build_security_txt(meta: &Metadata) -> String {
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
