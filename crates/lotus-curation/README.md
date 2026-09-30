# lotus-curation

The curation vocabulary: what a curation row is, and how the statements that
would complete it in Wikidata are assembled.

Curation means a chemist has a list of compounds and taxa, some of which Wikidata
already has and some of which it does not. This crate says which is which and
writes the statements that would fix the ones that are not. It never writes to
Wikidata; submitting them is a separate, deliberate act.

Pure. Nothing here talks to Wikidata or to `RDKit`.

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
