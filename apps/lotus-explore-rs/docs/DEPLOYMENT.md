# Deployment

The site is published by `.github/workflows/deploy.yml` with
`actions/upload-pages-artifact` + `actions/deploy-pages`, and is served from
`https://lotus.nprod.net/lotus-explore-rs/`. Everything below was measured
against the live host with `curl`, not inferred from the configuration.

## The canonical host is `lotus.nprod.net`

`lotus.nprod.net` is a CNAME to `lotusnprod.github.io`, and GitHub Pages 301s
the `github.io` name to the custom domain. So `github.io` is not an alternative
address for this site, it is a redirect away from it. Measured:

  | Request                                          | Result                                                 |
  | ------------------------------------------------ | ------------------------------------------------------ |
  | `https://lotusnprod.github.io/lotus-explore-rs/` | **301** -> `https://lotus.nprod.net/lotus-explore-rs/` |
  | `https://lotusnprod.github.io/`                  | **301** -> `https://lotus.nprod.net/`                  |
  | `https://lotus.nprod.net/lotus-explore-rs/`      | 200                                                    |
  | `https://lotus.nprod.net/`                       | 200                                                    |

`base_url` in `metadata/site-metadata.json` was still the `github.io` form,
which put the retired host into every generated artefact: `llms.txt`,
`robots.txt`'s `Sitemap:`, `sitemap.xml`, `humans.txt`, `security.txt`'s
`Canonical:`, and the `ai-catalog.json` `documentationUrl`, `logoUrl` and
per-tool `url` fields. An agent that fetched `documentationUrl` was sent to a
host that redirects, and `sitemap.xml` advertised redirect URLs to crawlers. All
of them now use `lotus.nprod.net`; `lotus_home_url` moves with it, since the
LOTUS initiative home is the same site at the root.

`index.html` is hand-maintained rather than generated, so its `og:url` and the
JSON-LD `url` are set by hand to match. Note the contrast with `rel=canonical`:
that one is empty in the source and rewritten at runtime from
`window.location.origin`, so it was always correct and never named the wrong
host. `og:url` and JSON-LD have no such fallback, which is exactly why they were
the ones that went stale.

Worth keeping in mind when reading a canonical URL from this repo: the
`github.io` name is a redirect, not a synonym. Anything that hardcodes it adds a
hop, and anything that treats it as the site's identity (a sitemap, an agent
catalog, a citation) points at the wrong origin.

### Changing the CNAME

`rg nprod.net` looks alarming -- 43 hits, 13 of them in this file -- but almost
every hit is generated output that is committed to the tree. There is exactly
**one** value to edit, plus one hand-written mirror of it, and tests fail if
either is missed.

Use `rg --hidden` for the full count: `.well-known/` is a dotfile directory, so
ripgrep skips it by default and the census looks smaller than it is.

  | File                          | Kind          | On a CNAME change                                                     |
  | ----------------------------- | ------------- | --------------------------------------------------------------------- |
  | `metadata/site-metadata.json` | **source**    | **Edit `base_url`. This is the change.**                              |
  | `index.html`                  | hand-written  | **Edit 2 URLs**: `og:url`, JSON-LD `url`. A test fails if you do not. |
  | `build.rs`                    | test fixtures | No action. Pinned to the real host on purpose.                        |
  | `docs/DEPLOYMENT.md`          | prose         | No action. A measurement of the old host is history.                  |

So a CNAME change is: edit `base_url`, edit the two `index.html` URLs, rebuild.
Nothing else in the tree names the host as a value.

The generated files are `public/llms.txt` (9 URLs), `public/sitemap.xml` (4),
`public/.well-known/ai-catalog.json` (4), `public/humans.txt` (2),
`public/robots.txt`, `public/.well-known/security.txt` and `public/_redirects` --
22 of the 43 hits. They are committed because the site is served from the
working tree, not built at deploy time, so they must be checked in. They are
regenerated from `base_url` on every build and never edited by hand.

`build.rs` derives the host from `base_url` with a single `hostname()` helper
and generates every artefact that carries an absolute URL, so none of them
restate a hostname. `lotus_home_url` was a second field naming the same host; it
is now optional and defaults to the origin of `base_url`, so it only needs
setting if the initiative ever moves to a host of its own.

`index.html` is the one file that stays hand-written: it is what dx serves, and
having build.rs write into it would fight the dev server's `watch_path` on that
path. It matters because its `og:url` and JSON-LD `url` are precisely the two
metadata fields with no runtime fallback --- `rel=canonical` and the `hreflang`
alternates are rewritten from `window.location.origin`, so they cannot go stale.

```sh
# 1. edit base_url in metadata/site-metadata.json
# 2. edit og:url and the JSON-LD url in index.html
# 3. regenerate, and let the tests confirm nothing was missed
cargo build -p lotus-explore-rs
cargo test -p lotus-explore-rs --test buildrs
```

**The tests are the safety net, so read a failure rather than skipping it.**

- `index_html_agrees_with_base_url` fails if `index.html` and `base_url`
  disagree.
- `only_the_documented_files_name_the_live_host` walks the whole app and fails
  if any file names the live host without being generated or being one of the
  four documented exceptions above. This is what keeps the table honest: a new
  hardcoded copy is a build failure rather than something the next CNAME change
  silently misses, which is how the original two-field drift shipped.
- `every_host_bearing_artefact_follows_base_url` re-derives all seven generated
  files from three different `base_url` values, so a generator that stops
  following the source is caught without touching the real config.

There is no separate step for these: the `buildrs` target is declared in
`apps/lotus-explore-rs/Cargo.toml` and runs under `just test` and CI, because
`cargo test` does not otherwise compile `build.rs` as a test target and tests
written there are silently never run (verified: one reports "0 passed" under
`cargo test --all-targets`).

## `llms.txt` fails the Lighthouse audit, and the file is fine

Lighthouse reports "llms.txt does not follow recommendations -- The llms.txt
file should be a Markdown file containing at least one H1 header", unscored. The
description is generic; the actual failure is that **Lighthouse fetches
`/llms.txt` at the origin root, and this app is served from a subpath.**

  | Request                                             | Result                       |
  | --------------------------------------------------- | ---------------------------- |
  | `https://lotus.nprod.net/llms.txt`                  | **404** `text/html`, 14390 B |
  | `https://lotus.nprod.net/lotus-explore-rs/llms.txt` | 200 `text/plain`, 3501 B     |

The origin root is the **LOTUS home site**, a different deployment that this
repository does not publish to, so the 404 body is that site's HTML. The file
this repo generates is served correctly and does have an H1 on line 1, a
blockquote summary, H2 sections and absolute Markdown links.

The audit is marked **not applicable**, not failed, which is Chrome's documented
behaviour for a 404, and `agentic-browsing` scores 100 with it in that state.
Nothing is being penalised; the "should contain an H1" wording is the audit's
boilerplate description, not a finding about this file.

There are two ways an audit can find the file, and the second is now correct:

1. **`/llms.txt` at the origin** --- needs a file at the root of the CNAME,
   which is the other site's repository. Not reachable from here.
2. **A `Link` header advertising it** --- emitted by `_headers`, which GitHub
   Pages ignores, so nothing is sent on the live host. Measured: no `Link`
   header at all. These targets used to be root-relative, which resolved to the
   domain root and 404'd on the subpath deploy; they are now absolute from
   `base_url`, so they are correct on any host that honours `_headers`, and all
   seven are verified 200 against the live host.

## Plain HTTP is served, not redirected

Lighthouse (and several security scanners) report "Redirects HTTP traffic to
HTTPS". The finding is correct and it is **not fixable from this repository**.

Measured on the live host:

  | Request                                     | Result                     |
  | ------------------------------------------- | -------------------------- |
  | `http://lotus.nprod.net/lotus-explore-rs/`  | **200 OK** over plain HTTP |
  | `https://lotus.nprod.net/lotus-explore-rs/` | 200                        |

`lotus.nprod.net` is a CNAME to `lotusnprod.github.io`, so the site is served by
GitHub Pages, and GitHub Pages does not implement `_redirects`. Two independent
measurements confirm the file is inert there:

  | Probe                                                        | Expected if honoured | Actual  |
  | ------------------------------------------------------------ | -------------------- | ------- |
  | `/lotus-explore-rs/no-such-route` (the `/* 200` SPA rewrite) | 200                  | **404** |
  | `Strict-Transport-Security` from `_headers`                  | present              | absent  |

The redirect is a repository setting, not a file: **Settings → Pages → Enforce
HTTPS**. It is off, which is why HTTP is answered with `200` instead of `301`.
Note that GitHub Pages also does not send the HSTS header `_headers` asks for,
so even once HTTPS is enforced the preload directive has to come from somewhere
else; the `max-age=63072000` in `_headers` is intent for a future CDN.

For a host that does honour these files, `_redirects` now carries a real
host-level rule ahead of the SPA rewrite:

```
http://lotus.nprod.net/*  https://lotus.nprod.net/:splat  301!
```

Moving to Cloudflare Pages or Netlify activates that, and `_headers`, unchanged.

## No `Cross-Origin-Opener-Policy` is delivered

Scanners also report "Ensure proper origin isolation with COOP", unscored. Same
root cause as the redirect above, and same conclusion: not fixable from this
repository. Measured on the live host, **no `cross-origin-*` header is sent at
all** --- not `Cross-Origin-Opener-Policy`, not `Cross-Origin-Embedder-Policy`,
not `Cross-Origin-Resource-Policy` --- even though `_headers` requests all
three. GitHub Pages ignores the file.

What is worth recording is that turning COOP on is **safe for this app**, since
that is the usual reason it stays off. The three things that break under
`same-origin` were each checked:

  | Risk under COOP                                                                | This app                                                                                                                                            |
  | ------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- |
  | A popup that writes back through `window.opener`                               | No read of `window.opener` anywhere. The two `open_with_url*` call sites are download triggers, where severing the opener is the desired behaviour. |
  | `target="_blank"` links losing their opener                                    | Every one already carries `rel="noopener noreferrer"`, so the protection COOP adds is already in place per-link.                                    |
  | COOP+COEP together setting `crossOriginIsolated`, which some apps then require | No `SharedArrayBuffer`, no `Atomics`, no `crossOriginIsolated` check. The wasm module needs neither.                                                |

The Ketcher iframe at `assets/ketcher/index.html` is same-origin, so it is
unaffected by COEP `credentialless`, and COOP does not gate iframes at all.

So `same-origin` plus `credentialless` is available as pure hardening at no
functional cost. Like the HTTPS redirect, it needs a host that honours the
headers --- on the present one, that means moving off GitHub Pages or fronting
it with a CDN that sets them.

## `_headers` is not honoured on the production path

`apps/lotus-explore-rs/build.rs` generates `public/_headers` with a full policy:
HSTS, `Content-Security-Policy`, `X-Frame-Options`, `Referrer-Policy`,
`Permissions-Policy`, plus the corrected cache rules. That file is Cloudflare
Pages / Netlify syntax. GitHub Pages has no equivalent and ignores it.

Measured on the live host:

  | Header                      | In `_headers`  | Actually sent               |
  | --------------------------- | -------------- | --------------------------- |
  | `Content-Security-Policy`   | yes            | absent                      |
  | `X-Frame-Options`           | yes            | absent                      |
  | `Strict-Transport-Security` | yes            | absent                      |
  | `Cache-Control`             | per-path rules | `max-age=600` on everything |

So the security headers and the cache policy in `_headers` are documentation of
intent for a future CDN, not a description of the current host. `_headers` is
kept because it is correct and costs nothing, and because the Ketcher iframe
rule in it is the only place the `frame-ancestors` conflict is resolved in
writing --- but do not read it as a claim about production.

The cache rule is the one that costs performance rather than just posture.
Lighthouse reports it as `cache-insight` ("Use efficient cache lifetimes"), and
it is the largest remaining host-level win: the content-hashed glue and the 1.4
MB module are immutable by construction, yet every returning visitor revalidates
both after 600 s.

  | Asset                                       | `_headers` asks for           | Actually sent |
  | ------------------------------------------- | ----------------------------- | ------------- |
  | `assets/lotus-explore-rs-dxh<hash>.js`      | `max-age=31536000, immutable` | `max-age=600` |
  | `assets/lotus-explore-rs_bg-dxh<hash>.wasm` | `max-age=31536000, immutable` | `max-age=600` |

The rules are already written and correct; they activate on a CDN host. Until
then this is a deployment decision (move off GitHub Pages or front it), not
something a change to this repository can fix.

## `.br` files are uploaded and never served

`pre_compress = true` in `Dioxus.toml` makes `dx` emit a `.br` sibling for
everything under `assets/`. Those siblings are uploaded and are individually
fetchable:

  | Request                       | Result                                  |
  | ----------------------------- | --------------------------------------- |
  | `assets/lotus-explore.css`    | 200, `content-encoding: gzip`, 9852 B   |
  | `assets/lotus-explore.css.br` | 200, `application/octet-stream`, 8360 B |

The host gzips on the fly and never negotiates the precompressed brotli. The
consequence is measurable on the critical path:

  | Encoding | wasm transfer |
  | -------- | ------------- |
  | brotli   | 456 KiB       |
  | gzip     | 585 KiB       |

129 KiB, worth roughly 0.63 s at mobile throttling --- a larger LCP lever than
every optimisation in [`PERFORMANCE.md`](PERFORMANCE.md) combined. The `.br`
files remain in the artifact because they are the right thing to ship for any
host that does negotiate them.

## What GitHub Pages actually honours

Only three things, and none of them are headers:

- **`filename` hashing** --- trivially, because it is just static file serving.
  All correctness therefore rests on hashed bundle names; nothing depends on a
  `Cache-Control` the host will not send.
- **HTTPS**, and a custom domain.
- **`.nojekyll`**, if added, to skip Jekyll processing.

There is no response-header mechanism at all. The one lever that does work is
reducing what has to be transferred, which is why the release profile work went
where it did.

## Practical consequences

1. `max-age=600` on everything means users revalidate the whole bundle roughly
   every ten minutes. Since the bundle names are content-hashed, a revalidation
   returns 304 and costs almost nothing; this is why the hashed-asset strategy
   matters more on Pages than the header policy does.
2. Nothing may depend on a long-lived immutable cache for correctness. The
   unhashed assets --- `assets/lotus-explore.css` and `assets/js/*` --- would be
   pinned stale for an hour on a host that honoured the 1 hour rule. They are
   the only assets that would suffer, and an hour of staleness is a CSS refresh,
   not a broken app.
3. Moving to Cloudflare Pages or Netlify would activate `_headers` unchanged,
   turning brotli negotiation and the security headers on with no code change.
   That is the cheapest available win on this page and it is a hosting decision,
   not an engineering one.

## Local measurement

The Dioxus dev server serves the bundle without the `.br` siblings and without
`_headers`, so Lighthouse against it overstates transfer and understates cache
behaviour. To measure what a real host does, serve
`target/dx/lotus-explore-rs/release/web/public` with a static server that serves
the `.br` sibling when the request carries `Accept-Encoding: br` **and sends an
ETag or `Last-Modified`** --- without a validator, Chrome re-fetches the same
URL for every reference and reports a phantom cost. See `PERFORMANCE.md` for the
size this caught.
