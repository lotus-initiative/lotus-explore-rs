# lotus-explore-rs

[![AGPL-3.0
license](https://img.shields.io/badge/License-AGPL%203.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0.html)
[![CI](https://github.com/lotusnprod/lotus-explore-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/lotusnprod/lotus-explore-rs/actions/workflows/ci.yml)

`lotus-explore-rs` --- LOTUS Explorer.

A linked open data (LOD) explorer for the LOTUS compound-taxon-reference
knowledge graph from Wikidata, queried via SPARQL. Built on the `lotus-*` crates
under `crates/`, which the CLI and the tests share, and the QLever SPARQL
endpoint.

## Quick start

From the app directory:

```bash
cargo run -p lotus-web-assets --bin fetch-assets
dx serve --platform web --package lotus-explore-rs --locked
```

To also run the optional API:

```bash
cargo run --locked --features server -p lotus-explore-rs
```

Then open `http://localhost:8080/?api_base=http://127.0.0.1:8787`.

Without the server, the explorer falls back to direct QLever/SPARQL queries.

## Running the full stack (API + WASM client) in Docker

The included [`Dockerfile`](../../Dockerfile) builds both the API server and the
WASM web bundle (including Ketcher) in a multi-stage build. The runtime image
serves static files via the built-in `ServeDir` fallback.

```bash
# Build the image
docker build -t lotus-explore-rs .

# Run (serves API on :8787 and static files at /app/public)
docker run -p 8080:8787 lotus-explore-rs

# Open http://localhost:8080
```

Environment variables:

  | Variable               | Default       | Description                                                          |
  | ---------------------- | ------------- | -------------------------------------------------------------------- |
  | `HOST`                 | `0.0.0.0`     | Bind address                                                         |
  | `PORT`                 | `8787`        | Listen port                                                          |
  | `PUBLIC_DIR`           | `/app/public` | Static files directory                                               |
  | `LOTUS_API_BASE`       | *(none)*      | Upstream SPARQL/API base URL                                         |
  | `APP_ENV`              | `development` | Use `production` with `CORS_ALLOWED_ORIGINS`                         |
  | `CORS_ALLOWED_ORIGINS` | *(none)*      | Comma-separated allowed origins (required when `APP_ENV=production`) |

## Architecture

See [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) for the full architectural
overview, and [`../../docs/FRONTENDS.md`](../../docs/FRONTENDS.md) for how the
web, desktop and CLI front ends differ from each other.

### Running as a desktop app

```bash
./mk web-dev-desktop   # from the repo root
```

The desktop build reuses these components unchanged; what differs is that `dx`
embeds only the assets that Rust names in an `asset!` call, not the whole
`public/` tree. Anything the app loads at runtime by composing its own URL ---
RDKit, Ketcher --- is therefore absent from the bundle. See
[`docs/FRONTENDS.md`](../../docs/FRONTENDS.md) for the details and the known
consequences.

## Development testing

```bash
./mk test   # nextest for every build, then the doctests
```

For production-sized local performance and Lighthouse checks, build the release
bundle first and serve that directory with any static file server:

```bash
cd apps/lotus-explore-rs
cargo run -p lotus-web-assets --bin fetch-assets
dx build --release --platform web --package lotus-explore-rs --locked \
  --debug-symbols=false --rustc-args=-Copt-level=z
cd ../..
cargo run -p lotus-web-assets --bin inject-wasm-preload
```

That writes the bundle to `target/dx/lotus-explore-rs/release/web/public`, which
is what the deploy publishes: 1.6 MiB raw / 527 KiB brotli, against the dev
server's 6.4 MiB. `./mk web-bytes` prints the current figure and refuses to run
against a stale bundle. `dx serve` intentionally serves the debug WASM bundle for
hot reload, which is why the two are not interchangeable for measurement.

## Setup: external assets

`fetch-assets` downloads ~115 MB of Ketcher, RDKit and the Scholia Citation.js
bundle into `public/assets/vendor`. It must run before serving or deploying:

```bash
cd apps/lotus-explore-rs   # from repo root
cargo run -p lotus-web-assets --bin fetch-assets
```

It defaults to the latest Ketcher and RDKit releases and Scholia's `main`
branch; override with `KETCHER_VERSION`, `RDKIT_VERSION` and `CITATION_JS_REF`
for a reproducible or mirror-based build. The curation bridges load the files on
demand. The static `index.html` owns early metadata, CSS and bootstrap
discovery; `document_head::CurationScripts` adds the route-specific bridges.

`dx serve` and `dx build` generate and watch Tailwind automatically, so Node.js
and npm are not required.

## Citation

- Paper (DOI): <https://doi.org/10.7554/eLife.70780>
- BibTeX: [`public/docs/references.bib`](./public/docs/references.bib)

## Site metadata

`public/llms.txt`, `public/humans.txt`, `public/robots.txt`,
`public/.well-known/security.txt`, `public/_headers`, and
`public/site.webmanifest` are generated from
[`metadata/site-metadata.json`](./metadata/site-metadata.json).

## Explorer ⇄ API integration

  | Scenario                | `api_base` source                     | API used            |
  | ----------------------- | ------------------------------------- | ------------------- |
  | Codeberg Pages (public) | none                                  | ✗ direct SPARQL     |
  | Local dev               | auto-detected `http://127.0.0.1:8787` | ✓ if server running |
  | Build-time              | `LOTUS_API_BASE` env var              | ✓                   |
  | Runtime override        | `?api_base=…` query param             | ✓                   |

`api_base` must identify the LOTUS REST API; QLever/Wikidata SPARQL URLs are
ignored and the explorer uses its direct SPARQL path instead.

## URL automation

The client uses typed routes:

- `/` --- Welcome landing page
- `/search` --- Explore
- `/curation` --- Curation
- `/draw` --- Structure editor

URL-driven execution and exports:

- `?execute=true` --- run query on load
- `?download=true&format=csv` --- download CSV
- `?download=true&format=json` --- download SPARQL Results JSON
- `?download=true&format=rdf` --- download RDF (Turtle)

When both `download` and `execute` are present, `download` takes priority.

## Archive

A frozen version is archived on Zenodo: <https://doi.org/10.5281/zenodo.5794106>

## License

`AGPL-3.0-only` --- see [`LICENSE`](https://www.gnu.org/licenses/agpl-3.0.html)
for details.
