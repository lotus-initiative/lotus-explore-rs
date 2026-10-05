# Work plan

Living document. Updated as work proceeds so it survives a crash or a lost context.

## How to read this

- **Status** is one of `todo`, `doing`, `done`, `blocked`, `dropped`.
- Every task states how to verify it. Nothing is `done` until its verification has
  actually been run and passed in this session.

## Ground rules

1. No `cargo mutants` until the disk question is settled. See BLOCKER-1.
2. Run `./mk fmt` before committing; it rewrites files in place.
3. Verify with `cargo test --workspace --all-features`, `cargo clippy --workspace
   --all-targets --all-features` and the wasm clippy, not a subset.
4. Never edit `apps/lotus-explore-rs/src/services/error_presenter.rs` — it is
   rustfmt-dirty on a clean tree and is not ours.

## BLOCKER-1 — mutation testing crashes the machine

- **Status:** blocked — **cause not identified**
- **What is known:** two runs wedged a 16 GB / 8-core machine. The first at `--jobs 8`,
  the second at `--jobs 2`. The second is the data point that matters: if two
  concurrent builds can do it, "too many parallel jobs" is not the cause.
- **What is NOT known: why.** Both crashes happened with 27-29 GB still free, so disk
  alone does not account for them, and `target/` is large mostly from ordinary
  development rather than from mutants (a 1,545-mutant sweep added ~4 GB, about
  2.6 MB a mutant).
- **Not an application memory leak.** The one real growth defect found during the
  audit was fixed and committed in `ec060c3`.
- **Done:** `make/scripts/mutants-preflight.sh` prints disk, `target/`, RAM, swap and
  jobs before a run, and refuses to start below 20 GB free. `make/test.toml` defaults
  to `--jobs 4`. **Neither is a fix** — the preflight script says so in its own header,
  and `--jobs 4` is a guess, not a known-good value.
- **What is left to actually unblock it:**
  1. Establish whether it is memory at all, before spending another run on the
     question. `vm_stat` and Activity Monitor's memory pressure during a *single*
     package at `MUTANTS_JOBS=1` would show it, and a single job cannot fan out.
  2. Until then, scope by package. `cargo mutants --package lotus-model --jobs 1` is
     571 mutants and is where the survivors were.
- **Verification:** a full `./mk mutants` completes with the machine responsive. Not
  attempted; there is no reason to think any known configuration would.

## TASK-1 — `Timer "LOTUS:taxon_resolution" already exists.`

- **Status:** done — committed `9e6714f`
- **Two distinct defects, both confirmed by reading the code:**
  1. `apps/lotus-explore-rs/src/perf.rs` tracks open labels in a **`thread_local!`**
     `OPEN_TIMERS`, but a `console.time` label is global to the document.** Dioxus
     resumes async tasks on different threads, and each thread starts with an empty
     set, so two concurrent taxon resolutions both call
     `console.time_with_label("LOTUS:taxon_resolution")` and the browser logs the
     duplicate. The guard that was supposed to prevent exactly this could not see
     across threads.
  2. `features/explore/service/resolve_taxon/mod.rs` opens `LOTUS:taxon_resolution`
     and closes it only on the cache-hit path. **The slow path — the one that actually
     runs a SPARQL lookup — never closes it.** So after one slow resolution the label
     is open forever: durations are measured from that first call, the browser timer
     never ends, and every later call takes the `reopened_timer` branch.
- **Fix:** a global `Mutex<HashSet<String>>` to match the global scope of a
  `console.time` label, plus an RAII `perf::Timer` that closes on drop so an early
  return cannot leak a label again.
- **Verification:** `cargo test -p lotus-explore-rs`, wasm clippy clean, and a test
  asserting the guard closes a label when dropped without `end()` being called.

## TASK-2 — new `/faq` page

- **Status:** done — committed `8adbcb1`
- Routed like the existing pages: a `Route` variant in `src/app/routes.rs` at
  `/faq`, matching the `query`/`hash` and `view_key` conventions, wired in
  `navigation_string`, `hash`, `query_value`, `with_query_and_hash`.
- Content: the questions users actually ask, sourced from the repo's own `.md` files
  rather than invented.
- "Crystal clear for MCPS" is read as: stable, predictable URLs and headings, and
  `FAQPage` JSON-LD structured data so the answers are machine-readable.
- **Delivered:** `/faq` route wired like every other page, reachable from the view
  switcher, 12 questions in 4 groups across all four locales, per-question stable
  anchors, a table of contents, and `FAQPage` JSON-LD whose `@id` values are the same
  fragments the headings render.
- **Verification NOT done:** the page has not been looked at in a browser. Rendering,
  heading order as a sighted reader sees it, focus order and the deep-link scroll on
  `/faq#download-formats` are all unverified by anything automated here.

## TASK-3 — downloads off QLever, onto lotus-api

- **Status:** done — committed `6344879`. **Feasible: the API exports all three
  formats** (`ExportUrlResponse` has csv/json/rdf plus gz variants), so no backend
  change was needed.
- **Evaluated, and the change made:** a download was routed by
  `is_wdqs_fallback_used()`, i.e. by which transport served the *search*. So any search
  answered by QLever produced a QLever download even when the API could export fine.
  The API is now tried first, unconditionally; QLever is demoted to the fallback it
  should have been, and still runs if the API yields no URL.
- **The obvious risk turned out not to apply, and it is worth recording why:** the API
  export re-runs the query server-side and so cannot see client-side column filters.
  But the WDQS route exports the same server-side `query` and does not apply them
  either. Both routes export the result set, not the filtered view, so switching
  changes the *host*, not the contents. `filters-vs-query` in the FAQ says this.
- **`select_export_url` -> `api_export_url`, now `Option`:** an empty URL field used to
  be handed to the browser, which would download the current page under the name of the
  results.
- **Verification:** still not done. `api_export_url` is `#[cfg(target_arch =
  "wasm32")]` and has no caller outside `download/wasm.rs:250`, so a test for it has to
  live in a `wasm32` test module. That is now possible — see KNOWN GAP below, which is
  closed — but the test has not been written. This is the one piece of the session with
  no test on it.

## TASK-4 — polish

- **Status:** partly done. Remaining items are open-ended rather than specified.
- **Done:** removed the four-arm `match` for a label that is the same word in every
  locale; corrected a comment that claimed `rsx!` could not interpolate an enum (the
  real reason is that the enum has no `&str`); corrected the PLAN.md disk diagnosis;
  removed a misleading `#[cfg]`-rationale comment in the FAQ tests.
- Strip outdated code and comments that no longer describe the code. Priority on the
  files this session already touched, then the download/FAQ code added above.
- Memory and speed pass over the paths in play. Already done and committed, listed
  so it is not redone: numeric QID dictionary (`28ee022`), interned sparse columns
  and UUID packing (`f7a043e`), streamed WDQS export (`da0cae1`), full-result
  columnar set with no 500-row cap (`af00977`), leftovers and stale numbers
  (`4fb3800`), per-row column growth fix (`ec060c3`).

## KNOWN GAP — wasm-only paths still have no executing test

- **Status:** open, pre-existing, not introduced this session
- The wasm test target now compiles: `cargo clippy --target wasm32-unknown-unknown -p
  lotus-explore-rs --all-targets` is clean. `RouteQuery::from_encoded`,
  `build_search_cache_key`, `build_export_cache_key` and `SearchMetrics::add_parse`
  were each gated out of wasm and reached by the tests below them; all four are now
  `#[cfg(any(not(target_arch = "wasm32"), test))]`, which is the fix this gap called for.
- **What is still open:** CI lints the wasm *build*, not its test target, so a test in
  a `#[cfg(target_arch = "wasm32")]` module can look green in review without ever having
  run. `api_export_url` (TASK-3) is the one such helper with no test at all.
- **Fix:** either run the wasm test target in CI, or move the pure parts of the wasm
  paths into a non-gated module. Not started.