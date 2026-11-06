# Technical Debt & Ratchet Ledger

Every unavoidable lint suppression, duplicate-dependency skip, echoes/coverage ratchet, or mutation exclusion in this repository is recorded here with its reason, owner, and removal condition.

## Active Exceptions

| Location | Gate / Lint | Reason | Owner | Removal Condition |
|---|---|---|---|---|
| `Cargo.toml` (`[workspace.lints.clippy]`) | `multiple_crate_versions = "allow"` | Transitive dependency splits across Dioxus web/desktop/mobile subsystems; audited and gated by `deny.toml` `[bans] skip`. | `@Adafede` | Upstream `dioxus` / WebKit crate version consolidation. |
| `deny.toml` (`[advisories] ignore`) | 3 unmaintained notices (`RUSTSEC-2024-0370` `proc-macro-error`, `RUSTSEC-2024-0436` `paste`, `RUSTSEC-2025-0057` `fxhash`) | Transitive build-time helpers with no security vulnerability and no upstream upgrade path yet. | `@Adafede` | Drop when transitive dependents migrate off `proc-macro-error`, `paste`, and `fxhash`. |
| `deny.toml` (`[bans] skip`) | 52 skipped duplicate crate names (`base64`, `bitflags`, `const-serialize`, `getrandom`, `rand`, `syn`, `thiserror`, `wasm-streams`, `windows-*`, etc.) | Version splits between `dioxus-web` 0.7.10, `dioxus-desktop` (WebKit/tao/image stacks), and native HTTP/TLS crates (`reqwest` 0.13). | `@Adafede` | Re-audit whenever `Cargo.lock` changes; `cargo deny` flags unnecessary skips. |
| `.cargo/mutants.toml` (`exclude_globs`) | 2 path globs (`apps/lotus-explore-rs/**`, `crates/lotus-search/src/**/wasm.rs`) | `apps/lotus-explore-rs` has ~2,700 Dioxus UI rendering mutants (covered opt-in via `./mk local:mutants-ui`); `wasm.rs` modules are `#[cfg(target_arch = "wasm32")]` and unobservable in host test runs. | `@Adafede` | Keep UI mutants in `./mk local:mutants-ui`; keep `wasm.rs` minimal. |
| `.cargo/mutants.toml` (`exclude_re`) | 4 regex exclusions (`DomainError::memory_limit`, `MemoryLimit`, `replace < with <= in preferred_over`, `replace build -> Result<reqwest::Client, String> with Ok`) | Two `wasm32`-only `MemoryLimit` arms, one provably equivalent `<` vs `<=` tie-break in `preferred_over` (`lotus-explore-core/src/export/archive.rs`), and `reqwest::Client::build` timeout/keepalive configuration. | `@Adafede` | Permanent (equivalent mutant and target/timeout boundaries). |

## Ratchet Floors (`mk.local`)

- `COVERAGE_FAIL_UNDER_LINES="58"`
- `ECHOES_MAX_APPROXIMATE=0`
- `ECHOES_MAX_REUSE=0`
- `Cargo.toml` lint `allow` count: `1`
- `MIRI_PACKAGES=(lotus-model lotus-query lotus-jsonld)`
- `MUTANTS_PACKAGES=(lotus-model lotus-search lotus-query lotus-curation lotus-jsonld lotus-cli lotus-web-assets lotus-explore-core)`
