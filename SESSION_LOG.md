# SESSION LOG

Work log for maintenance passes over `lotus-explore-rs`. Each pass records what
it measured, what it changed, and what it deliberately left alone with a reason.

---

## Inputs this pass was asked to read

The brief for this pass named three files to read first. Two do not exist:

| Named file                  | Status                                                                |
| --------------------------- | --------------------------------------------------------------------- |
| `docs/REFACTOR_PLAN.md`     | Deleted in `b9842ef` ("remove the plan that is finished")             |
| `SESSION_LOG.md`            | Did not exist; created by this pass                                  |
| `AI_AGENT_GUIDE.md`         | Does not exist. Closest is `.github/ai/AGENTS.md`: 25 lines, graphify only |

So the instruction to "finish anything SESSION_LOG.md lists as skipped or
open" had no referent. No such list was invented to close. Everything else in
the brief was self-contained and is recorded below.

---

## Baseline

Measured at `9c6dba9`, before any edit in this pass.

### Size

`tokei 15.0.0`, code lines excluding comments and blanks.

| Crate                   |    code | comments |  blank |
| ----------------------- | ------: | -------: | -----: |
| `apps/lotus-explore-rs` | 25,391 |    2,082 |  2,968 |
| `crates/lotus-curation` |  1,762 |      412 |    231 |
| `crates/lotus-query`    |  1,647 |      258 |    203 |
| `crates/lotus-cli`      |  1,398 |      212 |    158 |
| `crates/lotus-search`   |  1,348 |      353 |    245 |
| `crates/lotus-jsonld`   |  1,177 |      180 |    141 |
| `crates/lotus-model`    |    932 |      199 |    134 |
| `crates/lotus-web-assets` |   632 |      49 |     66 |
| **Total**               | **34,287** | **3,745** |          |

`apps/lotus-explore-rs` is 74% of the code. Its 25,391 lines include 2,082
lines of comment, the densest in the tree.

### Lints

| Measurement                                   | Baseline |
| --------------------------------------------- | -------: |
| `cargo clippy --workspace --all-targets`      |        0 |
| same, with `-W clippy::pedantic`              |        0 |
| `cargo fmt --check`                           |      pass |

`clippy::pedantic` is **already** `deny` at `priority = -2` in the workspace
`[lints.clippy]` table, as are `nursery` (`-3`) and `all` (`-1`). The brief asks
for a pedantic count to be lowered; there is nothing to lower, and enabling it
was not a change to make. Recorded so the final report is not read as a silent
no-op.

### Panic sites

Workspace `[lints.rust]` denies `expect_used` and `panic`, so these are zero by
construction rather than by luck.

| Construct           | Production code | Test code |
| ------------------- | -------------: | ---------: |
| `.unwrap()`         |              0 |         14 |
| `.expect(..)`       |              0 |          ~90 |
| `panic!(..)`        |              0 |           0 |
| `todo!(..)`         |              0 |           0 |
| `unimplemented!(..)`|              0 |           0 |

A naive grep finds 11 `.unwrap()` in `src/`. All 11 are inside
`#[cfg(test)] mod tests` blocks (`server/config.rs`, `server/tests.rs`,
`features/explore/service/resolve_taxon/mod.rs`,
`features/explore/service/api_pipeline.rs`) — grep cannot see block boundaries,
so the count was verified by locating each `mod tests` line above it.

### `#[allow(...)]` sites

81 total in `src/`.

| Lint                             | Sites |
| -------------------------------- | ----: |
| `missing_docs`                   |    17 |
| `dead_code`                      |    10 |
| `clippy::indexing_slicing`       |     7 |
| `clippy::too_many_lines`         |     6 |
| `clippy::redundant_pub_crate`    |     5 |
| `clippy::struct_excessive_bools` |     4 |
| `clippy::unused_async`            |     3 |
| `clippy::needless_pass_by_value`  |     3 |
| `clippy::cast_precision_loss`    |     3 |
| `clippy::cast_possible_truncation` |   3 |
| `clippy::unused_self`            |     2 |
| `clippy::struct_field_names`     |     2 |
| (remainder, one each)            |    16 |

### Tests

| Measurement                     | Baseline |
| ------------------------------- | -------: |
| `cargo test --workspace`        | 645 pass, 0 fail |
| wall clock (warm)               |   9.26 s |

### Bundle size

`cargo build --release --target wasm32-unknown-unknown -p lotus-explore-rs`.

| Measurement           |     Bytes |
| --------------------- | --------: |
| raw `.wasm`           | 2,570,943 |
| `wasm-opt -Oz`        | 2,244,664 |
| gzipped `-Oz`         |   781,488 |

### Dependencies

| Measurement                    | Baseline |
| ------------------------------ | -------: |
| `cargo tree -d` version pairs  |       30 |
| distinct duplicated crates     |       15 |
| `cargo machete`                |    clean |
| `cargo deny check`             |    clean |

### Coverage

Measured twice, because the first number was wrong and the wrongness mattered.

`cargo llvm-cov --workspace --lib` reports **84.48%** over 6,404 lines. That
figure is an artifact: `--lib` excludes the integration tests in each crate's
`tests/` directory, and in this workspace the library crates are tested almost
entirely from there. `lotus-query/src/query.rs` reads as **8.13%** under `--lib`
and **92.95%** with integration tests counted, because all twenty of its
behaviours are pinned in `tests/query_contract.rs`.

The correct measurement is `cargo llvm-cov --workspace` (no `--lib`):

| Crate                   | Regions |  Missed |  Region cover |
| ----------------------- | ------: | ------: | ------------: |
| `lotus-model`           |     821 |      19 |       97.69% |
| `lotus-curation`        |   1,984 |      72 |       96.37% |
| `lotus-query`           |   1,098 |      41 |       96.27% |
| `lotus-jsonld`          |   1,447 |      83 |       94.26% |
| `lotus-search`          |   1,119 |     263 |       76.50% |
| `lotus-cli`             |     943 |     510 |       45.92% |
| `lotus-web-assets`      |     948 |     627 |       33.86% |
| **Total**               | **17,286** | **6,420** | **62.86%** |

The four areas the brief names are all above 94%: model 97.69%, curation 96.37%,
query parsing and building 96.27%, jsonld 94.26%.

`lotus-web-assets` is the real gap at 33.86% and 627 missed regions, the largest
absolute hole in the tree. It rewrites HTML and JavaScript on the way into the
bundle, and a silent break there surfaces as a blank page in a deploy rather than
a failing test. Recorded as this pass's coverage target.

The many `0.00%` files in the app are Dioxus `#[component]` functions. They
execute in a browser and in no test, which is a property of the framework
rather than a gap worth closing here.

---

## Known issues carried into this pass

- `lotus-query/src/query.rs` at 8% line coverage (§4 target).
- Desktop styling: `dx` generates `public/assets/lotus-explore.css` but omits it
  from the desktop bundle, so `--features desktop` runs unstyled. Deferred by
  explicit instruction.
- 15 duplicated crates in the dependency graph, all allowed in `deny.toml` with
  a documented reason (§3).

## Decisions not acted on

Recorded here rather than left silent, per the brief.

- **`docs/REFACTOR_PLAN.md` is not being restored.** It was deleted on purpose
  once the plan it described was finished, and the architecture it explained now
  lives in `docs/ARCHITECTURE.md`. Restoring a finished plan as a
  documentation-consistency fix would undo a deliberate decision.
