// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the build script, in their own file.
//!
//! `build.rs` writes the files the deployed site serves -- `llms.txt`,
//! `robots.txt`, the `OpenAPI` document, the 404 page -- and rewrites the host and
//! base path in them from the build environment. A wrong value here is a
//! published page, so the checks are on the output rather than on the call.

use super::*;
use std::path::Path;

/// The real `site-metadata.json`, so these tests cannot drift from it.
fn real_metadata() -> Result<Metadata, Box<dyn Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("metadata/site-metadata.json");
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

/// A `Metadata` with `base_url` swapped: how a CNAME change is simulated.
fn with_base_url(base_url: &str) -> Result<Metadata, Box<dyn Error>> {
    let mut meta = real_metadata()?;
    meta.site.base_url = base_url.to_owned();
    meta.site.lotus_home_url = None;
    Ok(meta)
}

#[test]
fn host_and_hostname_drop_the_subpath() -> Result<(), Box<dyn Error>> {
    let meta = with_base_url("https://lotus.nprod.net/lotus-explore-rs/")?;
    assert_eq!(
        host(&meta),
        "https://lotus.nprod.net",
        "origin keeps the scheme"
    );
    assert_eq!(
        hostname(&meta),
        "lotus.nprod.net",
        "authority drops the subpath"
    );
    Ok(())
}

#[test]
fn every_host_bearing_artefact_follows_base_url() -> Result<(), Box<dyn Error>> {
    // Change `base_url` and no generated file may keep the old host.
    for base in [
        "https://lotus.nprod.net/lotus-explore-rs/",
        "https://lotus.example.org/explorer/",
        "https://lotus.example.org/",
    ] {
        let meta = with_base_url(base)?;
        let expected = hostname(&meta);
        // Any host that is not this one is stale for this base_url.
        let stale = ["nprod.net", "example.org", "github.io", "localhost"];

        let redirects = build_redirects_txt(&meta);
        assert!(
            redirects.contains(&format!(
                "http://{expected}/*  https://{expected}/:splat  301!"
            )),
            "redirect rule did not follow base_url for {base}"
        );
        assert!(
            !redirects.contains("http://https://"),
            "redirect rule has a doubled scheme for {base}"
        );

        for (name, contents) in [
            ("llms.txt", build_llms_txt(&meta)),
            ("humans.txt", build_humans_txt(&meta)),
            ("robots.txt", build_robots_txt(&meta)),
            ("sitemap.xml", build_sitemap_xml(&meta)),
            ("security.txt", build_security_txt(&meta)),
            ("ai-catalog.json", build_ai_catalog(&meta)?),
        ] {
            assert!(
                contents.contains(expected),
                "{name} does not carry the canonical host for {base}"
            );
            for host in stale {
                if expected.contains(host) {
                    continue;
                }
                assert!(
                    !contents.contains(host),
                    "{name} still names {host} while base_url is {base}"
                );
            }
        }

        assert_eq!(lotus_home(&meta), host(&meta), "LOTUS home for {base}");
    }
    Ok(())
}

#[test]
fn an_explicit_lotus_home_overrides_the_derived_one() -> Result<(), Box<dyn Error>> {
    let mut meta = real_metadata()?;
    meta.site.lotus_home_url = Some("https://example.org/lotus/".to_owned());
    assert_eq!(
        lotus_home(&meta),
        "https://example.org/lotus/",
        "an explicit override wins over the derived host"
    );
    Ok(())
}

#[test]
fn a_port_and_a_missing_scheme_survive_host_extraction() -> Result<(), Box<dyn Error>> {
    let meta = with_base_url("http://localhost:8080/lotus-explore-rs/")?;
    assert_eq!(host(&meta), "http://localhost:8080", "a dev port is kept");
    assert_eq!(
        hostname(&meta),
        "localhost:8080",
        "the port is part of the authority"
    );

    // A schemeless base_url must not yield "://host".
    let meta = with_base_url("lotus.example.org/lotus-explore-rs/")?;
    assert_eq!(
        host(&meta),
        "https://lotus.example.org",
        "a schemeless base_url defaults to https"
    );
    Ok(())
}

#[test]
fn index_html_agrees_with_base_url() -> Result<(), Box<dyn Error>> {
    // `index.html` is the one host-bearing file build.rs does not generate
    // (dx serves it, and writing into it would fight `watch_path`), so this
    // test stands in for generating it. `og:url` and the JSON-LD `url` are
    // the fields with no runtime fallback: `rel=canonical` and the hreflang
    // alternates are rewritten from `location.origin`.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("index.html");
    let html = fs::read_to_string(path)?;
    let base = real_metadata()?.site.base_url;

    for (what, needle) in [
        ("og:url", format!(r#"content="{base}""#)),
        ("JSON-LD url", format!(r#""url":"{base}""#)),
    ] {
        assert!(
            html.contains(&needle),
            "index.html {what} does not match base_url ({base}). \
             Update the hand-maintained URL in index.html, or a CNAME change \
             ships a page whose metadata names the old host."
        );
    }
    Ok(())
}

#[test]
fn index_html_declares_no_route_relative_manifest_or_stylesheet() -> Result<(), Box<dyn Error>> {
    // Both are created by the inline script with an absolute URL instead. A
    // declared relative href resolves against the *document* URL, so on a
    // trailing-slash route (/curation/) the browser asked for
    // /curation/site.webmanifest and /curation/assets/lotus-explore.css.
    // The host answers both with the SPA index.html: the manifest then fails
    // to parse ("Line: 1, column: 1"), and the stylesheet request is wasted.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("index.html");
    let html = fs::read_to_string(path)?;
    for (what, needle) in [
        ("manifest", r#"<link rel="manifest""#),
        ("stylesheet", r#"<link id="app-css""#),
    ] {
        assert!(
            !html.contains(needle),
            "index.html declares a {what} link with a relative href. Create it \
             in the inline script with the basePath URL instead, or a \
             trailing-slash route requests a URL that 404s to the SPA \
             index.html."
        );
    }
    Ok(())
}

#[test]
fn headers_advertise_absolute_link_relations() -> Result<(), Box<dyn Error>> {
    // The Link relations are how an agent finds llms.txt, the catalogs and
    // security.txt. They were root-relative, which resolves to the domain
    // root and 404s on the subpath deploy -- the same trap as the manifest
    // href. Every one is now absolute from `base_url`.
    let meta = real_metadata()?;
    let headers = build_headers_txt(&meta);
    let base = &meta.site.base_url;
    for (name, rel) in [
        ("llms.txt", "http://llmstxt.org/llms.txt"),
        (
            ".well-known/agent-skills.json",
            "https://specification.website/rel/agent-skills",
        ),
        (
            ".well-known/api-catalog.json",
            "https://specification.website/rel/api-catalog",
        ),
        (".well-known/ai-catalog.json", "ai-catalog"),
        ("sitemap.xml", "sitemap"),
        ("robots.txt", "robots"),
        (".well-known/security.txt", "security.txt"),
    ] {
        let expected = format!("Link: <{base}{name}>; rel=\"{rel}\"");
        assert!(
            headers.contains(&expected),
            "_headers does not advertise {name} as {base}{name}; a \
             root-relative target 404s on the subpath deploy"
        );
    }
    assert!(
        !headers.contains("Link: </"),
        "_headers still has a root-relative Link target: it resolves to the \
         domain root, not the app, and 404s"
    );
    Ok(())
}

/// Files allowed to name the live host, and why each is safe:
/// the source; `index.html`, hand-written because dx serves it (guarded by
/// `index_html_agrees_with_base_url`); documentation, where a measurement
/// of the old host is history; and this directory's own test fixtures, which are
/// pinned to the real host on purpose.
const HOST_BEARING_SOURCES: [&str; 6] = [
    "metadata/site-metadata.json",
    "index.html",
    "docs/DEPLOYMENT.md",
    // The build script and these tests, which name the live host to check the
    // rewrite below actually reaches the files it claims to.
    "build.rs",
    "build/tests.rs",
    // Names the published subpath to explain why a web asset URL has to be
    // relative. It is a comment, not a link, and it is the reason a reader
    // does not "fix" it back to a rooted path and break the deployed site.
    "src/vendor_assets.rs",
];

#[test]
fn only_the_documented_files_name_the_live_host() -> Result<(), Box<dyn Error>> {
    // Grepping the tree for the host must find one source, not twenty; a
    // new hardcoded copy has to fail the build rather than be missed by the
    // next CNAME change. Generated files under public/ are committed, so
    // they legitimately contain the host and are excluded by
    // GENERATED_METADATA (asserted still in sync below).
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let meta = real_metadata()?;
    let host = hostname(&meta).to_owned();

    let mut sources = Vec::new();
    let mut generated_with_host = Vec::new();
    for entry in walk(&root)? {
        let path = entry?;
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with("target/") || rel.contains("/target/") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if !text.contains(&host) {
            continue;
        }
        if rel.starts_with("public/")
            && GENERATED_METADATA.contains(&rel["public/".len()..].as_ref())
        {
            generated_with_host.push(rel);
        } else if !HOST_BEARING_SOURCES.contains(&rel.as_ref()) {
            sources.push(rel);
        }
    }

    assert!(
        sources.is_empty(),
        "these files name the live host but are neither generated nor an \
         documented exception, so a CNAME change would miss them: {sources:?}. \
         Add the host to site-metadata.json and generate the file, or list it \
         in HOST_BEARING_SOURCES in build/tests.rs if it is genuinely hand-written."
    );
    // If GENERATED_METADATA drifted, the loop above would stop recognising
    // generated files and this would pass on an empty set.
    assert!(
        !generated_with_host.is_empty(),
        "no generated file contained the host, so the generated-file exclusion \
         is not matching anything - GENERATED_METADATA has drifted from what \
         build.rs actually writes"
    );
    Ok(())
}

/// Minimal recursive walk; `walkdir` is not a dependency of the build script.
fn walk(dir: &Path) -> Result<Vec<std::io::Result<PathBuf>>, Box<dyn Error>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            out.extend(walk(&path)?);
        } else {
            out.push(Ok(path));
        }
    }
    Ok(out)
}

#[test]
fn generated_metadata_list_covers_everything_the_script_writes() {
    // A generated file missing from this list can survive a rename and ship
    // stale, which is how security.txt was missed.
    for name in [
        "llms.txt",
        "humans.txt",
        "robots.txt",
        "sitemap.xml",
        "site.webmanifest",
        "_headers",
        "_redirects",
        ".well-known/ai-catalog.json",
        ".well-known/security.txt",
    ] {
        assert!(
            GENERATED_METADATA.contains(&name),
            "{name} is generated but missing from GENERATED_METADATA"
        );
    }
}

/// Every client-side route.
///
/// One list, asserted against every file that has to name it. The route is
/// real to the browser -- Dioxus renders it -- but a request for it is answered
/// by the *host*, and the host is configured in three places that have no
/// compiler between them. `/faq` was rendered and linked from the nav and still
/// 404'd on a cold load, because it was missing from all three and from the
/// prefix-derivation list besides.
const CLIENT_ROUTES: [&str; 4] = ["/search", "/curation", "/draw", "/faq"];

fn repo_file(relative: &str) -> Result<String, Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relative);
    Ok(fs::read_to_string(root)?)
}

#[test]
fn every_client_route_is_served_by_the_router() -> Result<(), Box<dyn Error>> {
    let router =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/server/mod.rs"))?;
    for route in CLIENT_ROUTES {
        assert!(
            router.contains(&format!(".route(\"{route}\", route_index(")),
            "{route} is a Dioxus route but the server router never serves it, so \
             a cold load of that URL 404s"
        );
    }
    Ok(())
}

#[test]
fn every_client_route_is_published_by_the_export_stage() -> Result<(), Box<dyn Error>> {
    // The `export` stage copies index.html into a directory per route, and that
    // is what the Pages deploy publishes.
    let dockerfile = repo_file("Dockerfile")?;
    for route in CLIENT_ROUTES {
        assert!(
            dockerfile.contains(&format!("public/index.html {route}/index.html")),
            "{route} has no index.html in the export stage, so the static deploy \
             404s on it"
        );
    }
    Ok(())
}

#[test]
fn every_client_route_is_served_by_the_static_image() -> Result<(), Box<dyn Error>> {
    // The nginx routes are one regex alternation, so the assertion is on the
    // bare segment rather than on a `/route` literal that no file contains.
    let nginx = repo_file("docker/nginx.conf")?;
    for route in CLIENT_ROUTES {
        assert!(
            nginx.contains(route.trim_start_matches('/')),
            "{route} is missing from the nginx SPA-route location, so the static \
             image falls through to the 404 page"
        );
    }
    Ok(())
}

#[test]
fn every_client_route_is_recognised_when_deriving_the_base_path() -> Result<(), Box<dyn Error>> {
    // Two lists, and a route missing from either one breaks the in-page asset
    // URLs for anyone who lands on it directly: the base is derived as
    // `/lotus-explore-rs/faq` instead of `/lotus-explore-rs`, and every asset
    // then 404s.
    let html = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("index.html"))?;
    for route in CLIENT_ROUTES {
        assert!(
            html.contains(&format!("\"{route}\"")),
            "{route} is missing from index.html's routeSuffixes, so landing on it \
             derives the wrong base path"
        );
    }

    let url_state = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/features/explore/url_state.rs"),
    )?;
    for route in CLIENT_ROUTES {
        assert!(
            url_state.contains(&format!("\"{route}\"")),
            "{route} is missing from deployment_base_path's suffix list, so \
             deployment_href builds a link under the route instead of the base"
        );
    }
    Ok(())
}

#[test]
fn every_client_route_is_in_the_sitemap() -> Result<(), Box<dyn Error>> {
    // The sitemap, the nav, and the router disagreed: `/faq` was in two of them.
    let meta = real_metadata()?;
    let base = meta.site.base_url.trim_end_matches('/');
    let sitemap = build_sitemap_xml(&meta);
    for route in CLIENT_ROUTES {
        assert!(
            sitemap.contains(&format!("<loc>{base}{route}/</loc>")),
            "{route} is a navigable page but is absent from sitemap.xml"
        );
    }
    Ok(())
}
