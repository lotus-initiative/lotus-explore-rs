# Contributing

## Before you start

Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) first. The workspace is
arranged so that a change usually has an obvious home, and that arrangement is
load-bearing: the web app and the CLI share their query building and their
curation vocabulary precisely so that a query the explorer runs and a query
`lotus search` runs are the same query.

## Running the checks

```bash
./mk setup   # once: installs every tool, at a pinned version
./mk ci      # every check CI runs, in order
```

`./mk ci` is exactly what the CI workflow runs — the workflow calls this
one command, so a check that is green locally and red in a pipeline cannot happen
by drift. It covers formatting (rustfmt and tombi), compilation, clippy with
warnings denied, tests, docs, SPDX headers, citation metadata, the wasm builds,
and the supply-chain checks. It needs no network access.

`./mk --list-all-steps` is the list of tasks. It is the authoritative one;
the summary above is not, because a copied list is a list that goes stale.

If it is slow, `./mk test` and `./mk lint` are the two that matter
most. The git hooks run the same tasks before a push — see `prek.toml`, which
delegates to them rather than repeating the cargo flags — so most failures are
caught before they reach CI.

### Tests run under nextest, and the doctests separately

The test suite runs under [`cargo-nextest`](https://nexte.st), which runs each
test in its own process and in parallel; it is roughly twice as fast as
`cargo test` on this workspace. nextest does **not** run doctests, so
`./mk test` runs the two in sequence, and so does every other place tests
run:

```bash
cargo nextest run --workspace          # the suite
cargo test --workspace --doc           # the doctests nextest skipped
```

The second command is not optional housekeeping. Each library crate's `README.md`
is its crate documentation via `#![doc = include_str!]`, and the example in it is
a doctest — so a dropped `cargo test --doc` leaves every README unchecked and
nothing notices.

Everything except the wasm builds runs with **no network access**, and that is a
property worth preserving. The search use case is tested through the `Http`
trait against a scripted conversation, so the QLever-to-WDQS fallback and the
retry policy are both covered without touching the network. If you find yourself
wanting to make a test hit a real endpoint, the thing to reach for is a fixture.

```bash
./mk metadata-write   # regenerate codemeta.json and CITATION.cff
```

### The one thing that does need the network

`./mk desktop` and the deploy fetch ~115 MB of third-party frontend assets —
Ketcher, RDKit, Citation.js — from four hosts. That is the only part of the gate
that reaches out, and it has been the source of intermittent failures: a single
reset connection failed a build that had nothing wrong with the commit.

It is now retried at two levels, and the reason is worth knowing before you
change either one. `fetch-assets` retries each request three times with a
doubling wait, so a blip inside a request is invisible. The `desktop` job and
the Dockerfile then retry the whole run and clear the partial tree first, for the
failure that retry cannot see: a run cut off part way through leaves a
half-populated `public/assets/`, and `dx build` succeeds against that — which
publishes a site with no structure editor rather than failing.

If you touch asset fetching, keep both levels and keep the `test -f` checks that
follow them.

## Where a change goes

  | You are changing | It belongs in |
  | --- | --- |
  | A domain type, a filter rule, validation | `lotus-model` |
  | A SPARQL string, a CSV column, an export format | `lotus-query` |
  | Which endpoint, when to retry, taxon resolution | `lotus-search` |
  | A curation row, a statement, the TSV reader | `lotus-curation` |
  | JSON-LD, CodeMeta, CITATION | `lotus-jsonld` |
  | A flag, an output format, an exit code | `lotus-cli` |
  | How many rows this machine renders, the clock, cache keys | the app |
  | Anything a person sees | the app |

The rule behind the table: **a crate that can be pure is pure.** `lotus-model`,
`lotus-query`, `lotus-curation` and `lotus-jsonld` have no HTTP, no clock and no
async runtime, and CI builds them for `wasm32` in their own right. If you find
yourself wanting a clock in one of them, the year is meant to be an argument.

## Two things that will bite you

**`SearchCriteria` has no `Default`.** "No filters" is not a value, it is a
value *plus a year*, so the constructor is `SearchCriteria::up_to_year(year)`.
Use the year your caller has.

**Name the crate, not a shim.** There are no `models.rs` or `queries.rs`
re-exports. `lotus_model::SearchCriteria` at a call site says where the type
lives; `crate::models::SearchCriteria` says only that the app once had a file
called `models.rs`.

**Half a serialisation contract is worse than none of it.** The URL parameter
names used to be built by a model method and parsed by a DTO in a different
crate, and a rename broke every link already in a browser's history with nothing
failing. Both halves now live in `CriteriaQueryDto`, and there is a round-trip
test to keep them together. If you add a parameter, add it there.

## Tests

- A test that would pass without the change is not a test. The most useful ones
  here assert a specific value: the exact SPARQL, the exact statements, the
  exact bytes.
- Prefer a fixture over a mock where a fixture will do.
- When behaviour changes on purpose, say in the commit message what the old
  behaviour was. `git log` is the only record of why a test asserts what it
  does.
- `docs/cli.md` is checked against `--help` in both directions and every
  documented example is run with `--explain`. If you change a flag, that test
  tells you.

## Headers

Every `.rs` file starts with exactly:

```rust
// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
```

`./mk license-headers` and CI both enforce it. Run it rather than fixing by
hand after the fact.

## Style

Match the file you are in. The conventions this repository holds to:

- Panics and `expect` are for tests and for genuinely unreachable states, never
  for input from a user, an endpoint, or a file. The lints are on.
- Comments explain **why**, and especially why the obvious thing is wrong. If a
  line needs a comment saying what it does, the line wants renaming instead.
- Prefer returning a value to mutating one, and an iterator to an index.
- A public item gets a doc comment; `#![warn(missing_docs)]` is on the library
  crates.
