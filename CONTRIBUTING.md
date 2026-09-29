# Contributing

## Before you start

Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) first. The workspace is
arranged so that a change usually has an obvious home, and that arrangement is
load-bearing: the web app and the CLI share their query building and their
curation vocabulary precisely so that a query the explorer runs and a query
`lotus search` runs are the same query.

## Running the checks

```bash
just ci
```

That is every check CI runs, in order: formatting, compilation, clippy with
warnings denied, tests, docs, SPDX headers, citation metadata, the wasm builds,
and unused dependencies. If it is slow, `just test` and `just clippy` are the
two that matter most.

Everything except the wasm builds runs with **no network access**, and that is a
property worth preserving. The search use case is tested through the `Http`
trait against a scripted conversation, so the QLever-to-WDQS fallback and the
retry policy are both covered without touching the network. If you find yourself
wanting to make a test hit a real endpoint, the thing to reach for is a fixture.

```bash
just metadata-write   # regenerate codemeta.json and CITATION.cff
```

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

`just license-headers` and CI both enforce it. Run it rather than fixing by hand
after the fact.

## Style

Match the file you are in. The conventions this repository holds to:

- Panics and `expect` are for tests and for genuinely unreachable states, never
  for input from a user, an endpoint, or a file. The lints are on.
- Comments explain **why**, and especially why the obvious thing is wrong. If a
  line needs a comment saying what it does, the line wants renaming instead.
- Prefer returning a value to mutating one, and an iterator to an index.
- A public item gets a doc comment; `#![warn(missing_docs)]` is on the library
  crates.
