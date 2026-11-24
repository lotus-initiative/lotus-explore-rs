// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Netlify / Cloudflare Pages `_headers` and `_redirects` generators.

use super::metadata::{Metadata, hostname};

/// `_headers`, with the CSP that this app can actually run under.
///
/// Two directives are shaped by what the client does at runtime, not by
/// preference, and both were measured on a host that honours this file:
///
/// - `'unsafe-eval'` is in `script-src` because the WASM client evaluates
///   strings: `document::eval` sets the theme and the `RDKit` bridge calls
///   (`features/curation/services/http_client.rs`,
///   `components/form_sections/field_examples.rs`,
///   `document_head.rs`, `app/shell.rs`). `'wasm-unsafe-eval'` covers
///   WebAssembly compilation and nothing else. Measured without it, the theme
///   effect threw an `EvalError`, an effect in the same flush panicked
///   (`RuntimeError: unreachable`), and the boot shell was never removed, so
///   the page rendered behind "Loading the explorer…" forever.
/// - `require-trusted-types-for 'script'` is gone, and could not be kept even
///   with `'unsafe-eval'`: `eval` is a Trusted Types sink and a policy written
///   in JavaScript cannot return a `TrustedScript`, so enforcement blocks the
///   app's own code. `index.html` still creates the default policy, first thing
///   in its inline script and ahead of every sink, so putting enforcement back
///   is adding `require-trusted-types-for 'script'` here --- and
///   `trusted-types default` with it, if policy names are to stay restricted
///   --- once nothing in the client evaluates strings.
///
/// The rest is posture: no framing, no plugins, forms self-only, and a
/// `connect-src` that names every third-party endpoint the client talks to.
pub fn build_headers_txt(meta: &Metadata, script_hashes: &str) -> String {
    let base = &meta.site.base_url;
    // The Link relations are absolute, from `base_url`. They were root-relative
    // (`</llms.txt>`), which resolves to the domain root and 404s on the subpath
    // deploy -- the same trap as the manifest href.
    format!(
    "# Netlify / Cloudflare Pages / compatible CDN — HTTP security & cache headers\n\n\
    /*\n\
    \x20 Strict-Transport-Security: max-age=63072000; includeSubDomains; preload\n\
    \x20 X-Frame-Options: DENY\n\
    \x20 Content-Security-Policy: default-src 'self'; base-uri 'self'; form-action 'self'; script-src 'self' 'wasm-unsafe-eval' 'unsafe-eval' {script_hashes}; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; connect-src 'self' https://qlever.dev https://query-next.wikidata.org https://query-scholarly-next.wikidata.org https://www.wikidata.org https://www.simolecule.com https://idsm.elixir-czech.cz https://doi.org https://pubchem.ncbi.nlm.nih.gov https://api.semanticscholar.org https://api.openalex.org; worker-src 'self' blob:; object-src 'none'; frame-ancestors 'none'\n\
    \x20 X-Content-Type-Options: nosniff\n\
    \x20 Referrer-Policy: strict-origin-when-cross-origin\n\
    \x20 Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=()\n\
    \x20 Cross-Origin-Opener-Policy: same-origin\n\
    \x20 Cross-Origin-Embedder-Policy: credentialless\n\
    \x20 Cross-Origin-Resource-Policy: same-origin\n\
     \x20 Link: <{base}llms.txt>; rel=\"http://llmstxt.org/llms.txt\"; type=\"text/plain\"\n\
     \x20 Link: <{base}.well-known/agent-skills.json>; rel=\"https://specification.website/rel/agent-skills\"; type=\"application/json\"\n\
     \x20 Link: <{base}.well-known/api-catalog.json>; rel=\"https://specification.website/rel/api-catalog\"; type=\"application/json\"\n\
     \x20 Link: <{base}.well-known/ai-catalog.json>; rel=\"ai-catalog\"; type=\"application/json\"\n\
     \x20 Link: <{base}sitemap.xml>; rel=\"sitemap\"; type=\"application/xml\"\n\
     \x20 Link: <{base}robots.txt>; rel=\"robots\"; type=\"text/plain\"\n\
     \x20 Link: <{base}.well-known/security.txt>; rel=\"security.txt\"; type=\"text/plain\"\n\n\
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
# `/assets/*`, not `/**/assets/*`: `dx` writes the hashed bundle to
# `/assets/<name>-<hash>.<ext>`, and a glob that expects a directory in front
# of `assets` does not match it. Lighthouse's `cache-insight` reported the
# 12 KiB glue with a zero cache lifetime for exactly that reason, on the hosts
# that honour this file.
#
# A path starts its line. Indented, it reads as another header of the block
# above and the rule is silently dropped, which is how
# `/assets/lotus-explore.css` spent its life with no rule at all.
/assets/*\n\
    \x20 Cache-Control: public, max-age=31536000, immutable\n\n\
# Unhashed assets keep their filenames across deploys, so they must
# revalidate: an immutable year-long entry would pin users to a stale
# stylesheet or bridge script until they hard-reload. These follow
# `/assets/*` above, so they override it.
/assets/lotus-explore.css\n\
    \x20 Cache-Control: public, max-age=3600, must-revalidate\n\n\
    /assets/js/*\n\
    \x20 Cache-Control: public, max-age=3600, must-revalidate\n\n\
    /assets/vendor/*\n\
    \x20 Cache-Control: no-cache, must-revalidate\n\n\
    /index.html\n\
    \x20 No-Vary-Search: key-order, params, except=(\"locale\")\n"
    )
}

/// `_redirects` is generated so the host lives only in `base_url`. The file is
/// inert on GitHub Pages but live on Cloudflare/Netlify, so a hand-kept host
/// there would have redirected to a domain that redirects back here.
pub fn build_redirects_txt(meta: &Metadata) -> String {
    // `http://HOST/*` needs the bare host; the full origin gives
    // `http://https://host/*`, which is not a valid target.
    let hostname = hostname(meta);
    format!(
        "# Netlify / Cloudflare Pages / compatible CDN \u{2014} HTTP redirect rules\n\
        # Reference: https://specification.website/spec/seo/\n\
        # Reference: https://specification.website/spec/seo/redirects/\n\
        #\n\
        # GENERATED by build.rs from metadata/site-metadata.json. Do not edit by\n\
        # hand: change `base_url` there instead, or the host in the rule below\n\
        # will drift from the canonical host used by every other file.\n\n\
        # Ensure canonical URL: strip trailing slash on non-root paths (optional)\n\
        # HTTP -> HTTPS. The `!` forces this ahead of the SPA rewrite below, so plain\n\
        # HTTP is never served. This is a host-level redirect, so it has to be the\n\
        # first rule. GitHub Pages ignores this whole file (it is Cloudflare/Netlify\n\
        # syntax) — there the equivalent is the \"Enforce HTTPS\" setting in the repo's\n\
        # Pages configuration, which is not expressible as a file. See docs/DEPLOYMENT.md.\n\
        http://{hostname}/*  https://{hostname}/:splat  301!\n\n\
        # SPA fallback: redirect unknown routes to index.html (200 = rewrite, not 301).\n\
        # Excludes static asset paths so the Ketcher iframe and WASM bundle load directly.\n\
        !/assets/*\n\
        !/wasm/*\n\
        !/favicon*\n\
        !/apple-touch-icon*\n\
        !/android-chrome*\n\
        !/site.webmanifest\n\
        !/robots.txt\n\
        !/sitemap.xml\n\
        !/llms.txt\n\
        !/humans.txt\n\
        !/.well-known/*\n\
        /*  /index.html  200\n"
    )
}
