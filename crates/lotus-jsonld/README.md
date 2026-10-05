# lotus-jsonld

JSON-LD for LOTUS results, following the Bioschemas profiles, plus the citation
metadata (`codemeta.json`, `CITATION.cff`) generated from the same description.

Google Dataset Search and the Bioschemas validator both read these documents, and
neither can read a page carrying no markup. Every document states the profile it
follows, so a validator knows which required properties apply.

```rust
use lotus_jsonld::{SOFTWARE, citation_cff, codemeta, software_jsonld};

// The real project, so this example cannot drift from what ships.
let software = SOFTWARE;

assert_eq!(software_jsonld(&software)["@type"], "SoftwareApplication");
assert_eq!(codemeta(&software)["@type"], "SoftwareSourceCode");
assert!(citation_cff(&software).contains("cff-version: 1.2.0"));
```

See the [root README](../../README.md) for the crates and how they fit together.
