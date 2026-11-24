# Session Log — `lotus-explore-rs` Structural Refactor

> **Note on file location:** The repository's `repo_hygiene::tests::session_log_is_absent` test enforces that `SESSION_LOG.md` does not exist at the repository root. Per the non-negotiable behavior-preservation rule, that test is preserved untouched and the session log is maintained here at `docs/audit/SESSION_LOG.md`.

---

## Baseline Metrics (`7b88120`)

| Metric | Value |
| --- | --- |
| Workspace crates | 9 |
| Total `.rs` files / prod LOC / total code LOC | 536 / 54,890 / 80,384 |
| Prod files > 300 LOC (total files > 300 LOC) | 51 (103) |
| Max prod file LOC | 2,041 (`crates/lotus-query/src/query.rs`) |
| Max prod fn LOC (non-i18n) | 261 (`format_values_clause` in `crates/lotus-query/src/query.rs:1498`) |
| Prod functions > 40 LOC / > 4 params | 182 / 72 |
| Max workspace dependency depth | 4 (`apps/lotus-explore-rs` → `lotus-explore-core` → `lotus-search` → `lotus-query` → `lotus-model`) |
| Prod `pub` items (non-app crates) | `lotus-explore-core`: 432, `lotus-model`: 271, `lotus-query`: 142, `lotus-curation`: 128, `lotus-search`: 124 (+ `lotus_model::*`), `lotus-jsonld`: 88, `lotus-cli`: 65, `lotus-web-assets`: 17 |
| Prod `Result<_, String>` sites | 61 |
| Prod `#[allow(...)]` / `#![allow(...)]` | 58 |
| Junk-drawer files (`utils`/`helpers`/`common`) | 6 |
| `lotus-cli` release binary size | 5.5 MiB (`2.0 MiB` `.text`; `lotus_explore_core`: `107.4 KiB`, `lotus`: `119.2 KiB`, `lotus_search`: `32.0 KiB`) |

---

## Step Log

### Step 1a — Phase 0 Audit, Target Architecture, and Migration Plan
- **Status:** In progress
- **Files created/updated:**
  - `docs/audit/AUDIT.md` (created)
  - `docs/ARCHITECTURE.md` (updated for target 11-crate layered architecture)
  - `docs/REFACTOR_PLAN.md` (created)
  - `docs/audit/SESSION_LOG.md` (created)
- **Decisions:**
  - Keep `repo_hygiene::tests::session_log_is_absent` intact and place the session log at `docs/audit/SESSION_LOG.md` (documented for reviewer in `REVIEW_NOTES.md`).
  - Introduce zero new external dependencies; implement deterministic seeded property-test helpers directly in workspace test modules.
- **Open questions:**
  - Check whether `lotus-cli` still needs any dependency on `lotus-explore-core` once `lotus-export` is extracted (appears removable).
