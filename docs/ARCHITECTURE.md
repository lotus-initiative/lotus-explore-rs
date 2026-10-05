# Architecture

The repository is a workspace of six library crates, one binary, and one
application. The arrangement is not a preference: it is what keeps the same
query from being written three times, and it is checkable.

## The crates

  | Crate                   | Contains                                                                              | Depends on                   |
  | ---                     | ---                                                                                   | ---                          |
  | `lotus-model`           | Domain types, filter semantics, structure classification, validation                  | `serde`, `thiserror`         |
  | `lotus-query`           | SPARQL construction, CSV parsing, export formats                                      | `lotus-model`                |
  | `lotus-search`          | The search use case: taxon resolution, retries, QLever→WDQS fallback, result assembly | `lotus-model`, `lotus-query` |
  | `lotus-curation`        | The curation vocabulary, the TSV reader, QuickStatements bundling                     | `serde`, `thiserror`         |
  | `lotus-jsonld`          | Bioschemas JSON-LD, CodeMeta, `CITATION.cff`                                          | `lotus-model`                |
  | `lotus-cli`             | The `lotus` binary                                                                    | all of the above             |
  | `lotus-web-assets`      | Vendors Ketcher and RDKit into `public/assets`, preloads the built wasm               | —                            |
  | `apps/lotus-explore-rs` | The web client and its optional native server                                         | the five above               |

The arrows all point inward. `lotus-model` and `lotus-curation` depend on no
other crate in the workspace, which is why they can hold the vocabulary that
everything else agrees on.

## Two rules, and why they are checkable

**A crate that can be pure is pure.** `lotus-model`, `lotus-query`,
`lotus-curation` and `lotus-jsonld` have no HTTP, no clock and no async runtime.
CI builds them for `wasm32` in their own right rather than relying on the app to
pull them in, which is what makes the claim testable. A crate that cannot read a
clock cannot read the current year by accident, so `SearchCriteria` takes the
year as an argument and its tests pin one.

**A decision is named.** The difference between `lotus-query` and `lotus-search`
is the difference between a transformation and a decision. Building a query
string and reading the CSV that comes back are pure, and are tested by string
and fixture comparison. Choosing an endpoint, retrying, falling back, and
deciding whether a taxon string is a name or a QID are decisions, and are tested
through the `Http` trait against a scripted conversation. That split is also
what lets a caller who wants a query and nothing else avoid inheriting an HTTP
client.

## Where things used to be wrong

The repository once had a `lotus` crate holding the models, the queries, the
parsers and the HTTP client, and the web app had a second copy of several of
those, including two implementations of "what a taxon filter means" that
disagreed. The refactor that produced the layout above deleted `crates/lotus`
and moved the app onto these crates, which is the only reason the arrows point
inward now. [`FRONTENDS.md`](FRONTENDS.md) has the later instance of the same
failure and why a duplicated dispatch is invisible to tests.

Two things came out of that refactor and look odd without knowing it:
`SearchCriteria` has no `Default`, because "no filters" is a value *plus a year*
and the old `default()` supplied the year by reading a clock; and
`has_year_filter`, `has_effective_filters` and `with_filters` take the year as an
argument, because the application has the clock.

## Testing

- The pure crates are tested with no network and no runtime: string comparisons
  for query builders, recorded fixtures for the parsers.
- `lotus-search` is tested through the `Http` trait, including the fallback path
  and the retry policy, which is otherwise very hard to provoke.
- `lotus-cli` runs its integration tests offline by asserting on `--explain`,
  which prints the SPARQL without sending it.
- `docs/cli.md` is checked against `--help` in both directions, and every
  documented example is run with `--explain` appended.

The full task list is `./mk --list-all-steps`; it is not copied here so that it
cannot go stale. [`CONTRIBUTING.md`](../CONTRIBUTING.md) covers the test
conventions and what to run.

## See also

- [`ARCHITECTURE.md`](../apps/lotus-explore-rs/docs/ARCHITECTURE.md) --- the
  application side, and why the clock lives there
- [`cli.md`](cli.md) --- the command line
- [`TAXON-SEARCH.md`](TAXON-SEARCH.md) --- how a taxon name becomes a QID, and the
  four nomenclatural relationships
- [`STRUCTURE-SEARCH.md`](STRUCTURE-SEARCH.md) --- how a name, an InChIKey or a QID
  becomes a compound identity, and when the structure service is called at all
- [`QUERY_MATRIX.md`](QUERY_MATRIX.md) --- the twelve argument combinations and the
  query each one builds
- [`SPARQL-VARIANTS.md`](SPARQL-VARIANTS.md) --- query forms measured against the
  live endpoint, and which were kept
