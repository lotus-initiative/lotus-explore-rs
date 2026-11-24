# lotus-export

Full-graph bulk ETL pipelines for LOTUS over an injected `lotus_search::Http` transport:

- `pipeline`: fetches the 4 core Wikidata tables, joins them by QID, and emits `frozen.csv` + `frozen_metadata.csv`.
- `archive`: paginates LOTUS SMILES from `QLever`, deduplicates by `InChIKey` (preferring isomeric `P2017`), and packages a deterministic `.tar.gz` with `manifest.json` and `SHA256SUMS`.
- `pubchem_tree`: fetches 12 Wikidata tables plus the `NPClassifier` cache to build biological, chemical, and `NPClassifier` trees.

```rust
use lotus_export::archive::{WriteArchiveInput, write_archive};
use lotus_export::archive::DEFAULT_QLEVER_ENDPOINT;

let artifacts = write_archive(&WriteArchiveInput {
    fetched_at: "2026-03-04T05:06:07Z",
    archive_filename: "lotus-2026-03-04.tar.gz",
    endpoint: DEFAULT_QLEVER_ENDPOINT,
    source_records: 0,
    missing_inchi_key_records: &[],
    records: &[],
    parse_errors: &[],
})
.unwrap();
assert!(artifacts.archive.starts_with(&[0x1f, 0x8b]));
```

## License

AGPL-3.0-only.
