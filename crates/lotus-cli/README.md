# lotus

Search the LOTUS knowledge graph from a terminal: chemical compounds, the
organisms they occur in, and the references that report them. The data is the
Wikidata projection of the [LOTUS database](https://lotus-db.com), queried over
SPARQL.

Every filter the web explorer offers is available here and the output formats
are the same, so a result set moves between the two without changing anything
but the command line.

## Install

```bash
cargo install --path crates/lotus-cli
```

Or build without installing:

```bash
cargo build --release -p lotus-cli
./target/release/lotus --help
```

## Use

```bash
# Find a compound and print it as CSV.
lotus search "quercetin" --format csv

# Everything reported for a taxon, restricted to one reference.
lotus search --taxon "Gentiana lutea" --doi 10.1000/xyz --format json

# Check what Wikidata already has for a file of compounds, and print the edits
# that would complete it. Read-only: nothing is ever submitted.
lotus curate compounds.json
```

## Documentation

[`docs/cli.md`](../../docs/cli.md) is the reference: every flag, the output
formats, the exit codes, and the endpoint overrides. It is checked against
`--help` by `crates/lotus-cli/tests/docs_in_sync.rs`, so a flag that changes
without the document changing fails the build.

`lotus man` writes the same reference as a manual page, and
`lotus completions <shell>` writes a completion script.

## What it will not do

`lotus curate` never writes to Wikidata. It looks each entry up, labels what is
missing, and prints the statements that would complete each one. Submitting them
is a separate, deliberate act by a person.

## Licence

AGPL-3.0-only.
