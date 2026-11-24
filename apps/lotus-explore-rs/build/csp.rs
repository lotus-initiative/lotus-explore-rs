// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Inline-script extraction and SHA-256 CSP digest formatting for `index.html`.

/// The `script-src` sources for the inline scripts in `index.html`, ready to be
/// interpolated into the CSP as `'sha256-…' 'sha256-…'`.
///
/// A CSP digest is only ever right for one exact text, so this is computed from
/// the file the build copies rather than written down. The digest that used to
/// be written down had drifted from the script it described, and a stale digest
/// does not degrade the page, it deletes the document's only inline script --
/// the one that sets `rel=canonical`, the manifest link, the stylesheet, the
/// base path and the Trusted Types policy. Measured against a host that honours
/// `_headers` (Cloudflare Pages or Netlify), that is an unstyled boot screen
/// forever: no stylesheet request is ever made, `webmcp.js`
/// is never loaded, and with no default policy every `innerHTML` sink in the
/// WASM client throws. So the hash is derived, and
/// `the_csp_carries_the_digest_of_every_inline_script` in `build/tests.rs`
/// ties the two files together.
///
/// `type="application/ld+json"` blocks are hashed along with the rest. CSP
/// never executes them, so the extra source is inert; skipping it would mean
/// carrying a list of "types that are not really scripts", which is a list that
/// has to be updated when someone adds another.
pub fn inline_script_hashes(index_html: &str) -> String {
    let mut sources: Vec<String> = inline_script_texts(index_html)
        .into_iter()
        .map(|script| format!("'sha256-{}'", base64_standard(&sha256(script.as_bytes()))))
        .collect();
    sources.sort();
    sources.dedup();
    sources.join(" ")
}

/// The text of every inline `<script>` in a document, exactly as a browser sees
/// it.
///
/// A small scanner rather than an HTML parser, because what a digest needs is
/// only the span between an open tag and the `</script` that closes it -- and
/// the HTML parser's rule for that end (`</script` ends it; a string literal
/// does not) is simpler than the rest of the grammar. A `<script src=…>` is
/// skipped: its text is empty and `'self'` already allows it.
pub fn inline_script_texts(html: &str) -> Vec<&str> {
    let mut scripts = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = html[cursor..].find("<script") {
        let tag_start = cursor + offset;
        let after_name = tag_start + "<script".len();
        // `<scripting>` is another element, and its body is not script text.
        if html[after_name..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            cursor = after_name;
            continue;
        }
        let Some(tag) = html[after_name..].split_once('>').map(|(tag, _)| tag) else {
            break;
        };
        let body_start = after_name + tag.len() + 1;
        let Some(end) = html[body_start..].find("</script") else {
            break;
        };
        if !tag_has_attribute(tag, "src") {
            scripts.push(&html[body_start..body_start + end]);
        }
        cursor = body_start + end;
    }
    scripts
}

/// Whether an open tag carries `name`, matched on attribute boundaries so
/// `src` does not match `data-src`.
fn tag_has_attribute(tag: &str, name: &str) -> bool {
    tag.split(|c: char| c.is_ascii_whitespace() || c == '=' || c == '/')
        .any(|field| field.eq_ignore_ascii_case(name))
}

pub fn sha256(bytes: &[u8]) -> Vec<u8> {
    use sha2::Digest as _;
    sha2::Sha256::digest(bytes).to_vec()
}

/// Standard base64 with padding, which is the encoding a CSP `sha256-` source
/// is written in.
///
/// Hand-rolled because the only thing this workspace needs it for is a 32-byte
/// digest and no base64 crate is in the graph. `sha256("")` and `sha256("ok")`
/// pin it in `build/tests.rs`.
pub fn base64_standard(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let byte = |index: usize| u32::from(chunk.get(index).copied().unwrap_or(0));
        let triple = (byte(0) << 16) | (byte(1) << 8) | byte(2);
        for index in 0..4 {
            // Every chunk contributes four characters: a short final chunk
            // contributes padding instead of the bytes it does not have.
            let symbol = if index <= chunk.len() {
                // The mask is what keeps this in range: a 6-bit group indexes
                // 64 characters. `?` is unreachable, and says so if it is not.
                char::from(
                    ALPHABET
                        .get(((triple >> (18 - 6 * index)) & 0x3f) as usize)
                        .copied()
                        .unwrap_or(b'?'),
                )
            } else {
                '='
            };
            out.push(symbol);
        }
    }
    out
}
