# Work plan

Living document. Updated as work proceeds so it survives a crash or a lost context.

Last updated: start of session.

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

- **Status:** blocked (deliberately not being worked on right now)
- **Cause, measured not guessed:** 8 cores, 16 GB RAM, **1 GB swap cap**, volume at
  **94% full with 27 GB free**, `target/` at **24 GB** (`target/debug` alone 19 GB).
  `cargo mutants` rebuilds into `target/debug` once per mutant; 1545 then 571 mutants
  against that tipped the volume and the 1 GB pagefile could not absorb it.
- **Not an application memory leak.** The one real growth defect found during the
  audit was fixed and committed in `ec060c3`.
- **To unblock, in order:**
  1. Free disk. `target/` is fully regenerable; a full `cargo clean` returns ~24 GB.
     **This is destructive to incremental build state and needs the user's go-ahead.**
  2. Lower `--jobs` in the `mutants` task in `make/test.toml` from 8. 8 concurrent
     `rustc` on a 1 GB swap is the other half of the problem.
  3. Add a free-space preflight to that task that refuses to start under a
     threshold, so it cannot silently fill the disk again.
- **Verification:** `./mk mutants` completes with the machine responsive throughout.
  Not attempted until 1–3 are done.

## TASK-1 — `Timer "LOTUS:taxon_resolution" already exists.`

- **Status:** doing
- **Two distinct defects, both confirmed by reading the code:**
  1. `apps/lotus-explore-rs/src/perf.rs` tracks open labels in a **`thread_local!`**
     `OPEN_TIMERS`, but a `console.time` label is global to the document.** Dioxus
     resumes async tasks on different threads, and each thread starts with an empty
     set, so two concurrent taxon resolutions both call
     `console.time_with_label("LOTUS:taxon_resolution")` and the browser logs the
     duplicate. The guard that was supposed to prevent exactly this could not see
     across threads.
  2. `features/explore/service/resolve_taxon/mod.rs` opens `LOTUS:taxon_resolution`
     at line 81 and closes it only on the cache-hit path (line 92). **The slow path
     — the one that actually runs a SPARQL lookup — never closes it.** So after one
     slow resolution the label is open forever: durations are measured from that
     first call, the browser timer never ends, and every later call takes the
     `reopened_timer` branch.
- **Fix:**
  1. Make `OPEN_TIMERS` a global `Mutex<HashSet<String>>` so it matches the global
     scope of a `console.time` label.
  2. Add an RAII timer guard in `perf.rs` that closes on drop, so an early return
     cannot leak a label again, and use it in `resolve()`.
- **Verification:** `cargo test -p lotus-explore-rs`, wasm clippy clean, and a test
  asserting the guard closes a label when dropped without `end()` being called.

## TASK-2 — new `/faq` page

- **Status:** todo
- Routed like the existing pages: a `Route` variant in `src/app/routes.rs` at
  `/faq`, matching the `query`/`hash` and `view_key` conventions, wired in
  `navigation_string`, `hash`, `query_value`, `with_query_and_hash`.
- Content: the questions users actually ask, sourced from the repo's own `.md` files
  rather than invented.
- Accessibility and structure are a hard requirement, not a nice-to-have: landmarks,
  one `h1`, disciplined heading order, real `<details>`/summary or equivalent for
  disclosure, `lang`, keyboard reachable, and a skip link if the shell has one.
- "Crystal clear for MCPS" is read as: stable, predictable URLs and headings, and
  `FAQPage` JSON-LD structured data so the answers are machine-readable.
- **Verification:** route parsing tests mirroring the existing ones in
  `src/app/routes.rs`; a test that every route renders; the full workspace gates.

## TASK-3 — downloads off QLever, onto lotus-api

- **Status:** todo
- **Goal:** CSV / RDF / JSON download should use the lotus-api export endpoints
  instead of routing through QLever, if feasible.
- **Must be evaluated before it is built.** Known already, to be confirmed against
  the code rather than assumed:
  - The WASM download path already prefers API export URLs when `api_base` is
    configured, so this may be partly in place.
  - The WDQS fallback is the QLever-shaped path and is what would change.
  - **The open risk to state plainly:** the API export re-runs a query server-side,
    so it cannot see *client-side column filters*. If there is no
    "export the current result set" endpoint, switching routes changes what a
    filtered download contains. That must be surfaced to the user, not silently
    traded away.
- **Verification:** tests for the URL selection logic in both directions (API
  configured and not), and that the fallback still works.

## TASK-4 — polish

- **Status:** todo
- Strip outdated code and comments that no longer describe the code. Priority on the
  files this session already touched, then the download/FAQ code added above.
- Memory and speed pass over the paths in play. Already done and committed, listed
  so it is not redone: numeric QID dictionary (`28ee022`), interned sparse columns
  and UUID packing (`f7a043e`), streamed WDQS export (`da0cae1`), full-result
  columnar set with no 500-row cap (`af00977`), leftovers and stale numbers
  (`4fb3800`), per-row column growth fix (`ec060c3`).

## Session log

- Found and fixed the per-row sparse-column growth defect → committed `ec060c3`
  (1221 tests passing, clippy clean native and wasm at that point).
- Two `cargo mutants` runs crashed the machine. Cause measured (see BLOCKER-1).
  All mutation processes killed; nothing running now.