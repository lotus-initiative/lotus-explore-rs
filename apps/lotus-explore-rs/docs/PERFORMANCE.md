# LOTUS performance

Every number here was measured, not estimated. The recipe that produces the byte
table is `just web-bytes`; the Lighthouse figures come from a static server that
negotiates the precompressed `.br` siblings, because `just preview` serves
neither the `.br` files nor `_headers` and therefore overstates every transfer.

## Where the time goes

On `/` at mobile throttling, the LCP element is the welcome paragraph.
Lighthouse attributes 2.6 ms to time-to-first-byte and 146 ms to element render
delay, so the element itself is cheap. The remaining \~3.4 s sits between first
paint (751 ms) and the app mounting: the module crossing the link, then
compiling.

LCP 3752 ms = FCP 751 ms + 456 KiB over a simulated 1.6 Mbit/s link + compile

That is the whole story. LCP here is a payload-and-compile problem, and no
amount of DOM or CSS work touches it. The main thread is not the constraint:
total scripting is \~430 ms and total blocking time is 20--45 ms.

## What shipped

  | Change               | wasm raw | wasm br | wasm gzip | transfer |
  | -------------------- | -------- | ------- | --------- | -------- |
  | `codegen-units = 16` | 1533738  | 485247  | 627861    | 518.4 KB |
  | `codegen-units = 1`  | 1468720  | 467094  | 599227    | 499.5 KB |
  |                      | −4.24 %  | −3.74 % | −4.56 %   | −3.6 %   |

Cost: `dx build` goes from 103 s to 128 s.

Lighthouse, mobile and desktop, light and dark, 3 rounds each, median:

  | Metric       | `cgu=16` | `cgu=1`  |
  | ------------ | -------- | -------- |
  | perf mobile  | 89       | 89       |
  | perf desktop | 100      | 100      |
  | LCP mobile   | 3752 ms  | 3752 ms  |
  | LCP desktop  | 722 ms   | 722 ms   |
  | TBT mobile   | 45/54 ms | 42/21 ms |

The performance score does not move, and this document does not pretend
otherwise. 3.7 % off the payload is \~80 ms of transfer, which is inside the
run-to-run spread of a metric whose dominant term is a 2.3 s download.

## What was tried and rejected

Each of these was built and measured rather than reasoned about. Sizes are
`just web-bytes` output for a release `dx build`.

  | Experiment                        | raw     | br     | gz     | verdict                  |
  | --------------------------------- | ------- | ------ | ------ | ------------------------ |
  | baseline: `opt-level=z`, `cgu=16` | 1529038 | 484149 | 624171 | reference                |
  | `opt-level=s`, `cgu=16`           | 1684551 | 504455 | 665153 | **worse**, +4.2 % brotli |
  | `opt-level=z`, `cgu=1`            | 1463410 | 465858 | 597064 | **kept**                 |
  | `+ panic = "abort"`               | 1465621 | 466744 | 598085 | **no-op**, +13 B raw     |
  | `dioxus/devtools` off             | 1463423 | 465709 | 597095 | **no-op**, ±0.03 %       |

All of the above were taken with a bare `dx build`. `just build` --- the path
the Dockerfile runs --- was compiling at `opt-level=s` for part of this work, so
its outputs came out 185300 raw bytes larger. The `opt-level=z` row is the one
that ships, and `just opt-levels` now keeps the three declarations in agreement.

### `opt-level = "s"` is not the counter-intuitive win it is claimed to be

The expectation is that `s` lets LLVM inline and vectorise before
`wasm-opt -Oz`, producing output that is both smaller and faster. On this
workspace it produces a binary 10.2 % larger raw and 4.2 % larger brotli. Since
LCP is transfer-bound and the last 4 % of transfer is worth \~80 ms, the larger
binary is simply worse on the only axis that matters. `opt-level = "z"` stays,
in `Cargo.toml`, in `Dioxus.toml`'s `wasm_opt.level`, and in the `--rustc-args`
of every `just` recipe, so all three entry points agree.

### `panic = "abort"` cannot shrink this module

`wasm32-unknown-unknown` already defaults to `panic="abort"`:

```
$ rustc --print cfg --target wasm32-unknown-unknown | grep panic
panic="abort"
```

So the setting is a no-op for the shipped artifact --- measured at +13 bytes of
raw over the identical build without it, which is build noise, not a saving. It
would also change the native build, where unwinding is still wanted by the test
suite. Nothing was gained, so nothing was taken. The trade-off the setting
implies --- a panic aborts the module instead of unwinding, so
`components/loading` and the retry paths must never expect to catch one --- is
already the shipped behaviour and was not worth re-deciding.

### `dioxus/devtools` is not in the shipped wasm to begin with

This looked like the most promising payload lever left. It is not, and the
reason is in `dioxus-web`:

```
#[cfg(all(feature = "devtools", debug_assertions))]
mod devtools;
```

The runtime hook is gated on `debug_assertions`, which a release build sets to
false. The `devtools` *feature* only adds `dioxus-core/serialize` (serde derives
that are never monomorphised, because nothing serialises a `VirtualDom`) and two
unused `web-sys` bindings. Confirmed three ways on a build with the feature off:

- 0 occurrences of `devtool`, `WebSocket`, `MessageEvent`, `subsecond` or
  `hot.reload` in the linked binary's strings;
- no `WebSocket` or `MessageEvent` in the wasm import section;
- 13 bytes of raw difference against the build with the feature on.

`dioxus-devtools` is a non-optional dependency of `dioxus-web`, so the crate is
in the graph either way; only the runtime is gated, and the runtime is already
off in release. Gating it behind a Cargo feature would have added a feature, a
`--features devtools` in the `serve` recipe, and a way for a release build to
accidentally enable it --- all to buy 0.03 % of the brotli payload. It was not
taken.

## The real remaining lever is the host, not the code

The module transfers as 456 KiB when the host serves the precompressed `.br`,
and as 585 KiB of on-the-fly gzip when it does not. That 129 KiB is worth
roughly 0.63 s at mobile throttling --- larger than everything on this page
combined. See [`DEPLOYMENT.md`](DEPLOYMENT.md) for what the production host
actually does.

Below that, the honest next step is code-splitting or trimming `lotus`, which is
a project rather than a tweak. There is no further profile dial: `opt-level` is
at `z`, LTO is fat, codegen units are 1, the panic strategy is already abort,
and the dev-only crates are already out of the wasm graph (verified, not assumed ---
`axum`, `utoipa`, `utoipa-swagger-ui`, `tokio`, `flat2`, `tempfile`,
`tower-http`, `clap` and `env_logger` are all absent, and no TLS stack is
reachable: `reqwest` on wasm uses the browser `fetch`).

## Taking the module off the critical path

`dx` preloads the JS glue but not the module, and the glue cannot fetch the
module until it has itself been downloaded, parsed and executed. The largest
response on the page was therefore waiting on a 45 KiB one:

```text
html -> glue js (45 KiB) -> fetch(module 1.4 MiB) -> instantiate -> mount
```

A `<link rel="preload" as="fetch" type="application/wasm" crossorigin>` in the
built `index.html` takes it off that chain. `crossorigin` is not optional:
`instantiateStreaming` fetches in CORS mode, and without it the preload sits in
a different cache slot and the module is fetched twice.

Measured from the real request timings, Lighthouse `devtools` throttling at 1.6
Mbit/s / 150 ms:

  |                         | before                       | after                        |
  | ----------------------- | ---------------------------- | ---------------------------- |
  | module request starts   | 520252 ms after the document | 220055 ms after the document |
  | observed LCP            | 3114 ms                      | 2940 ms                      |
  | simulated LCP, 5 rounds | 3752 ms                      | 3606 ms light / 3624 ms dark |
  | perf mobile, 5 rounds   | 89                           | 90 light / 90 dark           |
  | TBT mobile              | 37 / 17 ms                   | 11 / 0 ms                    |

The module now starts in the same \~9 ms window as the CSS and the glue instead
of 300 s behind them. This is the only LCP win in this document that came from
request scheduling rather than from making the module smaller.

Because the module name is content-hashed, this cannot be written by hand in the
source `index.html`; `inject-wasm-preload` (a second `lotus-web-assets` bin, run
by both `just build` and the Dockerfile) injects it after the bundle is emitted.
It copies the asset prefix out of the preload `dx` already wrote, so a
`--base-path` build works without the tool knowing about base paths. It reads
the module name out of the glue rather than listing `assets/`, because `dx`
leaves a superseded module behind on a hash change and guessing wrong costs a
wasted 1.4 MiB fetch.

## Three copies of one setting, and nothing comparing them

The optimisation level is declared in three places: `[profile.release]`
`opt-level` in `Cargo.toml`, `[web.wasm_opt].level` in `Dioxus.toml`, and
`--rustc-args=-Copt-level=` on every `dx` invocation in the justfile. `dx`
appends `--rustc-args` last, so the justfile wins and the profile is decorative.

They drifted: the justfile said `s`, the profile said `z`, and every
`just build` shipped a module 185300 raw / 27843 brotli bytes larger than
intended. CI stayed green, because each file was individually valid and nothing
compared them. `just opt-levels` now runs in `ci` and fails on any disagreement.

The cost was not only the bytes. A bare `dx build` and a `just build` were
compiling at different levels from identical source, so they produced different
binaries and every comparison between them was meaningless until this was found.
Measure through the shipping path, and not by invoking `dx` by hand.

## The dev server is not a measurement target

`just serve` (and `dx serve` generally) reports an enormous payload, and it is
not a bug in the app. A Lighthouse run against it shows roughly 65 MB, almost
all of it one request to `/wasm/lotus-explore-rs_bg.wasm` --- the debug module,
served unhashed alongside the Dioxus JS interpreter snippets.

Measured on this workspace, dev wasm from identical source:

  | dev rustc args                    | dev wasm    | build   |
  | --------------------------------- | ----------- | ------- |
  | default                           | 63.6 MiB    | 42 s    |
  | `-Copt-level=1`                   | 60.1 MiB    | 14 s    |
  | `-Cdebuginfo=0`                   | 37.5 MiB    | 12 s    |
  | `-Cdebuginfo=0 -Cstrip=debuginfo` | **6.4 MiB** | **6 s** |

Optimising barely moves it; the debug sections are the entire cost, and writing
them is most of the build time. `just serve` now strips them, which is a 10x
smaller payload and a 7x faster dev build, and neither flag changes codegen so
hot reload is unaffected. What it costs is line numbers in panic backtraces.

The remaining gap between 6.4 MiB and the shipped 456 KiB brotli is the
difference between a debug server and a release build. Take numbers from
`just build` and a static server that negotiates the precompressed `.br`
siblings, never from `dx serve`.

## A measurement trap worth recording

A test server that sends `Cache-Control: no-cache` **without** a validator (ETag
or Last-Modified) makes Chrome re-fetch the same URL on every reference.
`favicon.svg`, declared twice in `site.webmanifest` plus once in the shortcut
list, appeared as four 13 KB requests instead of one, inflating
`total-byte-weight` by 39 KB and inventing a 7 % phantom regression. With an
ETag the repeats become 304s and the same page measures 499 KB.

Any harness that serves the bundle must send a validator, or it will report a
phantom cost. The other thing to know is that `build.rs` runs
`clean_dx_output()`, which deletes `target/dx/<app>/<profile>/web/public` on
*any* wasm compile of the app. `just ci` runs
`cargo check --target wasm32-unknown-unknown`, so the CI gate silently destroys
the last `dx` bundle. Always run `dx build` after `just ci` if you intend to
measure afterwards.

## Render path

Two per-render allocations were removed, both real and both invisible to a
`MutationObserver` because the wasted work produced byte-identical output:

- The curation queue card deep-copied every `CurationInputRow` on every render.
  The page subscribes to the TSV textarea's signal, so every keystroke paid for
  it. Now 7 N heap allocations per keystroke are gone, in favour of a refcount
  bump.
- The download buttons cloned a `SearchCriteria` --- three `String`s --- three
  times per toolbar render, because `onclick: { .. }` is evaluated while the
  template is built rather than on the click. Now taken on click.

Both are documented as allocation fixes, not speedups, because that is what the
measurements support: at 100 queued rows the deep copy costs \~42 µs against a
16,700 µs frame budget, and long-task time while typing was 0 ms before and
after.

## What is left in the module, and why nothing is left to tune

Profiled with `twiggy` against an unstripped build (`RUSTFLAGS=-Cstrip=none`),
then attributed by parsing the name section against the code section. The
shipped 1.4 MB module is 74.5 % code and 23.8 % data across \~6,500 functions.
Split by area:

  | Area                              | Bytes     | Share  |
  | --------------------------------- | --------- | ------ |
  | Curation page and its services    | 149,130   | 10.9 % |
  | Results table and viewport        | 116,070   | 8.5 %  |
  | Search panel and form             | 75,721    | 5.6 %  |
  | Draw page / Ketcher panel         | 267       | 0.0 %  |
  | Shared framework, std, i18n, rest | 1,026,281 | 75.3 % |

Three quarters of the module is the floor under the app: dioxus, web-sys,
reqwest, serde, std and the four locale tables. The largest single feature, the
whole curation page, is 10.9 %. Dropping it to shrink the bundle is a product
decision, not an optimisation, and the draw page is already almost free because
Ketcher loads as a same-origin iframe rather than into the module.

The tuning levers are exhausted, each measured rather than assumed:

- **`wasm-opt`** is already at the floor. `-Oz --converge` is what ships, and
  adding `--vacuum` and `--remove-unused-names` by hand changes the brotli size
  by 50 bytes out of 467,133, because `-Oz` already runs them.
- **No stray sections ship.** The unstripped build carries a 534 KB
  `__wasm_bindgen_unstable` section and a 921 KB name section; `wasm-opt`
  removes both, and the shipped module contains only `target_features` (157
  bytes).
- **`reqwest` is already minimal for wasm**: `default-features = false`, and no
  `hyper`, `native-tls` or `rustls` rlib is built for the target.
- **`panic = "abort"` and `dioxus/devtools`** were already measured as no-ops
  above; `wasm32-unknown-unknown` aborts by default and the devtools module is
  gated on `debug_assertions`.

So the remaining reduction is a product or architecture decision -- fewer
locales, dropping a route, or code-splitting the module, which would mean
multi-module dynamic loading that this build does not currently do -- not a knob
left to turn.

## Source maps

Emitting no source map is deliberate, so this audit is expected to fail.

The audit that reports this is `missing-source-maps` ("Large JavaScript file is
missing a source map"), which is **unscored** --- it cannot move the performance
score, and Lighthouse's own copy notes it is offered for the debugging insight
rather than as a metric. Nothing here is a regression to fix.

Every entry it currently lists is one the app cannot act on:

- `assets/lotus-explore-rs_bg-<hash>.wasm` and
  `assets/vendor/rdkit/RDKit_minimal.wasm` are **WebAssembly, not JavaScript**.
  They are listed because the audit inspects the script-type requests the
  bundler and the Dioxus glue make; there is no JavaScript to map back to.
  Shipping `--debug-symbols` DWARF would cost far more than a `.map` --- the 65
  MB debug module is exactly what the deploy guard exists to catch.
- `assets/vendor/citation-js/citation.js` is a vendored **third-party** minified
  bundle, pinned by commit in `fetch-assets`. Upstream publishes no map:
  `citation.js.map` is a 404 at the pinned Scholia ref, and neither does RDKit
  (`RDKit_minimal.js.map` is a 404 on unpkg). The only way to produce one would
  be to minify from source ourselves, which would mean vendoring a build
  toolchain to reconstruct someone else's bundle.
- The **first-party** JavaScript is the 45 KiB Dioxus glue
  (`assets/lotus-explore-rs-dxh<hash>.js`), which is under the size threshold
  the audit applies and is therefore never listed.

Our own glue emits no `sourceMappingURL` and has no map, so there is also no
dangling 404 to chase. A map for it would be \~1.4 MB against a 45 KiB payload ---
a real transfer cost, paid by anyone whose browser fetches it, in exchange for
an unscored audit. Debugging the release build is what `--debug-symbols` and the
local `just serve` profile are for.
