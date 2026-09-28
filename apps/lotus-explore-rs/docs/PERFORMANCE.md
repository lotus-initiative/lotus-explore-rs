# LOTUS performance

Every number here was measured, not estimated. The recipe that produces the byte
table is `just web-bytes`; the Lighthouse figures come from a static server that
negotiates the precompressed `.br` siblings, because `just preview` serves
neither the `.br` files nor `_headers` and therefore overstates every transfer.

## Where the time goes

On `/` at mobile throttling, the LCP element is the welcome paragraph.
Lighthouse attributes 2.6 ms to time-to-first-byte and 146 ms to element render
delay, so the element itself is cheap. The remaining ~3.4 s sits between first
paint (751 ms) and the app mounting: the module crossing the link, then
compiling.

LCP 3752 ms = FCP 751 ms + 456 KiB over a simulated 1.6 Mbit/s link + compile

That is the whole story. LCP here is a payload-and-compile problem, and no
amount of DOM or CSS work touches it. The main thread is not the constraint:
total scripting is ~430 ms and total blocking time is 20--45 ms.

## What shipped

  | Change                     | wasm raw | wasm br  | wasm gzip | transfer |
  | -------------------------  | -------- | -------- | --------- | -------- |
  | `codegen-units = 16`       | 1533738  | 485247   | 627861    | 518.4 KB |
  | `codegen-units = 1`        | 1468720  | 467094   | 599227    | 499.5 KB |
  |                            | −4.24 %  | −3.74 %  | −4.56 %   | −3.6 %   |

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
otherwise. 3.7 % off the payload is ~80 ms of transfer, which is inside the
run-to-run spread of a metric whose dominant term is a 2.3 s download.

## What was tried and rejected

Each of these was built and measured rather than reasoned about. Sizes are
`just web-bytes` output for a release `dx build`.

  | Experiment                            | raw     | br      | gz      | verdict                        |
  | ------------------------------------- | ------- | ------- | ------- | -----------------------------  |
  | baseline: `opt-level=z`, `cgu=16`     | 1529038 | 484149  | 624171  | reference                      |
  | `opt-level=s`, `cgu=16`               | 1684551 | 504455  | 665153  | **worse**, +4.2 % brotli       |
  | `opt-level=z`, `cgu=1`                | 1463410 | 465858  | 597064  | **kept**                       |
  | `+ panic = "abort"`                   | 1465621 | 466744  | 598085  | **no-op**, +13 B raw           |
  | `dioxus/devtools` off                 | 1463423 | 465709  | 597095  | **no-op**, ±0.03 %             |

### `opt-level = "s"` is not the counter-intuitive win it is claimed to be

The expectation is that `s` lets LLVM inline and vectorise before
`wasm-opt -Oz`, producing output that is both smaller and faster. On this
workspace it produces a binary 10.2 % larger raw and 4.2 % larger brotli. Since
LCP is transfer-bound and the last 4 % of transfer is worth ~80 ms, the larger
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
measurements support: at 100 queued rows the deep copy costs ~42 µs against a
16,700 µs frame budget, and long-task time while typing was 0 ms before and
after.

## Source maps

`valid-source-maps` is 0 by design. No source map is emitted, so no
`sourceMappingURL` is emitted either, and there is no 404 to fix. This is
deliberate: a source map would be a second multi-megabyte artifact served to
nobody, in exchange for points on an audit that the app cannot act on.
