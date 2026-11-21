# Curation share-link schema

`lotus-explore-rs` reproduces curation sessions from the `/curation` route.

## Parameters

- `lang=<en|fr|de|it>`
  - selects the UI language
- `curation_rows=<url-encoded JSON array>`
  - serialized `CurationInputRow[]`
  - each row contains:
    - `name`
    - `smiles`
    - `taxon` (optional)
    - `doi` (optional)
    - `status` (optional; `revised` suppresses automatic `CREATE` emission)
- `curation_run=true`
  - auto-runs curation after the page loads

## Example

```text
/curation?lang=en&curation_rows=%5B%7B%22name%22%3A%22Caffeine%22%2C%22smiles%22%3A%22Cn1cnc2n%28C%29c%28%3DO%29n%28C%29c%28%3DO%29c12%22%2C%22taxon%22%3Anull%2C%22doi%22%3Anull%7D%5D&curation_run=true
```

## Notes

- Share links encode the queued input rows, not previously generated
  QuickStatements.
- QuickStatements are regenerated on load so results stay reproducible after
  dependency creation or Wikidata changes.
- If prerequisite entities are still missing, run the prerequisite block first
  in QS-Dev, create/merge items in Wikidata, then reopen or rerun the second
  pass.
