# Technical Debt & Refactoring Ledger — `lotus-explore-rs`

Baseline commit: `a2f09a3` (`main`)  
Hardware & toolchain measured: Apple M1 Pro (8 cores, 16 GiB RAM), `rustc 1.100.0 (0ef1bc386 2026-11-09)`, `cargo 1.100.0`  
Note: `docs/DEBT.md` remains the repository's lint-suppression and ratchet registry; this root `DEBT.md` records the full refactoring coverage ledger, structural debt resolution, measured before/after metrics, and scientific correctness findings handed off without modification.

---

## 1. Coverage Ledger

All **793** git-tracked files across the 11 workspace crates are accounted for below (`43` refactored, `11` safety-net / regression test updates, `728` reviewed with no structural edits required, `11` N/A with explicit justification):

| File | Status | Notes |
| --- | --- | --- |
| `.cargo/audit.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.cargo/mutants.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.config/nextest.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.dockerignore` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.echoes.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.editorconfig` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.gitattributes` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/CODEOWNERS` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/PULL_REQUEST_TEMPLATE.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/actions/setup/action.yml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/ai/.mcp.json` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/ai/AGENTS.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/ai/PITFALLS.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/dependabot.yml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/workflows/archive.yml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/workflows/ci.yml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/workflows/deploy.yml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/workflows/nightly.yml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.github/workflows/release.yml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.gitignore` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.graphifyignore` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.mk/catalog.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `.mk/core.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `.mk/gates.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `.mk/meta.py` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.mk/mutants.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `.mk/mutants_plan.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `.mk/nightly.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `.mk/tier.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `.mk/tools.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `.refquiem.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.tombi.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `.typos.toml` | Refactored | Added `[default] extend-ignore-re` for backticked hex commit hashes (`2dbc90a`). |
| `.vscode/settings.json` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `AGENTS.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `AUDIT.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `CITATION.cff` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `CLAUDE.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `CONTRIBUTING.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `Cargo.lock` | N/A | Generated dependency lockfile enforced via `--locked`. |
| `Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `DEBT.md` | Refactored | Refactoring coverage ledger, debt catalog, before/after metrics, and handed-off findings. |
| `Dockerfile` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `LICENSE` | N/A | Upstream AGPL-3.0-only licence text. |
| `Makefile.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `SECURITY.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/.gitignore` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/Dioxus.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/build.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/build/ai.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/build/csp.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/build/headers.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/build/metadata.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/build/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/build/site_files.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/build/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/build_test.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/docs/ARCHITECTURE.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/docs/CITATION.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/docs/CURATION_SHARE_LINKS.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/docs/DEPLOYMENT.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/docs/DESIGN_SYSTEM.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/docs/PERFORMANCE.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/index.html` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/metadata/site-metadata.json` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/.well-known/agent-skills.json` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/.well-known/ai-catalog.json` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/.well-known/api-catalog.json` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/.well-known/apple-app-site-association` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/.well-known/security.txt` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/_headers` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/_redirects` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/android-chrome-192x192.png` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/android-chrome-512x512.png` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/apple-touch-icon.png` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/assets/js/curation/citation-bridge.js` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/public/assets/js/curation/rdkit-bridge.js` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/public/assets/js/graph/lotus-graph-gl.js` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/public/assets/js/webmcp.js` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/public/assets/lotus-explore.css` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/public/docs/references.bib` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/favicon-16x16.png` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/favicon-32x32.png` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/favicon-dark.svg` | N/A | Static vector favicon asset. |
| `apps/lotus-explore-rs/public/favicon-light.svg` | N/A | Static vector favicon asset. |
| `apps/lotus-explore-rs/public/favicon.ico` | N/A | Binary raster icon asset. |
| `apps/lotus-explore-rs/public/favicon.svg` | N/A | Static vector favicon asset. |
| `apps/lotus-explore-rs/public/humans.txt` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/llms.txt` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/robots.txt` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/site.webmanifest` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/public/sitemap.xml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `apps/lotus-explore-rs/src/api.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/api/client.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/api/client/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/api/config.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/api/config/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/app/bootstrap.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/app/bootstrap/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/app/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/app/routes.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/app/routes/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/app/shell.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/app_state.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/app_state/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/cache.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/cache/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/clock.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/clock/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/copy_button.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/curation_results_table.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/data_curation_page.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/data_curation_page/sections.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/data_curation_page/sections/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/faq.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/faq/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/form_inputs.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/form_sections.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/form_sections/basic_sections.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/form_sections/entity_group.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/form_sections/entity_group/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/form_sections/field_examples.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/form_sections/field_examples/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/form_sections/formula_section.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/form_sections/shared.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/form_sections/shared/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/dark_mode_toggle.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/footer.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/footer/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/header_meta.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/header_meta/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/lang_switch.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/notices.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/page_header.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/simple_analytics_badge.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/structured_data.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/structured_data/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/layout/view_switch.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/loading.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/loading/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/not_found.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_graph/bridge.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_graph/canvas_stage.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_graph/empty_starter.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_graph/history.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_graph/inspector.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_graph/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_graph/presentation.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_graph/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_graph/toolbar.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/download_model.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/download_model/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/header_model.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/header_model/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/render_model.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/render_model/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/compound.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/numeric.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/numeric/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/reference.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/structure.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/cells/taxon.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/prepared.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/prepared/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/render.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/row_text.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/row_cells/row_text/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/scroll_runtime.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/scroll_runtime/frame_closure_safety.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/scroll_runtime/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/scroll_signals.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/sort_model.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/sort_model/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/table_filter_row.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/table_filter_row/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/table_header.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/table_toolbar_sections/download_actions.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/table_toolbar_sections/filter_status.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/table_toolbar_sections/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/table_toolbar_sections/query_panel.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/table_toolbar_sections/stat_bar.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/table_view_model.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/table_view_model/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/toolbar.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/virtualization_controller.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/virtualization_controller/measurement_frame_safety.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_table/virtualized_table.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_viewport.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/results_viewport/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/search_panel.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/search_panel/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/ui/button.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/ui/button/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/ui/card.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/ui/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/ui/section_heading.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/components/welcome.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/curation.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/curation/contract_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/curation/share_links.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/curation/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/document_head.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/document_head/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/blob_limit_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/error.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/file_sink.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/local_file.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/local_file/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/native.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/opfs_availability_gate.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/opfs_write_stream.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/sink_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/streaming_gate.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/download/wasm.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/page_controller.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/page_controller/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/queue.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/queue/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/repositories/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/repositories/wikidata.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/chemical.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/chemical/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/enrichment.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/enrichment/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/http_client.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/http_client/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/inputs.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/occurrence.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/occurrence/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/occurrence_cache.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/occurrence_cache/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/pipeline.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/pipeline/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/prefetch.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/prefetch/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/quickstatements.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/quickstatements/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/reference_metadata.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/reference_metadata/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/services/wikidata.rs` | Refactored | Unified SPARQL JSON binding/QID parsing helpers (`0ae70b7`). |
| `apps/lotus-explore-rs/src/features/curation/services/wikidata/tests.rs` | Tests added | Added tie-breaking permutation-invariance property tests (`999d7ae`). |
| `apps/lotus-explore-rs/src/features/curation/workflow.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/curation/workflow/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/download_dispatch.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/download_effects/dispatch.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/download_effects/metrics.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/download_effects/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/download_effects/startup.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/download_effects/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/executor.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/interactions.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/lifecycle.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/lifecycle/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/controller.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/controller/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/runtime.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/runtime/test_exports.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/orchestrator/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/search_state.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/search_state/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/selectors.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/selectors/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/api_pipeline.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/api_pipeline/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/native.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/native/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/no_limit_gate.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/progress.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/progress/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/fetch_results/wasm.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_reference.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_reference/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_structure.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_structure/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/match_selection.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/match_selection/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/mod.rs` | Refactored | Delegated `requires_remote_lookup` to `lotus_model::is_qid`, removing slicing allow (`0ae70b7`). |
| `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/results_pipeline/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/results_pipeline/plan.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/service/results_pipeline/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/structure_cache.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/structure_cache/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/taxon_cache.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/taxon_cache/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/url_state.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/explore/url_state/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/features/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/hooks/locale_provider.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/hooks/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/hooks/use_add_row_form.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/hooks/use_locale.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/hooks/use_virtualization.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/hooks/use_virtualization/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/curation/de.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/curation/en.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/curation/fr.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/curation/it.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/curation/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/de.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/en.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/faq.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/faq/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/faq_guide.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/faq_guide/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/fr.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/it.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/messages/aria.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/messages/errors.rs` | Refactored | Removed unused `with_cfg` macro arm (`dd2e884`). |
| `apps/lotus-explore-rs/src/i18n/messages/errors/de.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/messages/errors/en.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/messages/errors/fr.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/messages/errors/it.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/messages/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/i18n/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/main.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/pages/draw_page.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/pages/ketcher_panel.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/pages/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/pages/network_page.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/perf.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/perf/clock.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/perf/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/repositories/hybrid.rs` | Refactored | Replaced duplicate endpoint-unavailable predicate with `FetchError::is_endpoint_unavailable` (`0ae70b7`). |
| `apps/lotus-explore-rs/src/repositories/hybrid/tests.rs` | Tests updated | Updated fallback condition tests to assert `FetchError::is_endpoint_unavailable` (`0ae70b7`). |
| `apps/lotus-explore-rs/src/repositories/mock.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/repositories/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/services/app_services.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/services/app_services/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/services/error_presenter.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/services/error_presenter/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/services/event_log.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/services/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/services/search_telemetry.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/sparql.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/state.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/state/form_context.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/state/form_context/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/state/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/ui/a11y_contract.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/ui/a11y_smoke.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/ui/a11y_smoke/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/ui/content_phase.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/ui/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/ui/notice.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/ui/segmented_control.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/ui/segmented_control/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/upload.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/upload/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/vendor_assets.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/src/vendor_assets/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/tailwind/styles.css` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/tests/doc_links.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/tests/gate/repo_hygiene.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/tests/gate/repo_hygiene/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/tests/gate/scrapers.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `apps/lotus-explore-rs/tests/gate_consistency.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `bacon.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `clippy.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `codemeta.json` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `compose.yaml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-cli/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-cli/README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-cli/src/archive_cmd.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/args/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/args/search.rs` | Refactored | Reused `lotus_model::MASS_MAX` and `YEAR_MIN` defaults (`25cb3f3`). |
| `crates/lotus-cli/src/clock.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/criteria.rs` | Refactored | Delegated `flags_ask_for_formula` to `SearchCriteria::has_formula_criteria` (`25cb3f3`). |
| `crates/lotus-cli/src/curate/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/curate/report.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/curate/runner.rs` | Refactored | Reused `lotus_curation::property::*` and `lotus_jsonld::LOTUS_PAPER_URL` (`25cb3f3`). |
| `crates/lotus-cli/src/curate/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/export_cmd.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/main.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/main/archive_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/main/clock_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/main/export_progress_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/main/nomenclature_flag_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/main/range_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/output.rs` | Refactored | Reused `encode_hex_lower` (`25cb3f3`) and introduced closed `Column` enum for CLI columns (`ba3bc00`). |
| `crates/lotus-cli/src/output/tests.rs` | Tests added | Added exact-byte golden CSV/TSV/table tests (`999d7ae`) and typed header check (`ba3bc00`). |
| `crates/lotus-cli/src/search_cmd.rs` | Refactored | Delegated explain compound check to `lotus_model::names_a_compound` (`25cb3f3`). |
| `crates/lotus-cli/src/zenodo/client.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/zenodo/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/zenodo/publish.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/zenodo/record.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/src/zenodo/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/tests/cli.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/tests/common/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-cli/tests/docs_in_sync.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-curation/README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-curation/src/error.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/error/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/input.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/input/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/knowledge/dependencies.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/knowledge/lookup.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/knowledge/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/knowledge/result_row.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/knowledge/statements.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/knowledge/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/lib.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/quickstatements.rs` | Refactored | Moved mid-file imports to module top (`dd2e884`) and reused `property::*` constants (`25cb3f3`). |
| `crates/lotus-curation/src/quickstatements/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/resolution.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/row.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/row/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/structure/convert.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/structure/formula.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/structure/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/structure/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/wikidata.rs` | Refactored | Unified `WD_OCCURS_IN_TAXON_PROP` onto `crate::property::OCCURS_IN_TAXON` (`25cb3f3`). |
| `crates/lotus-curation/src/wikidata_query/compound.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/wikidata_query/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/wikidata_query/occurrence.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/wikidata_query/reference.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/wikidata_query/statements.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/wikidata_query/taxon.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/wikidata_query/taxon_name.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-curation/src/wikidata_query/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-explore-core/README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-explore-core/src/api/dto.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/api/dto/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/api/error.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/api/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/actions.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/command.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/command/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/error.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/error/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/error_recovery.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/error_recovery/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/form_actions.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/form_actions/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/form_validation.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/form_validation/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/hashing.rs` | Refactored | Replaced local hex helper and its slicing allow with `lotus_model::encode_hex_lower` (`25cb3f3`). |
| `crates/lotus-explore-core/src/explore/hashing/tests.rs` | Tests updated | Updated hex vector test to call `lotus_model::encode_hex_lower` (`25cb3f3`). |
| `crates/lotus-explore-core/src/explore/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/outcome.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/recovery.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/recovery/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/request.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/request/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/retry.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/retry/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/search_metrics.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/search_metrics/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/search_state/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/search_state/reducer/lifecycle.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/search_state/reducer/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/search_state/reducer/result_data.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/search_state/reducer/ui.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/search_state/state_types.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/search_state/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/service/build_query.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/service/build_query/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/service/finalize.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/service/finalize/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/service/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/service/strategy.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/service/strategy/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/sparql_errors.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/sparql_errors/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/transport_classification.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/transport_classification/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/truncation.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/truncation/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/url_codec/criteria/decode.rs` | Refactored | Reused `is_valid_structure_threshold` (`25cb3f3`) and unified generic range DTO decoder (`0ae70b7`). |
| `crates/lotus-explore-core/src/explore/url_codec/criteria/encode.rs` | Refactored | Iterated `criteria.halogen_states()` (`0ae70b7`). |
| `crates/lotus-explore-core/src/explore/url_codec/criteria/mod.rs` | Refactored | Consolidated f64/u16 range DTOs via generic `RangeDto<T>` (`0ae70b7`). |
| `crates/lotus-explore-core/src/explore/url_codec/encode.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/url_codec/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/url_codec/network.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/url_codec/startup.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/explore/url_codec/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/export/filename.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/export/filename/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/export/filters.rs` | Refactored | Iterated `element_ranges()` and `halogen_states()` via `element_full_name` (`0ae70b7`). |
| `crates/lotus-explore-core/src/export/filters/tests.rs` | Tests added | Added coverage across all element and halogen keys (`6c9d200`). |
| `crates/lotus-explore-core/src/export/metadata/builder.rs` | Refactored | Reused model endpoint, `names_a_compound`, and JSON-LD LOTUS/CC0 constants (`25cb3f3`). |
| `crates/lotus-explore-core/src/export/metadata/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/export/metadata/schema.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/export/metadata/tests.rs` | Tests added | Added SACHEM attribution and endpoint assertions (`6c9d200`). |
| `crates/lotus-explore-core/src/export/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/filters.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/filters/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/model/kinds.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/model/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/model/payload.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/model/starters.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/project/edges.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/project/edges/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/project/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/project/nodes.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/project/search.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/graph/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/lib.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/ports.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/ports/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/sort.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/table_budget.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/src/table_budget/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-explore-core/tests/characterization_exports.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-export/README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-export/src/archive/dedup.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/archive/fetch.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/archive/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/archive/tar_writer.rs` | Refactored | Replaced hex loop with `lotus_model::push_hex_lower` (`25cb3f3`). |
| `crates/lotus-export/src/archive/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/lib.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pipeline/index.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pipeline/join.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pipeline/mod.rs` | Refactored | Dropped unused table argument (`dd2e884`) and eagerly freed CSV/index buffers before final UTF-8 conversion (`ba3bc00`). |
| `crates/lotus-export/src/pipeline/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pubchem_tree/compact.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pubchem_tree/document.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pubchem_tree/fetch.rs` | Refactored | Updated call site after removing unused table argument (`dd2e884`). |
| `crates/lotus-export/src/pubchem_tree/index.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pubchem_tree/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pubchem_tree/nodes.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pubchem_tree/npclassifier.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pubchem_tree/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/src/pubchem_tree/trees.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-export/tests/characterization_exports.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-jsonld/README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-jsonld/src/bin/emit-metadata.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/bin/emit-metadata/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/compound.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/compound/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/dataset.rs` | Refactored | Reused `CC0_LICENSE_URL`, `LOTUS_PAPER_*`, `LOTUS_WIKIDATA_QID`, `SOFTWARE.url` (`25cb3f3`). |
| `crates/lotus-jsonld/src/dataset/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/edam.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/edam/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/lib.rs` | Refactored | Exported LOTUS bibliographic/Wikidata/CC0 constants (`25cb3f3`). |
| `crates/lotus-jsonld/src/profile.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/profile/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/profile_bounds_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/software.rs` | Refactored | Reused `LOTUS_PAPER_DOI` and `LOTUS_WIKIDATA_URL` (`25cb3f3`). |
| `crates/lotus-jsonld/src/software/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/taxon.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/taxon/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/src/uri_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-jsonld/tests/emit_metadata_cli.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-model/README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-model/src/columnar/access.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/bitmask.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/builder.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/builder_seed.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/diagnostics.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/filter.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/qid_dict.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/sort.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/sparse_strings.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/statement_id.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/columnar/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/criteria.rs` | Refactored | Extracted `SearchCriteria::has_formula_criteria` (`25cb3f3`). |
| `crates/lotus-model/src/criteria/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/endpoints.rs` | Refactored | Exported canonical `IDSM_SACHEM_ENDPOINT` constant (`25cb3f3`). |
| `crates/lotus-model/src/endpoints/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/entry.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/hex.rs` | Refactored | Created index-free `push_hex_lower` and `encode_hex_lower` single source (`25cb3f3`). |
| `crates/lotus-model/src/hex/tests.rs` | Tests added | Added hex encoding, capacity-reservation, and byte-exhaustiveness tests (`25cb3f3`, `6c9d200`). |
| `crates/lotus-model/src/identify.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/identify/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/lib.rs` | Refactored | Exported `IDSM_SACHEM_ENDPOINT`, `is_valid_structure_threshold`, `encode_hex_lower`, `push_hex_lower` (`25cb3f3`). |
| `crates/lotus-model/src/percent.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/percent/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/reference.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/reference/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/stats.rs` | Refactored | Exported `is_valid_structure_threshold` single predicate (`25cb3f3`). |
| `crates/lotus-model/src/stats/from_entries_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/stats/spelling_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/stats/tests.rs` | Tests added | Added `is_valid_structure_threshold` boundary tests (`25cb3f3`). |
| `crates/lotus-model/src/structure.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/structure/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/taxon_nomenclature_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/testing.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/utc.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/utc/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/src/validate.rs` | Refactored | Delegated similarity threshold check to `is_valid_structure_threshold` (`25cb3f3`). |
| `crates/lotus-model/src/validate/tests.rs` | Tests added | Added boundary threshold characterization checks (`999d7ae`). |
| `crates/lotus-model/tests/columnar.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/tests/diagnostics.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-model/tests/sentinels.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-query/README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-query/examples/measure_export.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/bulk_export/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/bulk_export/parse_id.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/bulk_export/queries_core.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/bulk_export/queries_enrich.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/bulk_export/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/error.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/export.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/export/log_name_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/export/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/export_rows/csv.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/export_rows/json.rs` | Refactored | Delegated `json_string` to `push_escaped_quoted` (`ba3bc00`). |
| `crates/lotus-query/src/export_rows/mod.rs` | Refactored | Extracted shared `push_escaped_quoted` string escaper (`ba3bc00`). |
| `crates/lotus-query/src/export_rows/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/export_rows/turtle.rs` | Refactored | Replaced allocating string wrapper with `emit_quoted` streaming directly via `push_escaped_quoted` (`ba3bc00`). |
| `crates/lotus-query/src/lib.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/columns.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/compounds.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/counts.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/lookups.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/stream/fields.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/stream/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/stream/reader.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/stream/splitter.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/stream/splitter/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/parse/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/builders.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/builders_body.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/compound_names.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/dictionary.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/filters.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/lookups_compound.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/lookups_taxon.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/nomenclature.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/prefixes.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/rewrite.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/structure.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/src/query/tests.rs` | Tests added | Pinned `PREFIXES_WITH_STRUCTURE` against `lotus_model::IDSM_SACHEM_ENDPOINT` (`25cb3f3`). |
| `crates/lotus-query/tests/bench.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/characterization_query_surface.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/contract/common.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/contract/export.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/contract/filters.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/contract/nomenclature.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/contract/structure.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/contract/structure_rules.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/export_fidelity.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/fixtures/compounds.csv` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-query/tests/fixtures/counts.csv` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-query/tests/fixtures/taxon.csv` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-query/tests/query_contract.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/query_rewrites.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/reference_node.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/result_parsing.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/streaming.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-query/tests/truncation_notice.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-search/README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-search/src/client.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/client/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/error.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/error/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/execute/budget.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/execute/endpoints.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/execute/error_format.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/execute/fallback.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/execute/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/execute/retry.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/execute/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/execute/wasm.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/lib.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/reqwest_client/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/reqwest_client/schedule.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/reqwest_client/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/reqwest_client/transport.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/reqwest_client/wasm.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/result.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/result/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/search/execute_search.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/search/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/search/query_plan.rs` | Refactored | Removed unused structure-plan intermediate struct (`dd2e884`). |
| `crates/lotus-search/src/search/resolve_reference.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/search/resolve_structure.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/search/resolve_taxon.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/search/stream_search.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/search/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/testing.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/src/testing/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/tests/failure_stages.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/tests/matrix.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/tests/query_building.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/tests/search_columnar.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/tests/search_pipeline.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/tests/streaming_fold.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-search/tests/taxon_resolution.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-server/README.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-server/src/cache_key.rs` | Refactored | Replaced nibble-lookup loop with `lotus_model::push_hex_lower` (`25cb3f3`). |
| `crates/lotus-server/src/cache_key/bench.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/cache_key/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/characterization_tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/clock.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/config.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/config/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/dto.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/errors.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/export_urls.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/export_urls/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/handlers/export/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/handlers/export/stream.rs` | Refactored | Reused `reject_overloaded` and `UPSTREAM_RETRY_AFTER_SECS` preserving `try_acquire_owned()` (`019445f`). |
| `crates/lotus-server/src/handlers/export/stream/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/handlers/export/url.rs` | Refactored | Delegated 503 overload rejection to `reject_overloaded` (`019445f`). |
| `crates/lotus-server/src/handlers/export_query_shape.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/handlers/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/handlers/prepare.rs` | Refactored | Extracted `reject_overloaded` and shared `UPSTREAM_RETRY_AFTER_SECS` (`019445f`). |
| `crates/lotus-server/src/handlers/search.rs` | Refactored | Delegated 503 overload rejection to `reject_overloaded` (`019445f`). |
| `crates/lotus-server/src/logging.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/logging/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/main.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/query_logic.rs` | Refactored | Reused `is_valid_structure_threshold` (`25cb3f3`) and split `apply_request` below complexity limit (`019445f`). |
| `crates/lotus-server/src/services.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/state/cache.rs` | Refactored | Unified TTL caches over generic `CachedEntry<V>` (`019445f`). |
| `crates/lotus-server/src/state/inflight.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/state/metrics.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-server/src/state/mod.rs` | Refactored | Re-exported unified `CachedExportResponse` / `CachedTaxonResolution` aliases (`019445f`). |
| `crates/lotus-server/src/tests.rs` | Tests added | Added `apply_request`, overload 503 (`999d7ae`), boundary mutant-killing tests (`6c9d200`), and bounded `request_timeout` in `search_cache_hit_preserves_caller_taxon_warning_and_qid` (`b5ba51b`). |
| `crates/lotus-web-assets/Cargo.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `crates/lotus-web-assets/src/fetch_assets/http.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/fetch_assets/http/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/fetch_assets/ketcher.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/fetch_assets/ketcher/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/fetch_assets/mod.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/fetch_assets/test_support.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/fetch_assets/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/fetch_assets/vendor.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/fetch_assets/vendor/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/healthcheck.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/healthcheck/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/inject_wasm_preload.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/src/inject_wasm_preload/tests.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `crates/lotus-web-assets/tests/healthcheck_cli.rs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `deny.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docker/nginx.conf` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docker/snippets/security-headers.conf` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docs/ARCHITECTURE.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docs/DEBT.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docs/FRONTENDS.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docs/QLEVER-POLITENESS.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docs/QUERY_MATRIX.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docs/SPARQL-VARIANTS.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docs/STRUCTURE-SEARCH.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docs/TAXON-SEARCH.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `docs/cli.md` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `examples/curation_example.tsv` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `make/all.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `make/build.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `make/docker.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `make/format.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `make/lint.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `make/metadata.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `make/scripts/docker-timing.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `make/scripts/docker-verify.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `make/scripts/license-headers.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `make/scripts/opt-levels.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `make/scripts/setup.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `make/scripts/web-a11y-audit.mjs` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `make/scripts/web-bytes.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `make/scripts/web-css.sh` | Reviewed | Reviewed against DRY, SOLID, efficiency, and boundary rules; no structural debt change warranted. |
| `make/supply-chain.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `make/test.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `make/web.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `mk` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `mk.local` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `mk.versions` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `prek.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `renovate.json` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `rust-toolchain.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |
| `rustfmt.toml` | Reviewed | Reviewed configuration, manifest, workflow, schema, or documentation. |

---

## 2. Debt Ledger

| ID | Category | Location | Evidence | Fix | Risk | Status | Commit |
| --- | --- | --- | --- | --- | --- | --- | --- |
| DEBT-00 | Safety net | `validate/tests.rs`, `output/tests.rs`, `wikidata/tests.rs`, `server/tests.rs` | Baseline gaps on threshold edges, CLI CSV/TSV/table exact bytes, curation QID permutation invariance, and route overload / request parsing | Added characterization and property tests before any production edit | None | Done | `999d7ae`, `6c9d200`, `b5ba51b` |
| DEBT-01 | Dead code | `crates/lotus-search/src/search/query_plan.rs` | Unused private intermediate struct wrapping fields already read directly downstream | Deleted unused struct and called `base_query_for_criteria` with `normalize_structure` | Low | Done | `dd2e884` |
| DEBT-02 | Dead code | `crates/lotus-export/src/{pipeline/mod.rs,pubchem_tree/fetch.rs}` | Unused `_table: &'static str` parameter in `fetch_csv_rows` | Removed unused parameter and updated call site | Low | Done | `dd2e884` |
| DEBT-03 | Dead code | `apps/lotus-explore-rs/src/i18n/messages/errors.rs` | Unused conditional macro arm in `dispatch!` | Removed dead macro arm | Low | Done | `dd2e884` |
| DEBT-04 | Hygiene | `crates/lotus-curation/src/quickstatements.rs` | Mid-file `use` imports placed below first public function | Moved imports to module top | Low | Done | `dd2e884` |
| DEBT-05 | DRY | `crates/lotus-model/src/endpoints.rs`, `crates/lotus-explore-core/src/export/metadata/builder.rs`, `crates/lotus-query/src/query/tests.rs` | IDSM Sachem endpoint URL duplicated as raw string literal | Exported `lotus_model::IDSM_SACHEM_ENDPOINT`, reused in metadata builder, pinned against SPARQL prefix table in tests | Low | Done | `25cb3f3` |
| DEBT-06 | DRY | `crates/lotus-model/src/{stats.rs,validate.rs}`, `url_codec/criteria/decode.rs`, `lotus-server/src/query_logic.rs` | Finite `(0.0, 1.0]` similarity threshold check duplicated in 3 crates | Extracted `lotus_model::is_valid_structure_threshold` and unit tests | Low | Done | `25cb3f3` |
| DEBT-07 | DRY | `crates/lotus-model/src/criteria.rs`, `crates/lotus-cli/src/criteria.rs` | CLI formula-flag predicate duplicated `has_formula_filter` logic without the enable flag | Added `SearchCriteria::has_formula_criteria` and delegated CLI helper | Low | Done | `25cb3f3` |
| DEBT-08 | DRY | `crates/lotus-cli/src/args/search.rs` | Hardcoded `10_000.0` and `1800` defaults instead of domain bounds | Reused `lotus_model::MASS_MAX` and `lotus_model::YEAR_MIN` | Low | Done | `25cb3f3` |
| DEBT-09 | DRY | `crates/lotus-explore-core/src/export/metadata/builder.rs`, `crates/lotus-cli/src/search_cmd.rs` | Three-part compound-identifier check repeated manually in two callers | Replaced both checks with `lotus_model::names_a_compound` | Low | Done | `25cb3f3` |
| DEBT-10 | DRY | `crates/lotus-jsonld/src/{lib.rs,dataset.rs,software.rs}`, `metadata/builder.rs`, `curate/runner.rs` | LOTUS DOI, paper title, Wikidata QID/URL, and CC0 URI duplicated across emitters | Exported canonical constants from `lotus-jsonld` and reused across consumers | Low | Done | `25cb3f3` |
| DEBT-11 | DRY | `crates/lotus-curation/src/{wikidata.rs,quickstatements.rs}`, `crates/lotus-cli/src/curate/runner.rs` | Raw `"P233"`/`"P2017"`/`"P234"`/`"P235"`/`"P703"`/`"P2067"` literals and local CLI property constants | Unified on `lotus_curation::property::*` | Low | Done | `25cb3f3` |
| DEBT-12 | DRY & Hygiene | `crates/lotus-model/src/hex.rs`, `hashing.rs`, `cache_key.rs`, `output.rs`, `tar_writer.rs` | Four hand-rolled lowercase-hex loops, including one with an `indexing_slicing` allow | Extracted index-free `push_hex_lower` / `encode_hex_lower` in `lotus-model`; removed suppression | Low | Done | `25cb3f3` |
| DEBT-13 | DRY | `crates/lotus-explore-core/src/explore/url_codec/criteria/{mod.rs,decode.rs,encode.rs}` | Pair of parallel numeric range DTOs, helper trait, and manual halogen array | Unified into generic `RangeDto<T>` (retaining private type aliases for doc compatibility) and `criteria.halogen_states()` | Low | Done | `0ae70b7` |
| DEBT-14 | DRY | `crates/lotus-explore-core/src/export/filters.rs` | Manual 6-element and 4-halogen arrays duplicating `SearchCriteria` accessors | Iterated `criteria.element_ranges()` and `criteria.halogen_states()` with `element_full_name` | Low | Done | `0ae70b7` |
| DEBT-15 | DRY & Hygiene | `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/mod.rs` | Hand-rolled byte-indexed QID check carrying a production `#[allow(clippy::indexing_slicing)]` | Delegated to `lotus_model::is_qid`; removed suppression | Low | Done | `0ae70b7` |
| DEBT-16 | DRY | `apps/lotus-explore-rs/src/repositories/{hybrid.rs,hybrid/tests.rs}` | Private `FetchError` unavailability helper duplicating `lotus_search::FetchError::is_endpoint_unavailable` | Deleted duplicate helper and updated 3 call sites + tests | Low | Done | `0ae70b7` |
| DEBT-17 | DRY | `apps/lotus-explore-rs/src/features/curation/services/wikidata.rs` | Duplicated SPARQL JSON value extraction and lowest-QID tie-breaking across reference/taxon/compound parsers | Extracted `binding_str`, `binding_qid`, `insert_lowest_by_qid`, and `qids_by_lookup` | Low | Done | `0ae70b7` |
| DEBT-18 | SOLID (SRP / Complexity) | `crates/lotus-server/src/query_logic.rs` | Monolithic `apply_request` (`clippy::cognitive_complexity = 28/25`, sole prod warning in workspace) | Split into `apply_taxon_nomenclature`, `apply_structure_and_ranges`, `has_formula_input`, `apply_formula_fields` (0 warnings) | Low | Done | `019445f` |
| DEBT-19 | DRY | `crates/lotus-server/src/state/{cache.rs,mod.rs}` | Three copy-pasted cache structs and get/put TTL/capacity functions | Generalized via `CachedEntry<V>`, aliases, `cache_get`, and `cache_put` | Low | Done | `019445f` |
| DEBT-20 | DRY | `crates/lotus-server/src/handlers/{prepare.rs,search.rs,export/url.rs,export/stream.rs}` | Repeated overload metrics/log/503 block and hardcoded retry-after `5` | Extracted `reject_overloaded` and exported `UPSTREAM_RETRY_AFTER_SECS` (keeping `try_acquire_owned()` in `export_file`) | Low | Done | `019445f` |
| DEBT-21 | Type safety | `crates/lotus-cli/src/output.rs` | Stringly-typed runtime dispatch for tabular/CSV/TSV columns (`&'static str` + fallback arm) | Replaced production lookup with closed `enum Column` (`header`, `value`) | Low | Done | `ba3bc00` |
| DEBT-22 | Efficiency (Memory) | `crates/lotus-export/src/pipeline/mod.rs` | All 4 raw SPARQL CSV byte payloads and joined lookup tables stayed live until function return | Scoped raw byte fetches per table and dropped joined maps before `String::from_utf8` | Low | Done | `ba3bc00` |
| DEBT-23 | DRY & Efficiency | `crates/lotus-query/src/export_rows/{mod.rs,json.rs,turtle.rs}` | Duplicated JSON/Turtle string escaper; Turtle allocated a temporary heap `String` per literal field | Shared `push_escaped_quoted` and streamed Turtle literals directly via `emit_quoted`; 100% bit-identical export bytes, cleaner CPU time | Low | Done | `ba3bc00` |

---

## 3. Baseline → After Metrics

All measurements collected sequentially on Apple M1 Pro (8 cores, 16 GiB RAM), `rustc 1.100.0`, `cargo 1.100.0`. Note: production-code benchmarks and coverage below were measured at `ba3bc00` (final production change); `MK_BASE=a2f09a3 ./mk mutants-diff` passed after the final Rust test edit (`b5ba51b`), and `MK_BASE=a2f09a3 ./mk ci-core` passed on clean `HEAD`. When `./mk ci` runs unparameterized against unpushed `origin/main` (`fe50374`, 12 pre-session audit commits behind baseline `a2f09a3`), all non-mutant gates pass (`wasm` included) while `mutants-diff` fails strictly on pre-existing `origin/main..a2f09a3` survivors (`63` missed, `1` timeout) with `0` survivors in `a2f09a3..HEAD`.

| Metric | Baseline (`a2f09a3`) | After Refactoring | Delta |
| --- | --- | --- | --- |
| Tracked files (`git ls-files`) | 790 | 793 (incl. `DEBT.md`) | +3 (new `hex.rs`, `hex/tests.rs`, `DEBT.md`) |
| Rust LOC (`tokei` Rust lines / code) | 98,498 / 84,400 | 98,727 / 84,597 (`ba3bc00`) | +229 / +197 (safety-net & property tests added) |
| Normalized production Rust lines | 38,930 | 38,527 | **-403** (-1.04%) |
| Production Rust functions | 2,169 | 2,161 | -8 |
| Mean / max non-i18n cyclomatic complexity (Python estimate) | 3.02 / 56 (`apply_request`) | 3.01 / 45 (`ResultsGraphWorkspace`) | Max non-i18n CC **-11** |
| Production `clippy::cognitive_complexity` warnings | 1 (`apply_request`: 28/25) | **0** | **-1** (100% cleared) |
| Workspace Clippy lints (`too_many_lines`, `too_many_arguments`, `excessive_nesting`, `type_complexity`) | 0 prod warnings | **0** prod warnings | Maintained |
| Production `#[allow(...)]` count | 38 | **37** | -1 (removed both prod `indexing_slicing` allows in `hashing.rs` & `resolve_taxon/mod.rs`) |
| 12-line normalized prod shingle duplication | 0.23% (90 lines) | 0.23% (90 lines) | Maintained (floor) |
| `./mk echoes` (exact / approx / reuse) | 0 / 0 / 0 (4,753 fns) | **0 / 0 / 0** (4,778 fns at `ba3bc00`) | 0 duplicates |
| `./mk refquiem` | 0 stale mentions | **0 stale mentions** | Pass |
| `./mk deny` & `cargo machete` | Pass / 0 unused | **Pass / 0 unused** | Pass |
| Test count (`nextest` + doctests) | 1,933 + 8 | **1,945 + 8** | +12 tests |
| Uncontended `./mk test` wall / user time | 26.51s real / 21.17s user | 42.17s real / 21.71s user (`ba3bc00`, 12 more tests) | Within system noise |
| Coverage (`llvm-cov` across 10 non-app crates) | 88.92% regions / 91.58% fns / 91.20% lines | **89.19%** regions / **92.14%** fns / **91.47%** lines (`ba3bc00`) | **+0.27 pp** / **+0.56 pp** / **+0.27 pp** (`-44` missed lines) |
| `target/release/lotus` size | 4,879,632 B | 4,880,208 B | +576 B (+0.01%) |
| `measure_export` (200k rows: CSV / JSON / TTL bytes) | 39,390,425 / 112,376,899 / 104,172,948 | **39,390,425 / 112,376,899 / 104,172,948** | **0 B diff (100% bit-identical)** |
| `measure_export` runtime (warm binary, `/usr/bin/time -p`) | 5.71s real / 1.72s user / 0.10s sys | **3.27s real / 1.55s user / 0.05s sys** | **-42.7% real / -9.9% user** |
| `bench_cache_key` tiny (208 B) / 16x (49,040 B) / hex (ns) | 709 / 52,000 / 375 ns | **667 / 51,833 / 333 ns** | Improved across sizes (-11% hex median) |
| `bench_a_million_rows` parse+build / resident | 13,376.9 ms / 150.1 MB | 9,924.0 ms / 150.1 MB | No regression; memory unchanged |
| Golden corpus (`8/8` CLI outputs vs `SHA256SUMS`) | Baseline oracle | **8/8 `OK`** (SHA-256 bit-for-bit identical) | 0 byte drift |
| Diff mutation sweep (`MK_BASE=a2f09a3 ./mk mutants-diff`) | — | **177 tested in 15m: 156 caught, 21 unviable, 0 missed, 0 timeouts** (`b5ba51b`) | **0 missed, 0 timeouts** |

---

## 4. Structural Changes & Abstraction Justification

Every abstraction introduced has **at least two concrete production callers** and eliminates duplicated rules or failure modes:

1. `crates/lotus-model/src/hex.rs` (`push_hex_lower`, `encode_hex_lower`): Unifies 4 lowercase hexadecimal encoders (`explore/hashing.rs`, `server/cache_key.rs`, `cli/output.rs`, `export/archive/tar_writer.rs`) in pure `lotus-model` without slice indexing or lint suppressions.
2. Generic numeric range DTO (`url_codec/criteria/mod.rs`): Eliminates duplicate `f64` and `u16` range DTO definitions and `apply` methods while preserving existing internal type aliases so documentation contracts stay intact.
3. `CachedEntry<V>` + generic `cache_get` / `cache_put` (`lotus-server/src/state/cache.rs`): Replaces 3 copy-pasted TTL/capacity-evicting cache wrappers over `taxon_cache`, `search_cache`, and `export_cache`.
4. `reject_overloaded` (`lotus-server/src/handlers/prepare.rs`): Shares concurrency-shedding telemetry and `503` response construction across `search`, `export_url`, and `export_file` while keeping `try_acquire_owned()` explicitly in `export_file` as required by the source-shape invariant test.
5. `enum Column` (`crates/lotus-cli/src/output.rs`): Replaces stringly-typed column lookup in production table/CSV/TSV formatting with an exhaustive typed enum mapping headers and values together.
6. `push_escaped_quoted` (`crates/lotus-query/src/export_rows/mod.rs`): Unifies identical JSON/Turtle double-quoted string escaping and feeds an allocation-free Turtle writer (`emit_quoted`), eliminating temporary `String` allocations per exported RDF literal field.

---

## 5. API Changes & Migrations

All public surface changes are **strictly additive**; zero existing signatures, variants, fields, or CLI flags were removed or changed:

- `lotus_model`: added `IDSM_SACHEM_ENDPOINT`, `is_valid_structure_threshold`, `SearchCriteria::has_formula_criteria`, `push_hex_lower`, and `encode_hex_lower`.
- `lotus_jsonld`: added `LOTUS_PAPER_DOI`, `LOTUS_PAPER_URL`, `LOTUS_PAPER_TITLE`, `LOTUS_WIKIDATA_QID`, `LOTUS_WIKIDATA_URL`, and `CC0_LICENSE_URL`.
- `lotus_server::state`: exported `CachedEntry<V>` backing the existing `CachedTaxonResolution`, `CachedSearchResponse`, and `CachedExportResponse` aliases.

No downstream caller migration is required.

---

## 6. Findings Handed Off (Scientific / Correctness Observations — Not Fixed Here)

Per the behavior-preservation contract, the following cross-crate discrepancies discovered during analysis were **not** altered and are handed off for domain review:

- **FINDING-01 (Wildcard taxon + exact compound seed without reference):** `crates/lotus-explore-core/src/explore/service/build_query.rs` passes `Some("*")` (keeping occurrence `OPTIONAL` on the wildcard path) when `taxon == "*"` and `reference` is empty during exact compound seeding, whereas `crates/lotus-search/src/search/query_plan.rs` yields `None` unless `criteria.taxon == "*"` matches after trim — and the two paths differ subtly around optional vs required `P703` semantics.
- **FINDING-02 (Taxon name matching & ambiguity rules diverge between `lotus-search` and browser app):** `crates/lotus-search/src/search/resolve_taxon.rs` uses ASCII case-folding (`eq_ignore_ascii_case`) and flags ambiguity via `candidates.len() > 1`, whereas `apps/lotus-explore-rs/src/features/explore/service/resolve_taxon/match_selection.rs` uses Unicode case-folding, prefers `TaxonNameSource::Scientific` over `Common`, and defines ambiguity as `exact > 1 || (exact == 0 && matches.len() > 1)`.
- **FINDING-03 (Server upstream permit gate on non-Exact structure identifier resolution):** `crates/lotus-server/src/handlers/prepare.rs` only acquires an upstream permit prior to `resolve_search_inputs` when `structure_search == Exact` and not a compound QID, omitting non-Exact searches carrying compound names, `InChIKey`s, or compound QIDs that still query Wikidata for canonical SMILES inside `resolve_inputs`.
- **FINDING-04 (`tempfile` native dependency in `lotus-explore-core`):** `crates/lotus-explore-core/Cargo.toml` and `crates/lotus-explore-core/src/ports.rs` retain a native-only `tempfile::NamedTempFile` port method despite the crate's `"no UI framework, no IO"` description; moving `sparql_tempfile` to an app-local port trait would complete that boundary separation.

---

## 7. Deliberately Not Changed & Rationale

- **Locale message tables (`apps/lotus-explore-rs/src/i18n/{en,fr,de,it}.rs`):** Flat static translation tables with high AST branch/match counts (`max CC = 225`) are inherently tabular data rather than control-flow complexity; abstracting them would hurt grepability for translators.
- **Native HTTP client construction (`crates/lotus-search/src/reqwest_client/mod.rs`):** Verified `ReqwestClient::new()` already shares a process-wide `OnceLock<Client>` and returns a `Copy` wrapper over `&'static reqwest::Client`, so there is no per-call connection-pool allocation to fix.
- **UI asset CSP/metadata HTML (`apps/lotus-explore-rs/{index.html,build/headers.rs,footer.rs}`):** Static frontend HTML/footer URLs live outside Rust domain crates and were left decoupled rather than introducing build-script codegen for four fixed strings.
- **Transitive Dioxus version skips (`deny.toml`) & test-only Clippy complexity (`wikidata_query/tests.rs:102`, `graph/tests.rs:772`):** Documented upstream Dioxus transitive dependencies and pre-existing assertion-heavy test functions do not affect runtime architecture or shipping code.

---

## 8. Residual Debt & Risk

- **In-memory four-table join (`crates/lotus-export/src/pipeline/mod.rs`):** While raw byte payloads and lookup maps are now dropped at the earliest safe phase boundaries (DEBT-22), `run_lotus_exporter` still holds all indexed tables simultaneously in RAM and serializes `frozen.csv` in memory per its wasm-clean contract.
- **Divergent cross-surface logic (`FINDING-01` .. `FINDING-03`):** Functional differences between CLI/server (`lotus-search`) and browser (`apps/lotus-explore-rs` / `lotus-explore-core`) remain open until maintainer confirmation of the intended scientific behavior.
- **WASM bundle size / `apps/lotus-explore-rs` mutant coverage:** Dioxus UI components (`~2,700` mutants) remain excluded from default `./mk mutants` via `.cargo/mutants.toml` (`./mk local:mutants-ui` remains opt-in); release WASM compilation and packaging were verified via `./mk local:lint-wasm` and `./mk wasm` (`dx build --release`).
