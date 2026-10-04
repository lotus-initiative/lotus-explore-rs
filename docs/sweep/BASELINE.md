# Sweep baseline

Recorded on `sweep/overnight` before any change except the one wasm-gate fix
noted under "Gates". Everything here is a measurement, not an estimate.
Reproduce with the commands quoted next to each number.

## Machine and toolchain

| | |
|---|---|
| `git rev-parse HEAD` at baseline | `8a68da141f0ae697289379a3ca62aee447ee315f` (on `main`) |
| Branch | `sweep/overnight`, branched from the commit above |
| `rustc --version` | `rustc 1.99.0 (b940084d7 2026-09-28)` |
| `cargo --version` | `cargo 1.99.0 (5f94df478 2026-08-27)` |
| `rustfmt --version` | `rustfmt 1.10.0-stable (b940084d7 2026-09-28)` |
| Host | macOS, `aarch64-apple-darwin`, 8 cores, 16 GB RAM |
| Free disk at baseline | 13–17 GiB (volume sits at 97–98% full) |
| Load average during the session | 25–42, sustained (see "Measurement validity") |

## Test count

The authoritative command is the repo's own, `./mk test`, which is
`cargo nextest run` over three separate builds plus the doctests. It is used
in preference to a bare `cargo test --workspace`, because a bare workspace run
only builds the browser client and silently skips the `server` and `desktop`
builds (see the comment on `[tasks."test"]`).

| Build | Command | Result |
|---|---|---|
| browser client | `cargo nextest run --workspace --locked --no-fail-fast` | **1224 passed**, 3 skipped, 0 failed |
| HTTP API | `cargo nextest run -p lotus-explore-rs --features server --locked --no-fail-fast` | **631 passed**, 0 failed |
| native window | `cargo nextest run -p lotus-explore-rs --features desktop --all-targets --locked --no-fail-fast` | **590 passed**, 0 failed |
| doctests | `cargo test --workspace --doc --locked --quiet` | **6 passed** (1 each for curation/jsonld/model/search, 2 for query) |

The 3 "skipped" are the `#[ignore]`d benchmark entry points in
`crates/lotus-query/tests/bench.rs`.

### The 1271 figure could not be reproduced

The brief states 1271 passing tests. At this commit the largest single
number available is 1224, and no combination of the four runs above sums to
1271 (1224 + 6 doctests = 1230; adding the 3 ignored gives 1233). The
difference is most likely a count taken with a different feature set, a
different harness, or a different commit — the doctests in particular are
run by `cargo test --doc` and not by nextest, so a nextest-only figure and a
`cargo test` figure do not agree.

I could not reproduce it, so I am not claiming it. **The gate enforced for the
rest of the sweep is "no test count below the numbers in the table above",
per build**, rather than a number I cannot account for. Flagged in
`REVIEW.md` for a human to confirm what 1271 was measured from.

## Gates at baseline

| Gate | Command | Baseline status |
|---|---|---|
| format | `./mk fmt-check` | **green** |
| clippy, native | `./mk lint` (`--workspace --all-targets --all-features -- -D warnings`) | **green** |
| clippy, wasm32 | `./mk lint-wasm` (pure crates, `lotus-search` no-default-features, app) | **RED at `8a68da1`** |
| tests | `./mk test` | green (counts above) |
| wasm test target compiles | `./mk check-wasm` / `lint-wasm-app` | green |
| docs in sync | `./mk test` → `-p lotus-cli --test docs_in_sync` (5 tests) | **green** |

### The one red gate, and why it was fixed first

`./mk lint-wasm` failed on `main` before this sweep changed anything:

```
error: unused variable: `format`
   --> apps/lotus-explore-rs/src/download/wasm.rs:118:20
    |
118 | async fn open_sink(format: DownloadFormat, filename: &str, rows: usize) -> Sink {
```

`open_sink` never read its `format` parameter: the caller passes `format`
separately to `finish(..)` for the content type, and the filename already
carries the extension. Native builds compile a different `download` module, so
only the wasm lint saw it. Every wasm-related change is gated behind this
command, so it was fixed before anything else — commit `62dd958`, no test
edits, all four builds still green.

## WASM size

Measured on the bundle from `./mk web-build` (the shipped artefact, not a
hand-rolled `cargo build`).

| Measurement | Value |
|---|---|
| raw `.wasm` | **1,673,150 B** (1,633.9 KiB) |
| `wasm-opt -Oz` | **1,672,853 B** (1,633.6 KiB; −297 B, **−0.02%**) |
| `gzip -9` | **679,349 B** (663.4 KiB) |
| `gzip -9` after `-Oz` | 678,913 B (662.9 KiB; −436 B) |
| brotli (`.br`, pre-compressed by `dx`) | **527,152 B** (514.8 KiB) |
| JS glue | 51,597 B raw / 11,956 B br |

```bash
./mk web-build
out=target/dx/lotus-explore-rs/release/web/public
w=$(ls $out/assets/*_bg-*.wasm)
stat -f%z "$w"; gzip -c9 "$w" | wc -c; stat -f%z "$w.br"
wasm-opt -Oz -o /tmp/base_oz.wasm "$w" && stat -f%z /tmp/base_oz.wasm
```

`wasm-opt -Oz` buys 0.02%. `dx` already runs an equivalent pipeline, so there
is no free win here and no reason to add `wasm-opt` to the build.

### `twiggy top -n 20`

```
 Shallow Bytes │ Shallow % │ Item
───────────────┼───────────┼─────────────────────
         76917 ┊     4.60% ┊ data[110]
         51896 ┊     3.10% ┊ code[715]
         40199 ┊     2.40% ┊ data[24]
         22341 ┊     1.34% ┊ data[1975]
         22142 ┊     1.32% ┊ code[0]
         19760 ┊     1.18% ┊ code[1]
         18269 ┊     1.09% ┊ code[45]
         17171 ┊     1.03% ┊ data[44]
         14917 ┊     0.89% ┊ code[18]
         13448 ┊     0.80% ┊ code[2]
         13225 ┊     0.79% ┊ code[3]
         11892 ┊     0.71% ┊ code[1306]
         11541 ┊     0.69% ┊ code[5]
         11341 ┊     0.68% ┊ code[6]
         10488 ┊     0.63% ┊ code[7]
         10170 ┊     0.61% ┊ code[8]
         10058 ┊     0.60% ┊ code[2269]
          9714 ┊     0.58% ┊ code[9]
          9691 ┊     0.58% ┊ code[10]
          9338 ┊     0.56% ┊ code[11]
      1268632 ┊    75.82% ┊ ... and 6836 more.
      1673150 ┊   100.00% ┊ Σ [6856 Total Rows]
```

Every row is anonymous. The module is built with `debug = false`, so there are
no symbol names for `twiggy` to attribute to a crate or a function, and
`twiggy top` 0.8.0 has no `--features`/crate mode. Crate-level attribution
needs `cargo bloat`, which is **not installed**; installing it on a volume at
97% full was judged a worse trade than reporting the gap. `cargo tree -d` for
duplicate dependencies is the cheap substitute and is the number to trust for
the dependency audit. Listed under "what I could not measure".

## Compile time

| Measurement | Command | Result |
|---|---|---|
| clean, whole workspace | `CARGO_TARGET_DIR=<tmp> cargo build --workspace --locked --timings` | **43.39 s** wall, 159.73 s user, 33.40 s sys |
| incremental, no changes | `cargo build --workspace --all-targets --locked` | **5.00 s** wall, 2.35 s user, 2.71 s sys |

The clean figure is a single sample taken under load average ~28 on 8 cores.
Treat it as an order of magnitude, not a regression baseline: anything under
about ±20% is inside this machine's noise. `--timings` HTML lands in
`target/cargo-timings/cargo-timing.html` (this cargo has no `--timings-file`).

No `cargo clean` was run against the main `target/` at any point. The clean
measurement used a throwaway `CARGO_TARGET_DIR` under the temp directory,
which was deleted immediately afterwards (it cost 1.0 GB of a volume that has
13 GB free).

## Runtime

The repo has **no criterion**; it has a hand-rolled harness in
`crates/lotus-query/tests/bench.rs` that prints its own numbers and asserts
nothing, on the stated principle that "a benchmark that fails on a slow
machine is a test that gets deleted". criterion was **not** added: it is a new
dependency and a second convention in a workspace that deliberately has one,
and it would have to be argued for against the repo's own documented
reasoning. The gap it leaves is recorded honestly below.

```bash
cargo test -p lotus-query --release --locked -- --ignored --nocapture bench
```

Fixture: generated, not recorded, at the fill rates measured against a real
QLever export (20.7% inchikey, 15.0% SMILES, 10.3% mass, 8.3% formula, ~1.2
rows per compound).

### Parsing and aggregation (single samples — see caveat)

| Rows | CSV | parse + build | of which build | throughput | resident | B/row |
|---|---|---|---|---|---|---|
| 1,000,000 | 291 MB | **2049.1 ms** | 1424.1 ms | 135.5 MB/s | **87.9 MB** | 92.1 |
| 2,000,000 | 583 MB | **3869.3 ms** | 2505.3 ms | 143.8 MB/s | **171.4 MB** | 89.9 |
| 2,990,730 | 873 MB | **5522.7 ms** | 3516.0 ms | 150.7 MB/s | **254.2 MB** | 89.1 |

The widest search is the one the module documents: 2,990,730 rows, ~873 MB of
CSV, held in 254 MB — 31% of the CSV. Interning is already doing its job:
taxon names are 400 entries / 1.5 MB against 2,990,730 rows, and reference
DOIs 91,706 entries / 1.9 MB.

### Query planning and scanning (single samples)

| Operation | 1M rows | 2M rows |
|---|---|---|
| plan text filter (dictionary scan) | 127.2 ms | 257.4 ms |
| scan rows (all survive) | 14.7 ms | 31.0 ms |
| plan mass filter (100k survive) | 1.0 ms | 2.8 ms |
| sort by name | 8.6 ms | 840.9 ms |

`sort by name` at 2M rows (840.9 ms) against 1M rows (8.6 ms) is a 98x jump for
2x the data and does **not** look like a real property of the sort. It is much
more likely the machine (see below) or a threshold in the sort. Worth a human
look; not claimed as a finding.

### Memory

`dhat` is **not installed**, and neither is an equivalent allocation counter.
What exists is `resident_bytes()` inside `bench.rs`, which reports the resident
size of the built `ColumnarResultSet` — real numbers, but only the *retained*
structure, not peak process RSS and not an allocation count.

| Rows | peak bytes recorded | allocation count |
|---|---|---|
| 1,000,000 | 87.9 MB retained | not measured |
| 2,000,000 | 171.4 MB retained | not measured |
| 2,990,730 | 254.2 MB retained | not measured |

Peak *process* RSS and total allocations are therefore **not measured**. Any
memory claim later in this sweep must say "retained", not "peak", unless
`dhat` gets installed.

### SPARQL generation (added after this baseline, `7beb904`)

```bash
cargo test -p lotus-search --release --test matrix -- --ignored --nocapture bench_matrix
```

30 samples per cell, median and spread.

| | |
|---|---|
| median of the twelve cell medians | **9,833 ns** (9.8 µs) |
| spread of the cell medians | **7,541 ns .. 15,917 ns** |
| slowest cell's median ÷ fastest | 2.1x |
| worst within-cell max ÷ that cell's median | up to 140x |

The finding is the spread. The difference *between* cells is inside the noise, and
~10 µs against a network round trip of hundreds of milliseconds is ~0.001% of the
work. Recorded so nobody spends a night proving it.

### Cache-key construction (added after this baseline, `177221b`)

```bash
cargo test -p lotus-explore-rs --release --bin lotus-explore-rs -- --ignored --nocapture bench_cache_key
```

50 samples per case. Two runs, because absolute values move with load on this host
(load average 15.5 for the first, lower for the second):

| case | bytes | run 1 | run 2 |
|---|---|---|---|
| tiny (a lookup) | 208 | 1,417 ns | 708 ns |
| everything, occurrence optional | 2,221 | 2,792 ns | ~1,500 ns |
| one taxon (matrix cell 5) | 2,642 | 3,125 ns | 1,833 ns |
| structure search | 2,210 | 2,833 ns | ~1,500 ns |
| synthetic 16x | 42,272 | 31,208 ns | 20,333 ns |

Absolute numbers move by ~1.7x between runs; the shape does not. sha256 is
67–73% of the cost and scales with query length; the hex encoding is ~375–750 ns
**flat**, because its input is always a 32-byte digest; the prefix `format!` is
the remaining 7–11%.

### What the runtime baseline still does not cover

- **Peak RSS and allocation count.** `dhat` is not installed; see Memory above.
- **Crate-level wasm attribution.** `cargo bloat` is not installed; see WASM size
  above.
- **Criterion.** The single-sample harnesses are as described above; the two
  benches added afterwards do take 30 and 50 samples with medians.

## Measurement validity — read this before trusting a delta

Two facts about this host make single-shot timing unreliable, and both are
recorded so that a later reader does not mistake noise for a regression.

1. **Load average is 25–42 on 8 cores, and it is not the build.** `ps` during
   the session attributes it to five `CoreServices/Metadata.framework`
   processes (Spotlight) at 15–30% each, plus `WindowServer` at ~90%. Writing
   millions of files into `target/` triggers Spotlight indexing of `target/`,
   which is both the load and, most likely, the disk pressure: this host is a
   desktop with a graphical session, not a build server, and the volume is at
   97–98% full. Adding `target/` to Spotlight's privacy list is the fix and
   is **not** done here — it changes machine configuration, which is a human's
   call, not an unattended sweep's. It is the single highest-value thing a
   human could do for future measurement work.
2. **A mass deletion of `target/debug` happened mid-session.** The first
   `cargo test --workspace` failed with `could not execute process
   .../doc_links-... (never executed)`, and `target/debug` turned out to have
   been deleted outright while `target/wasm32-unknown-unknown` survived. The
   rebuild then failed twice more with `extern location for ... does not
   exist` for `proc_macro2` and `dunce` — stale fingerprints pointing at files
   that were gone. A probe file written into `target/` survived 30 s, so this
   is not a blanket reaper of the directory; the most likely cause is APFS
   reclaiming space under pressure. **Consequence:** the first test run of the
   session reported a failure that was not a code defect, and had the space
   reclaim continued, build artefacts would keep vanishing mid-build. Free
   disk is watched before every heavy step for this reason.

Because of (1), **the noise floor for a wall-clock number on this host is
large**. A change counts as an improvement only if it moves a number well
beyond that spread; sub-20% wall-clock deltas on a single sample are treated
as noise and reverted.

## Deviations from the brief, and why

- **`ionice -c 3` is not used.** It does not exist on macOS. Every heavy run
  is wrapped in `nice -n 19` instead. `ulimit -v` capping is likewise not
  applied to native builds: the allocator reserves address space far beyond
  what it uses, so a 50%-of-RAM virtual cap makes rustc fail spuriously. It is
  applied nowhere rather than applied wrongly.
- **Gates are the repo's `./mk` tasks, not the commands in the brief.** The
  repo's are stricter (`--all-features` on clippy; `server` and `desktop`
  linted and tested separately), so they subsume the brief's. `mk` exists
  because `cargo make` with workspace support re-runs every task eight times,
  once per crate.
- **No criterion benches were added**, for the reason under "Runtime".
