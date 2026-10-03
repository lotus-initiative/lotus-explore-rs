# Sweep log

One line per item: time, commit, item, result. Times are the real local clock
(2026-10-03), taken from the commit dates rather than estimated — an earlier
draft of this file had estimated times and they were wrong.

| Time | Commit | Item | Result |
|---|---|---|---|
| 21:52 | — | baseline: branch `sweep/overnight` from `8a68da1`, rustc 1.99.0 | 8 cores / 16 GB, load avg 25–42, 13–17 GiB free |
| 21:58 | — | baseline: tests | 1224 web / 631 server / 590 desktop / 6 doc, 0 failed. **1271 not reproducible** — see BASELINE.md |
| 22:00 | — | baseline: gates | fmt green, clippy native green, **clippy wasm RED at `8a68da1`** |
| 22:05 | — | baseline: wasm size | raw 1,673,150 B · `-Oz` −0.02% · gzip 679,349 B · br 527,152 B |
| 22:07 | — | baseline: compile time | clean 43.39 s · incremental 5.00 s (single samples, load ~28) |
| 22:10 | — | baseline: runtime + memory | parse+build 2049/3869/5523 ms, retained 87.9/171.4/254.2 MB at 1M/2M/2.99M rows |
| 22:03 | `62dd958` | C (wasm gate) | **fixed** `open_sink`'s unused `format` param; `lint-wasm` green, all builds pass, no test edits |
| 22:12 | `888d34e` | baseline docs | BASELINE.md + REVIEW.md; three gaps recorded as *not measured* |
| 22:19 | `d07b061` | A: matrix + `*` fix | **behaviour change**: `*` now requires `P703`. 1224→1231 web; 631/590 unchanged |
| 22:22 | `7beb904` | B: SPARQL generation bench | baseline added: median 9.8 µs/query, cell medians 7.5–15.9 µs, 30 samples |
| 22:23 | `bca6b1a` | A/D: correct the empty-taxon bound | my own claim was wrong — the 500-row cap was deliberately removed |
| 22:25 | `68abea2` | D: FAQ entries | +3 entries × 4 locales: structure-only, wildcard-taxon, empty-taxon-box |
| 22:33 | — | E: mutation, query builder | 115 mutants: 88 caught, 27 unviable, **0 missed** (6 min) |
| 22:41 | — | E: mutation, parsing | 79 mutants: 46 caught, 31 unviable, **2 missed** (3 min) |
| 22:42 | `116edac` | E: kill the 2 parsing mutants | `\|\|`→`&&` in `Columns::resolves_any`; 46→48 caught, **0 missed** |
| 23:05 | — | E: mutation, dedup/count | 368 mutants: 233 caught, 27 unviable, **103 missed**, 5 timeouts (23 min) || 23:05 | — | E: mutation, dedup/count | 368 mutants: 233 caught, 27 unviable, 103 missed, 5 timeouts (23 min) |
| 23:52 | — | E: kill 23 columnar mutants | +6 tests; 233→256 caught, **103→80 missed**. Found a real compound-filter bug |
| 23:52 | `7400fc1` | E: kill 23 columnar mutants | +6 tests; 103→80 missed; compound-filter bug written up, not fixed blind |
| 23:38 | `513e77c` | E: MUTANTS.md | full accounting: 80 survivors grouped, 1 equivalent, 3 needing a human |
| 23:41 | `a8f88c5` | E: cache-key attempt | not the mutants.toml exclusion; the suite is not hermetic. Corrected |
| 23:50 | — | final gates | fmt/clippy/clippy-wasm/docs_in_sync all pass; 1239 web, 631 server, 590 desktop |
| 23:52 | `b1aabd3` | handoff | SUMMARY.md: commit table, one behaviour change called out, what was not measured |
