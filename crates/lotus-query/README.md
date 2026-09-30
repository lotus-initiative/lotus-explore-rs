# lotus-query

SPARQL construction and result parsing for LOTUS.

Two halves, and neither one needs HTTP, an async runtime or a clock: `query`
builds query strings from a `lotus-model::SearchCriteria`, and `parse` turns a
`text/csv` payload back into model types.

Running the query is somebody else's job -- `lotus-search` drives this crate
against a real endpoint. That split is what lets the builders be tested by string
comparison and the parsers against recorded fixtures.

```rust
use lotus_query::{compounds_by_taxon_query, parse_taxon_csv};

let sparql = compounds_by_taxon_query("Q131841");
assert!(sparql.contains("wd:Q131841"));

let csv = b"taxon,taxon_name\nQ131841,Gentiana lutea\n";
let taxa = parse_taxon_csv(csv).expect("the header carries both columns");
assert_eq!(taxa[0].name, "Gentiana lutea");
```

See the [root README](../../README.md) for the crates and how they fit together.
