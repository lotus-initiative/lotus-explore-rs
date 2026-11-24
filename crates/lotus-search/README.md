# lotus-search

The search use case: give it a criteria set and a taxon string, get back rows, a
count, and notes about anything that had to be adjusted on the way.

This layer owns the decisions -- which endpoint to answer, when to fall back, and
whether a taxon string is an item or a name to look up. All IO goes through a
two-method `Http` trait, so the whole use case including the fallback path is
testable offline against a script of canned answers.

```rust
use lotus_model::SearchCriteria;
use lotus_search::{SearchRequest, build_execution_query};

let req = SearchRequest::new(
    SearchCriteria {
        taxon: "Q158572".into(),
        ..SearchCriteria::up_to_year(2026)
    },
    2026,
);
let sparql = build_execution_query(&req, Some("Q158572"));
assert!(sparql.contains("wd:Q158572"));
```

See the [root README](../../README.md) for the crates and how they fit together.
