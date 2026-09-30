# lotus

Search the LOTUS knowledge graph from a terminal: chemical compounds, the
organisms they occur in, and the references that report them. The data is the
Wikidata projection of [LOTUS](https://doi.org/10.7554/eLife.70780), queried over
SPARQL.

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

`search` takes filters, not a query term. To find a compound, search by its
structure; to find everything reported for an organism, search by taxon.

```bash
# Everything Wikidata reports for one organism, as CSV.
lotus search --taxon "Gentiana lutea" --format csv

# Compounds similar to benzene, and see the SPARQL instead of running it.
lotus search --structure c1ccccc1 --structure-search similarity --threshold 0.9
lotus search --structure c1ccccc1 --explain

# Narrow by formula, publication year, and element counts.
lotus search --taxon "Isaria cicadae" --year-min 2015 --year-max 2024
lotus search --taxon "Voacanga africana" --carbon 10..20 --bromine excluded
```

The binary is `lotus`. If you did not install it, prefix these with
`./target/release/`.

## Documentation

[`docs/cli.md`](../../docs/cli.md) is the reference: every flag, the output
formats, the exit codes, and the endpoint overrides. It is checked against
`--help` by `crates/lotus-cli/tests/docs_in_sync.rs`, and so is every example in
this file --- a `lotus` line that no longer parses fails the build.

`lotus man` writes the same reference as a manual page, and
`lotus completions <shell>` writes a completion script.

## What it will not do

`lotus curate` never writes to Wikidata. It looks each entry up, labels what is
missing, and prints the statements that would complete each one. Submitting them
is a separate, deliberate act by a person.

## Licence

AGPL-3.0-only.
