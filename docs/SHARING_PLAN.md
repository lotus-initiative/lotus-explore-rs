# Sharing plan: lotus_exporter.py, lotus_pubchem_tree.py, lotus-wikidata-archive

This note records what the three Python sources share, where each shared part
lives in the Rust workspace, and what is new. It is written before any code so
the port does not create parallel implementations.

## What the three sources share

| Shared concern | lotus_exporter.py | lotus_pubchem_tree.py | lotus-wikidata-archive | Where it lives in Rust |
|---|---|---|---|---|
| Wikidata/SPARQL access | QLever endpoint, `execute_with_retry` | QLever endpoint, `execute_with_retry` | QLever endpoint, paged `LIMIT`/`OFFSET` + retry | `lotus-search` (existing `Http` + retry; `execute`/`execute_streaming`) |
| SPARQL queries | 18 fixed queries (triplets, taxon, reference, structure metadata) | 12 fixed queries (compound-taxon, taxon tree, compound tree, references) | 2 fixed queries (SMILES with/without InChIKey) | `lotus-query` (new `bulk_export` module, pure query builders) |
| LOTUS data model | compound ↔ taxon ↔ reference triplets | same triplets + compound P279 tree + taxon P171 tree | compound SMILES with P703 + P235 | `lotus-model` (existing types; no new types needed) |
| Extract QIDs from Wikidata URLs | `extract_qid` (strip `http://www.wikidata.org/entity/`) | same | `wikidata_id` (rsplit on `/`) | `lotus-query` bulk_export (one helper, reused by all three) |
| Tabular processing | Polars joins/coalesce/filter | Polars group_by/agg/dict build | CSV rows + dedup by InChIKey | `lotus-explore-core` (new `export::pipeline`, injectable HTTP) |
| External caches | NPClassifier, ClassyFire, OTT (fetch + join) | NPClassifier (fetch + tree) | none | `lotus-explore-core` pipeline; cache URLs from config |
| Rate limiting / batching | PubChem batched VALUES (5000 CIDs/InChIKeys per query) | none | QLever paged 50k rows + retry-after | `lotus-search` retry; pipeline batches PubChem calls |
| Output serialization | CSV.GZ (frozen.csv, frozen_metadata.csv, changes report) | JSON (biological tree, chemical tree, full + PubChem compact) | tar.gz + manifest.json + SHA256SUMS | `lotus-query` `export_rows` (streaming CSV) + new archive writers |
| CLI entry | `export -o DIR -v` | `export-pubchem-tree -o DIR` | `--output-dir --publish --dry-run ...` | `lotus-cli` (new `archive` subcommand; `export` and `export-pubchem-tree` already wired) |
| UI entry | marimo app (fetch → build → download) | marimo app (fetch → build trees → download) | none (CLI only) | `apps/lotus-explore-rs` (new pages, thin Dioxus adapter) |
| Dry-run / no-network | none | none | `--dry-run` (validate without Zenodo HTTP) | pipeline takes injectable transport; tests use `lotus-search::testing` |

## What is NOT shared (intentionally separate)

- **`smiles-rs` validation** (archive only): the Python `lotus-wikidata-archive` uses
  `smiles-rs` to validate parsed SMILES and record `ParseErrorRecord`s. The Rust port omits
  this: `smiles-rs` has no `wasm32` story and the workspace's linters deny `expect`/`unwrap`
  on untrusted input, which `smiles-rs`'s parser does not guarantee. SMILES validation is
  documented as a known gap; `parse_errors` is always zero in the Rust port, and the CSV
  column is still emitted so the format stays compatible. If validation is needed later it
  lands behind a `native-only` Cargo feature, not in the shared crate.
- **Zenodo publishing** (archive only): dry-run by default; real push requires explicit opt-in
  flag + env var/secret. Not part of the exporter or pubchem tree.
- **NPClassifier tree** (pubchem tree only): a second chemical classification tree; the
  exporter only joins NPClassifier as flat metadata columns.
- **PubChem API calls** (pubchem tree only): the Python `lotus_pubchem_tree.py` fetches
  compound properties from the PubChem REST API in batches of 5000. The Rust port reuses the
  Wikidata SPARQL queries for all compound/ taxon /reference data and does not call the
  PubChem API, which keeps the tree builder fully offline-testable. PubChem API integration
  is documented as a future optional feature behind an injectable transport.

## Crate boundary decisions

No new crate. The three sources map onto existing crates:

- **`lotus-query`** — pure SPARQL query builders for all bulk-export queries (bulk_export module). Already has `export::ExportFormat` and `export_rows::RowExporter` for streaming CSV.
- **`lotus-explore-core`** — the export pipeline: fetch (injectable `Http`), process (joins/coalesce), write (files). No Dioxus, testable natively, wasm-clean when the transport is injected.
- **`lotus-search`** — the `Http` trait + retry + `testing` mock transport, already used by the search use case. Reused unchanged.
- **`lotus-cli`** — `archive` subcommand (the `export` and `export-pubchem-tree` subcommands are already wired).
- **`apps/lotus-explore-rs`** — thin UI pages over the core pipeline.

## Why no new crate

A new `lotus-export` crate would duplicate the dependency graph (`lotus-model`, `lotus-query`, `lotus-search`) and the `Http` abstraction that `lotus-search` already owns. The export pipeline is a *use case* (fetch → process → write), which is exactly what `lotus-explore-core` is for ("domain logic ... no UI framework, no IO" — but it can depend on `lotus-search` which owns IO). Adding the pipeline there keeps one crate per concern instead of a fifth crate that re-exports the same four.

## File layout (new files only)

```
crates/lotus-query/src/bulk_export.rs         # pure SPARQL query builders
crates/lotus-query/src/bulk_export/tests.rs    # exact SPARQL assertions
crates/lotus-explore-core/src/export/pipeline.rs   # fetch + process + write pipeline
crates/lotus-explore-core/src/export/pipeline/tests.rs # golden fixture tests
crates/lotus-explore-core/src/export/archive.rs    # tar.gz + manifest + checksums
crates/lotus-explore-core/src/export/archive/tests.rs
crates/lotus-explore-core/src/export/pubchem_tree.rs  # biological/chemical/NPClassifier trees
crates/lotus-explore-core/src/export/pubchem_tree/tests.rs
```

Existing files touched (additive only):
- `crates/lotus-query/src/lib.rs` — re-export `bulk_export`
- `crates/lotus-explore-core/src/export/mod.rs` — declare `pipeline`, `archive`, `pubchem_tree` modules
- `crates/lotus-cli/src/main.rs` — add `archive` subcommand + entry point
- `docs/cli.md` — document new subcommands (checked by `docs_in_sync`)
- `docs/ARCHITECTURE.md` — note where export lives and why
