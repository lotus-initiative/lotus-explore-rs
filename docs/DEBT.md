# Technical Debt & Ratchet Ledger

Every unavoidable lint suppression, duplicate-dependency skip, echoes/coverage ratchet, or mutation exclusion in this repository is recorded here with its reason, owner, and removal condition.

## Active Exceptions

| Location | Gate / Lint | Reason | Owner | Removal Condition |
|---|---|---|---|---|
| `Cargo.toml` (`[workspace.lints.clippy]`) | `multiple_crate_versions = "allow"` | Transitive dependency splits across Dioxus web and native HTTP/TLS subsystems; audited and gated by `deny.toml` `[bans] skip`. | `@Adafede` | Upstream `dioxus` / crate version consolidation. |
| `apps/lotus-explore-rs/src/main.rs` | `clippy::empty_enums` (crate-level allow) | Clippy 1.100's new nursery lint fires inside Dioxus's `Props` derive, once per `#[component]`; the enum is generated, not written here. | `@Adafede` | Drop when `dioxus` stops emitting an empty enum, or clippy skips derive output. |
| `apps/lotus-explore-rs/src/app/routes.rs` (`mod route_table`) | `unreachable_code` | Since Rust 1.100 the impls the `Routable` derive generates trip the lint. The module holds only `Route` so the allow covers nothing else. | `@Adafede` | Drop when a `dioxus-router` release no longer triggers it. |
| `deny.toml` (`[bans] skip`) | 11 skipped duplicate crate names (`block-buffer`, `const-serialize`, `const-serialize-macro`, `cpufeatures`, `crypto-common`, `digest`, `getrandom`, `rustc-hash`, `sha2`, `syn`, `wasm-streams`) | Version splits between `dioxus-web` / `manganis` 0.7.10 and native/shared dependencies (`sha2` 0.11, `reqwest` 0.13, `syn` 3). | `@Adafede` | Re-audit whenever `Cargo.lock` changes; `cargo deny` flags unnecessary skips. |
| `.cargo/mutants.toml` (`exclude_globs`) | 2 path globs (`apps/lotus-explore-rs/**`, `crates/lotus-search/src/**/wasm.rs`) | `apps/lotus-explore-rs` has ~2,700 Dioxus UI rendering mutants (covered opt-in via `./mk local:mutants-ui`); `wasm.rs` modules are `#[cfg(target_arch = "wasm32")]` and unobservable in host test runs. | `@Adafede` | Keep UI mutants in `./mk local:mutants-ui`; keep `wasm.rs` minimal. |
| `.cargo/mutants.toml` (`exclude_re`) | 4 regex exclusions (`DomainError::memory_limit`, `MemoryLimit`, `replace < with <= in preferred_over`, `replace build -> Result<reqwest::Client, String> with Ok`) | Two `wasm32`-only `MemoryLimit` arms, one provably equivalent `<` vs `<=` tie-break in `preferred_over` (`lotus-explore-core/src/export/archive.rs`), and `reqwest::Client::build` timeout/keepalive configuration. | `@Adafede` | Permanent (equivalent mutant and target/timeout boundaries). |
| `.refquiem.toml` (`allow`) | `allow = ["contentUrl"]` | Schema.org `"contentUrl"` for the live Wikidata SPARQL endpoint in `dataset_jsonld` (`crates/lotus-jsonld/src/dataset.rs`), unrelated to the removed field on offline `metadata::DataDownload` exports. Configured in `.refquiem.toml` rather than inline because `--base` branch scans reject inline markers for pre-base deletions as unused. | `@Adafede` | Keep while `dataset_jsonld` distributes a live endpoint URL. |

## Ratchet Floors (`mk.local`)

- `COVERAGE_FAIL_UNDER_LINES="58"`
- `ECHOES_MAX_APPROXIMATE=0`
- `ECHOES_MAX_REUSE=0`
- `Cargo.toml` lint `allow` count: `1`
- `MIRI_PACKAGES=(lotus-model lotus-query lotus-jsonld)`
- `MUTANTS_PACKAGES=(lotus-model lotus-search lotus-query lotus-curation lotus-jsonld lotus-cli lotus-web-assets lotus-explore-core)`
