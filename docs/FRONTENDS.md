# The three front ends

LOTUS ships one core and three ways to reach it. They are not three
implementations of the search: `lotus-model`, `lotus-query`, `lotus-search`,
`lotus-curation` and `lotus-jsonld` hold all of the logic, and each front end is
a thin shell over them. This document is about what differs between them, and
why.

  | Front end | Crate                                      | Built by                                | Talks to                 |
  | ---       | ---                                        | ---                                     | ---                      |
  | Web       | `apps/lotus-explore-rs` (`dioxus/web`)     | `dx build --platform web`               | QLever / WDQS over HTTPS |
  | Desktop   | `apps/lotus-explore-rs` (`dioxus/desktop`) | `dx serve --desktop --features desktop` | QLever / WDQS over HTTPS |
  | CLI       | `crates/lotus-cli`                         | `cargo build`                           | QLever / WDQS over HTTPS |

All three hit the same public endpoints, so a query that works in one works in
all three. The endpoints are overridable with `LOTUS_QLEVER_ENDPOINT`,
`LOTUS_WDQS_ENDPOINT` and `LOTUS_WDQS_SCHOLARLY_ENDPOINT`.

## Shared rules

- **`lotus curate` never writes.** It is a read-only auditor: it looks up each
  entry in Wikidata, labels what is missing, and exits. There is no code path
  that submits a statement, and that is deliberate.
- **Endpoints are overridable**, so all three can be pointed at a local QLever.
- **Licence.** Everything here is AGPL-3.0-only, including the docs.

## Web

```bash
cd apps/lotus-explore-rs
cargo run -p lotus-web-assets --bin fetch-assets   # once, fetches RDKit/Ketcher
dx serve --platform web --package lotus-explore-rs --locked
```

Optional API alongside it:

```bash
cargo run --locked --features server -p lotus-explore-rs
# then open http://localhost:8080/?api_base=http://127.0.0.1:8787
```

Without the server the explorer queries QLever and WDQS directly from the
browser.

Two things are specific to the web build:

- **The document is static HTML.** `index.html` is checked in and owns the early
  metadata, the stylesheet link, and the bootstrap script. The stylesheet is
  linked at parse time, before the wasm module has booted, which is what stops
  the first paint from flashing an unstyled page.
- **Every file under `public/` is reachable.** A dev server, and the deployed
  site, serve the whole tree. The curation bridges therefore load RDKit by
  requesting `assets/vendor/rdkit/RDKit_minimal.js` and it simply resolves.

## Desktop

```bash
./mk web-dev-desktop  # from the repo root
# equivalently, from apps/lotus-explore-rs:
dx serve --package lotus-explore-rs --desktop --locked --features desktop
```

`fetch-assets` runs first, for the same reason it does on web.

The desktop build reuses the web components unchanged. There is no separate
desktop UI to keep in sync, and no feature that hides part of the app on native
for the sake of the window.

Note the flags: Dioxus supplies a different entry point per renderer and two of
them cannot be enabled at once, so the renderer is chosen by a feature. Without
`--features desktop` a native build has no user interface and exits immediately.

What differs is the document and the asset pipeline, and both bite:

- **Dioxus generates the document.** There is no `index.html` to read, so the
  stylesheet is attached from the component tree with
  `document::Stylesheet { href: asset!(...) }` in
  `document_head::AppStylesheet`.
- **Only `asset!`-referenced files are embedded.** This is the important one.
  `dx` copies into the bundle the files that Rust names in an `asset!` call, not
  the whole `public/` tree. `fetch-assets` writes RDKit and Ketcher into
  `public/assets/vendor`, and nothing in Rust references them, so they are
  absent from the bundle. The window has no CSS problem --- `AppStylesheet`
  fixed that --- but the curation and structure-editor pages depend on those
  files, and see them 404.

Two consequences that are easy to mistake for unrelated bugs:

- `document_head::asset_url` returns an empty string on native, so the Ketcher
  panel gets an empty iframe `src`.
- The RDKit bridge composes its own URL from `data-lotus-base-path`, which the
  desktop document never sets.

Both are the same underlying cause: runtime-composed asset URLs are invisible to
the bundler. The fix is to name the assets in Rust with `asset!` and hand the
resolved `bundled_path()` to the bridge, and for RDKit also pass a `locateFile`
override, because hashed filenames land side by side in `assets/` and
`RDKit_minimal.js` would otherwise look for `RDKit_minimal.wasm` next to itself
and not find it.

## CLI

```bash
cargo run -p lotus-cli -- search --help
cargo run -p lotus-cli -- curate --help
```

`lotus` is the same search and curation logic without a browser or a window. It
is the fastest way to check a query, and the only front end that is easy to put
in a pipeline.

- Output is plain text by default, with `--format` for JSON, CSV and TSV.
- `lotus curate` audits a `.json` file against Wikidata and reports per entry.
- `docs/cli.md` is the reference, and `crates/lotus-cli/tests/docs_in_sync.rs`
  fails if it drifts from `--help`.

## When a fix belongs in more than one place

Search and curation behaviour belongs in the shared crates, not in the front
ends. If a change makes the web app show a different result, the fix belongs in
`lotus-query` or `lotus-search` and all three front ends get it. The front ends
differ in transport, in how they reach a file, and in what they do with the
answer --- not in what the answer is.
