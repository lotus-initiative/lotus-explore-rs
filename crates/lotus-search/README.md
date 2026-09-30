# lotus-search

The search use case: give it a criteria set and a taxon string, get back rows, a
count, and notes about anything that had to be adjusted on the way.

This layer owns the decisions -- which endpoint to answer, when to fall back, and
whether a taxon string is an item or a name to look up. All IO goes through a
two-method `Http` trait, so the whole use case including the fallback path is
testable offline against a script of canned answers.

```rust
use lotus_search::{is_qid, normalize_structure, standardize_taxon_name};

assert!(is_qid("Q131841"));
assert!(!is_qid("Gentiana lutea"), "that is a name to look up");
assert_eq!(normalize_structure("  CCO  "), "CCO");
assert_eq!(standardize_taxon_name("  gentiana lutea "), "Gentiana lutea");
```

See the [root README](../../README.md) for the crates and how they fit together.
