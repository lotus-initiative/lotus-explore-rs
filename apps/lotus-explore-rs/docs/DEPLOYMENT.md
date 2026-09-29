# Deployment

The site is published by `.github/workflows/deploy.yml` with
`actions/upload-pages-artifact` + `actions/deploy-pages`, and is served from
`https://lotus.nprod.net/lotus-explore-rs/`. Everything below was measured
against the live host with `curl`, not inferred from the configuration.

## Plain HTTP is served, not redirected

Lighthouse (and several security scanners) report "Redirects HTTP traffic to
HTTPS". The finding is correct and it is **not fixable from this repository**.

Measured on the live host:

  | Request                                       | Result                          |
  | --------------------------------------------- | ------------------------------- |
  | `http://lotus.nprod.net/lotus-explore-rs/`    | **200 OK** over plain HTTP      |
  | `https://lotus.nprod.net/lotus-explore-rs/`   | 200                             |

`lotus.nprod.net` is a CNAME to `lotusnprod.github.io`, so the site is served by
GitHub Pages, and GitHub Pages does not implement `_redirects`. Two independent
measurements confirm the file is inert there:

  | Probe                                                        | Expected if honoured | Actual |
  | ------------------------------------------------------------ | -------------------- | ------ |
  | `/lotus-explore-rs/no-such-route` (the `/* 200` SPA rewrite)  | 200                  | **404** |
  | `Strict-Transport-Security` from `_headers`                    | present              | absent |

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

## `_headers` is not honoured on the production path

`apps/lotus-explore-rs/build.rs` generates `public/_headers` with a full policy:
HSTS, `Content-Security-Policy`, `X-Frame-Options`, `Referrer-Policy`,
`Permissions-Policy`, plus the corrected cache rules. That file is Cloudflare
Pages / Netlify syntax. GitHub Pages has no equivalent and ignores it.

Measured on the live host:

  | Header | In `_headers` | Actually sent |
  | --------------------- | ------------- | ------------- |
  | `Content-Security-Policy` | yes | absent |
  | `X-Frame-Options` | yes | absent |
  | `Strict-Transport-Security` | yes | absent |
  | `Cache-Control` | per-path rules | `max-age=600` on everything |

So the security headers and the cache policy in `_headers` are documentation of
intent for a future CDN, not a description of the current host. `_headers` is
kept because it is correct and costs nothing, and because the Ketcher iframe
rule in it is the only place the `frame-ancestors` conflict is resolved in
writing --- but do not read it as a claim about production.

## `.br` files are uploaded and never served

`pre_compress = true` in `Dioxus.toml` makes `dx` emit a `.br` sibling for
everything under `assets/`. Those siblings are uploaded and are individually
fetchable:

  | Request                        | Result                                     |
  | ------------------------------ | -----------------------------------------  |
  | `assets/lotus-explore.css`     | 200, `content-encoding: gzip`, 9852 B      |
  | `assets/lotus-explore.css.br`  | 200, `application/octet-stream`, 8360 B    |

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

`just preview` serves the bundle without the `.br` siblings and without
`_headers`, so Lighthouse against it overstates transfer and understates cache
behaviour. To measure what a real host does, serve
`target/dx/lotus-explore-rs/release/web/public` with a static server that serves
the `.br` sibling when the request carries `Accept-Encoding: br` **and sends an
ETag or `Last-Modified`** --- without a validator, Chrome re-fetches the same
URL for every reference and reports a phantom cost. See `PERFORMANCE.md` for the
size this caught.
