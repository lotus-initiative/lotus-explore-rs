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
  `frozen_metadata.csv`, streaming rows to bounded memory.
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

### Architecture

- `docs/SHARING_PLAN.md`: documents the code-sharing plan across the three
  ports — which logic lives in which existing crate, and what is intentionally
  not shared (smiles-rs validation, Zenodo publishing, NPClassifier tree,
  PubChem API access).
