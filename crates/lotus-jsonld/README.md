# lotus-jsonld

JSON-LD for LOTUS results, following the Bioschemas profiles, plus the citation
metadata (`codemeta.json`, `CITATION.cff`) generated from the same description.

The point is findability: Google Dataset Search and the Bioschemas validator both
read the document, and neither can read a page that carries no markup. Every
document states the profile it follows, so a validator knows which required
properties apply rather than inferring them from the shape.

```rust
use lotus_jsonld::{Software, citation_cff, codemeta, software_jsonld};

let software = Software {
    name: "LOTUS Explorer",
    description: "Explore the LOTUS knowledge graph over SPARQL.",
    url: "https://lotus.nprod.net/lotus-explore-rs/",
    repository: "https://github.com/lotusnprod/lotus-explore-rs",
    doi: None,
    version: "0.1.0",
    license: "AGPL-3.0-only",
    keywords: &["LOTUS", "SPARQL"],
};

assert_eq!(software_jsonld(&software)["@type"], "SoftwareApplication");
assert_eq!(codemeta(&software)["@type"], "SoftwareSourceCode");
assert!(citation_cff(&software).contains("cff-version: 1.2.0"));
```

See the [root README](../../README.md) for the crates and how they fit together.
