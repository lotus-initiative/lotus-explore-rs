# Sweep log

One line per item: time, commit, item, result. Times are local, 2026-10-03.

| Time | Commit | Item | Result |
|---|---|---|---|
| 21:52 | — | baseline: branch `sweep/overnight` from `8a68da1`, rustc 1.99.0 | 8 cores / 16 GB / 13–17 GiB free, load avg 25–42 |
| 21:55 | — | baseline: tests | 1224 web / 631 server / 590 desktop / 6 doc, 0 failed. **1271 not reproducible** — see BASELINE.md |
| 22:00 | — | baseline: gates | fmt green, clippy native green, **clippy wasm RED at `8a68da1`** |
| 22:05 | — | baseline: wasm size | raw 1,673,150 B · `-Oz` −0.02% · gzip 679,349 B · br 527,152 B |
| 22:07 | — | baseline: compile time | clean 43.39 s · incremental 5.00 s (single samples, load ~28) |
| 22:10 | `62dd958` | C (wasm gate) | **fixed** `open_sink`'s unused `format` param; `lint-wasm` green, all builds still pass, no test edits |
| 22:14 | — | baseline: runtime + memory | parse+build 2049/3869/5523 ms and retained 87.9/171.4/254.2 MB at 1M/2M/2.99M rows || 22:40 | — | A: query matrix | 12 cells enumerated; `*` vs absent found identical. See below |
| 22:55 | `a1f0e42` | A: matrix tests + `*` fix | **behaviour change**: `*` now requires `P703`. 1224→1231 web, 631/590 unchanged |
