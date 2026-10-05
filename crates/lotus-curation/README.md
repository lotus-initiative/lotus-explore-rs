# lotus-curation

The curation vocabulary: what a curation row is, and how the statements that
would complete it in Wikidata are assembled.

A chemist has a list of compounds and taxa, some of which Wikidata already has.
This crate says which is which and writes the statements that would fix the rest.
It never writes to Wikidata; submitting them is a separate, deliberate act.

Pure: nothing here talks to Wikidata or `RDKit`.

```rust
use lotus_curation::{CurationStatus, parse_tsv};

let rows = parse_tsv("name\tsmiles\ttaxon\tdoi\naspirin\tCC(=O)Oc1ccccc1C(=O)O\n")
    .expect("`name` and `smiles` are the only required columns");

assert_eq!(rows[0].name, "aspirin");

// "Not checked" is not "new": a row that was never looked up has not
// established that the compound is absent.
assert_ne!(CurationStatus::NotChecked, CurationStatus::NewCompound);
```

See the [root README](../../README.md) for the crates and how they fit together.
