# lotus-model

The LOTUS vocabulary: what a filter set is, what a result row is, and what makes
a filter active.

Pure. No IO, no async, no clock, and no platform, so it builds for
`wasm32-unknown-unknown` and is testable without a runtime. A caller that needs
the current year passes it in.

A filter is a value, not a query. It holds no SPARQL and no endpoint, and whether
a filter is *active* is decided by comparing against the defaults rather than by a
flag, so a criteria set survives a round trip through a URL or a JSON body without
carrying its provenance.

```rust
use lotus_model::{SearchCriteria, validate_criteria};

let criteria = SearchCriteria {
    taxon: "Gentiana lutea".to_owned(),
    mass_max: 500.0,
    ..SearchCriteria::up_to_year(2026)
};

assert!(criteria.has_mass_filter());
assert!(validate_criteria(&criteria, 2026).is_ok());
```

See the [root README](../../README.md) for the crates and how they fit together.
