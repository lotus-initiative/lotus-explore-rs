# Changelog

All notable changes to this project are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org).

## [Unreleased]

### Added

- **Archive CLI subcommand** (`lotus archive`): ports `lotus-wikidata-archive`.
  Fetches all LOTUS InChIKey-to-SMILES mappings from Wikidata via QLever,
  deduplicates by InChIKey (preferring isomeric P2017 over canonical P233),
  and packages the result as a versioned `tar.gz` with a manifest JSON and
  `SHA256SUMS` file. Zenodo publishing is dry-run by default; a real push
  requires `--publish` and the `ZENODO_TOKEN` environment variable.
- **Export CLI subcommand** (`lotus export`): ports `lotus_exporter.py`.
  Writes the full LOTUS compound–taxon–reference graph as `frozen.csv` and
  `frozen_metadata.csv`. The tables are joined in memory, so peak memory grows
  with the graph.
- `lotus export` consolidates its 16 per-property SPARQL queries into 4
  per-entity queries (`compound_taxon_reference`, `compounds`, `taxa`,
  `references`) scoped to occurring LOTUS entities, indexes CSV rows directly
  without per-row `HashMap` allocations, and both `lotus export` and
  `lotus export-pubchem-tree` show a zero-dependency `[====>---]` progress bar
  on interactive `stderr` terminals.
- **PubChem tree CLI subcommand** (`lotus export-pubchem-tree`): ports
  `lotus_pubchem_tree.py`. Builds three PubChem-format trees as JSON
  (biological, chemical, NPClassifier) in full and compact forms.
- CI workflow `.github/workflows/archive.yml` for scheduled/manual archiving,
  `workflow_dispatch` only, dry-run by default. The `publish` input gates
  the Zenodo push step; CI never sets it.
- FAQ entry (all four locales) for `cli-export`.
  The `cli-export-pubchem-tree` and `cli-archive` FAQ entries were removed
  from the website — those are CLI-only tools documented in `docs/cli.md`,
  not web-app features.

### Refactored

- Fixed two `cargo echoes` approximate context groups: extracted
  `insert_children_result` (shared child-conversion block in
  `biological_to_pubchem` and `chemical_to_pubchem`) and `decode_archive`
  (shared tar.gz decoding in archive tests) to eliminate duplicated code.

### Fixed

- Curation: one new compound named in several rows is created once, with
  every occurrence, instead of once per row (which made duplicate items).
  SMILES that differ only in case (`c1ccccc1`, `C1CCCCC1`) are no longer
  treated as duplicates of each other. `lotus curate --offline` no longer
  emits invalid taxon and reference statements that created empty items. One
  unreadable structure fails its own row rather than the whole file. When two
  items share an InChIKey, the kept item is judged by its own statements.
- Year-filtered searches report the endpoint's totals; the count query was
  invalid SPARQL and the counts fell back to the returned page.
  `lotus search --limit` now says exactly when rows were left out.
- Queries that run out of time, on QLever or on WDQS, are no longer retried,
  and long native downloads are no longer cut off after two minutes.
- `lotus archive`: `SHA256SUMS` names the archive file that is written, and a
  failed `--dry-run` exits non-zero.
- API server: requests are validated like CLI and browser searches; an
  export cut short upstream fails instead of downloading as a partial file;
  export downloads count against the concurrency limits; a disconnected
  client no longer pins a cached search result or error.
- The `static` Docker image's CSP allows the page's inline boot script.
- CSV exports no longer end in a blank line, and a byte-order mark before a
  header no longer hides the first column.
- `CITATION.cff` and `codemeta.json` no longer give the LOTUS article's DOI as
  the software's own.

- Exact compound searches now fetch canonical SMILES (P233) even when no
  isomeric SMILES (P2017) exists, restoring structure depictions and SMILES in
  downloaded CSV, JSON, and RDF results; all result, occurrence, count,
  dictionary, and compound-lookup queries now treat P233 and P2017 as optional
  display/resolution properties so compounds lacking P233 are not dropped or
  stripped of occurrences, and substructure/similarity searches on resolved
  compound names or InChIKeys substitute the resolved SMILES into the Sachem
  query.
- `clippy::future_not_send` on wasm32: added
  `#![cfg_attr(target_arch = "wasm32", allow(clippy::future_not_send))]`
  to `lotus-explore-core`, matching the same allow already present in
  `lotus-search` and `lotus-curation`. On the browser, `lotus_search::Http`
  implementations use JS promises (`Rc<RefCell>`) that are `!Send`.
- `clippy::redundant_pub_crate` in `cache.rs`: changed `pub(crate)` to `pub`
  in a private module re-export. This was masked by the
  `future_not_send` compilation failure on wasm32.

### Architecture

- `docs/SHARING_PLAN.md`: documents the code-sharing plan across the three
  ports — which logic lives in which existing crate, and what is intentionally
  not shared (smiles-rs validation, Zenodo publishing, NPClassifier tree,
  PubChem API access).
