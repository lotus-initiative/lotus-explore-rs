# Sweep log

One line per item: time, commit, item, result. Times are the real local clock,
taken from the commit dates rather than estimated — an earlier draft of this
file had estimated times and they were wrong.

## Session one

| Time | Commit | Item | Result |
|---|---|---|---|
| 21:52 | — | baseline: branch `sweep/overnight` from `8a68da1`, rustc 1.99.0 | 8 cores / 16 GB, load 25–42, 13–17 GiB free |
| 21:58 | — | baseline: tests | 1224 web / 631 server / 590 desktop / 6 doc. **1271 not reproducible** |
| 22:00 | — | baseline: gates | fmt green, clippy native green, **clippy wasm RED at `8a68da1`** |
| 22:05 | — | baseline: wasm size | raw 1,673,150 B · `-Oz` −0.02% · gzip 679,349 B · br 527,152 B |
| 22:07 | — | baseline: compile time | clean 43.39 s · incremental 5.00 s |
| 22:10 | — | baseline: runtime + memory | parse+build 2049/3869/5523 ms, retained 87.9/171.4/254.2 MB |
| 22:03 | `62dd958` | C — wasm gate | **fixed** `open_sink`'s unused `format`; `lint-wasm` green |
| 22:12 | `888d34e` | baseline docs | three gaps recorded as *not measured* |
| 22:19 | `d07b061` | A: matrix + `*` fix | **behaviour change**: `*` now requires `P703`. 1224→1231 web |
| 22:22 | `7beb904` | B: SPARQL generation bench | median 9.8 µs/query, cell medians 7.5–15.9 µs, 30 samples |
| 22:23 | `bca6b1a` | A/D: correct the empty-taxon bound | my own claim was wrong — the 500-row cap was deliberately removed |
| 22:25 | `68abea2` | D: FAQ | +3 entries × 4 locales |
| 22:33 | — | E: mutation, query builder | 115 mutants, **0 missed** (6 min) |
| 22:41 | — | E: mutation, parsing | 79 mutants, **2 missed** (3 min) |
| 22:42 | `116edac` | E: kill the 2 parsing mutants | 2→**0 missed** |
| 23:05 | — | E: mutation, dedup/count | 368 mutants, 103 missed, 5 timeouts (23 min) |
| 23:37 | `7400fc1` | E: kill 23 columnar mutants | 103→80 missed; found the compound-filter bug |
| 23:38 | `513e77c` | E: MUTANTS.md | full accounting |
| 23:41 | `a8f88c5` | E: cache-key attempt | not the mutants.toml exclusion; the suite is not hermetic |
| 23:42 | `b1aabd3` | handoff | SUMMARY.md |

## Session two — mutants, the cache-key bench, cleanup

| Time | Commit | Item | Result |
|---|---|---|---|
| 08:57 | `177221b` | B: cache-key bench | median 1.8–3.1 µs; hex is a flat ~20% of cost. Not optimised |
| 09:02 | `1494ce6` | fix: `lotus-search` tests | `cargo test -p lotus-search` did not compile alone; dev-dep on self |
| 09:30 | `106cd8c` | E: lotus-search ×4 modules | 145 mutants, **52 missed → 20**. Found the duplicated retry rule |
| 10:38 | `fbbcf01` | E: export_rows | 66 mutants, **16 missed → 2**. Found an untested JSON separator |
| 10:39 | `39b3e11` | E: MUTANTS.md | 29 module runs, ~1,300 mutants, every survivor accounted for |
| — | — | E: lotus-curation ×4 | 122 mutants, **0 missed** |
| — | — | E: lotus-jsonld ×4 | 68 mutants, **0 missed** |
| — | — | E: lotus-cli ×2, lotus-web-assets ×3 | 115 mutants, **0 missed** |
| 10:49 | `61e0383` | C: server dispatch | **behaviour change**: API no longer answers a blank taxon box narrowly |
| 10:51 | `a0fff9f` | C: two duplicate helpers | removed `sanitize_taxon_input` and a second `is_qid` |
| 10:55 | `707f543` | D: FRONTENDS.md | the API is a fourth front end, and the rule it broke |
| 10:58 | `68d6a8d` | sweep write-up | three findings for a human, one of them against this sweep |
| — | — | final gates | fmt / clippy native / clippy wasm / docs_in_sync all pass |

## Cumulative gate movement

| | Session one start | Now |
|---|---|---|
| web tests | 1224 | **1265** |
| server tests | 631 | **634** |
| desktop tests | 590 | 590 |
| doctests | 6 | 6 |
| modules mutation tested | 2 | **29** |
| mutants run | 194 | **~1,300** |

No test was ever deleted, weakened or edited to make something pass. Every
number went up or stayed flat.
