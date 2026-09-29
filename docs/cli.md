# lotus --- the command line

Search the LOTUS knowledge graph: chemical compounds, the organisms they occur
in, and the references that report them. The data is the Wikidata projection of
the LOTUS database, queried over SPARQL.

Every filter the [web explorer](https://lotus.nprod.net/lotus-explore-rs) offers
is available here, and the output formats are the same, so a result set can be
moved between the two without changing anything but the command line.

This file is checked against `--help` by `tests/docs_in_sync.rs`. If a flag
changes and this file does not, the test fails.

## Install

```bash
cargo install --path crates/lotus-cli
```

`--path` is a `cargo install` flag, not one of `lotus`'s own.

Or build without installing:

```bash
cargo build --release -p lotus-cli
./target/release/lotus --help
```

## search

```bash
lotus search --taxon "Gentiana lutea"
lotus search --structure c1ccccc1 --structure-search similarity --threshold 0.9
lotus search --taxon "Voacanga africana" --carbon 10..20 --bromine excluded
lotus search --taxon "Isaria cicadae" --year-min 2015 --year-max 2024
```

### Filters

  | Flag                                                                           | Meaning                                                          |
  | ---                                                                            | ---                                                              |
  | `--taxon`                                                                      | Scientific name, Wikidata QID, or `*` for every organism         |
  | `--structure`                                                                  | SMILES, or an MDL molfile (V2000/V3000)                          |
  | `--structure-search`                                                           | `substructure` or `similarity`                                   |
  | `--threshold`                                                                  | Tanimoto cutoff for a similarity search, 0 to 1                  |
  | `--mass-min`, `--mass-max`                                                     | Molecular mass range, in daltons                                 |
  | `--year-min`, `--year-max`                                                     | Publication year range                                           |
  | `--formula`                                                                    | Exact molecular formula; subscripts are accepted                 |
  | `--limit`                                                                      | How many rows to return. The counts still describe the whole set |
  | `--format`                                                                     | How to write the rows; see below                                 |
  | `--log`                                                                        | Verbosity on stderr: `error`, `warn`, `info`, `debug`            |
  | `--explain`                                                                    | Print the SPARQL and send nothing                                |
  | `--carbon`, `--hydrogen`, `--nitrogen`, `--oxygen`, `--phosphorus`, `--sulfur` | Atom-count range, e.g. `10..20`                                  |
  | `--fluorine`, `--chlorine`, `--bromine`, `--iodine`                            | Halogen presence: `allowed`, `required`, `excluded`              |

An element range is written `MIN..MAX`, and either end may be omitted:
`--carbon ..20` is at most 20 carbons, `--carbon 5..` is at least 5. A bare
number is an upper bound, because that is the bound a chemist narrows with.

Any of the formula filters turns the formula section on by itself: typing
`--carbon 10..20` is filtering by formula, and you do not also need `--formula`.

### Output

`--format` takes `table`, `tsv`, `csv`, `json`, `jsonl`, `jsonld` or `query`.

Data goes to stdout and diagnostics to stderr, so redirection captures the data
and nothing else:

```bash
lotus search --taxon "Gentiana lutea" --format csv > gentiana.csv
lotus search --taxon "Gentiana lutea" --format jsonl | jq -r .name
```

- `table` is aligned for a terminal and is not meant for parsing. It omits the
  structure column, which is too wide to read and too useful to lose --- use
  `--format json` when you want it.
- `tsv` and `csv` are one header row followed by the rows, with values quoted
  when they contain the separator.
- `json` is one object with the query, the rows, the counts and a `truncated`
  flag. `jsonl` is one row per line, for streaming.
- `jsonld` is Bioschemas JSON-LD: a `Dataset` for the result set, the LOTUS
  source, and a `MolecularEntity` per compound. See
  [`lotus-jsonld`](../crates/lotus-jsonld).
- `query` prints the SPARQL and no rows.

### Looking at the query without running it

`--explain` builds the query and prints it without sending anything, so it works
offline:

```bash
lotus search --taxon "Gentiana lutea" --carbon 10..20 --explain
lotus search --structure c1ccccc1 --structure-search similarity --explain
```

Because no request is made, no taxon is resolved either, so the printed query
has no `P171` ancestry filter. That is the point: you see the shape, and you can
paste it into any SPARQL endpoint.

The query `--explain` prints is not capped by `--limit`, so pasting it returns
the whole result set rather than the first page.

## curate

Check a set of findings against what Wikidata already has, and emit the
statements a curator would submit.

```bash
lotus curate findings.tsv
lotus curate --format jsonl < findings.tsv
lotus curate findings.tsv --output statements.txt
```

The input is a TSV with `name` and `smiles` columns, and optionally `taxon` and
`doi`. Columns are matched by name, so extra columns from a spreadsheet export
are ignored rather than shifting everything:

```tsv
name    smiles  taxon   doi
Quercetin   O=c1c(O)c(-c2ccc(O)c(O)c2)oc2cc(O)cc(O)c12  Gentiana lutea  10.7554/eLife.70780
```

Pass `-` or nothing to read stdin. `--format` takes `table`, `tsv`, `json` or
`jsonl`.

### What a run looks up

Wikidata identifies a compound by its InChIKey, and a curator's file has SMILES,
so a run converts each structure first and then asks Wikidata about the result.
That is two or more requests per row, against two public services, so the command
tells you when it cannot reach either.

A row is reported as one of:

| Status              | Meaning                                                   |
| ------------------- | --------------------------------------------------------- |
| `existing_complete` | Wikidata has the compound and the occurrence              |
| `existing_updates`  | The compound is there; the occurrence is missing          |
| `new_compound`      | No such item, and statements to create one                |
| `pending_dependencies` | A taxon or reference the row needs is not there yet     |
| `not_checked`       | Not looked up, so nothing is known                        |
| `error`             | The lookup failed; this is not the same as "absent"       |

`not_checked` and `error` are the two that matter. A row reported as
`new_compound` has been shown to be absent; a row reported as `not_checked` has
not, and a network failure that read as "absent" would put a duplicate of a
compound that has been in Wikidata for years into every run.

Because matching is on the InChIKey rather than the SMILES, a compound written
`CCO` in one row and `OCC` in another is one compound, not two.

A genus on its own is not resolved: "Gentiana" matches hundreds of species, and
curating an occurrence against the genus rather than a species is a data error.
The row is reported instead of guessed at.

### Offline

`--offline` makes no requests at all and reports every row as `not_checked`:

```bash
lotus curate findings.tsv --offline
```

The statements are still emitted, because there is something to read either way,
but the status says they are a draft. Under `--offline` that is every row.

**Nothing is submitted to Wikidata, and there is no flag that would.** The
output is QuickStatements for a person to read and submit, and the command says
so on stderr every time; `--quiet` silences that reminder and the count of
unchecked rows, for when stderr is noise. There is deliberately no `--apply`: a
batch of statements arriving from a script at 3am is not a decision anybody made
on purpose.

Each row becomes one compound statement plus, when the row names them, a
separate occurrence and a separate reference. They are separate because a
`QuickStatements` run stops at the first failure --- an occurrence pointing at a
taxon nobody has created would take down the compound statement in the same
block, which is the part a curator most wants.

The formats differ in what they carry. `table` lists the rows and then appends
the statements below them. `json` carries the rows, the statements and the
citation. `tsv` and `jsonl` are one row per line and carry neither the
statements nor the citation, so they are for piping into something else, not for
curation.

Rows that name the same structure, taxon and DOI under different names are one
finding, because a spreadsheet duplicate is an artefact rather than a second
thing to curate. The comparison folds case on all three, since `CCO` and `cco`
are the same molecule.

## completions

```bash
lotus completions bash > /etc/bash_completion.d/lotus
lotus completions zsh  > "${fpath[1]}/_lotus"
lotus completions fish > ~/.config/fish/completions/lotus.fish
```

`elvish` is also available.

## man

```bash
lotus man | man -l -
```

## Exit codes

  | Code | Meaning                                       |
  | ---- | --------------------------------------------- |
  | 0    | Success                                       |
  | 1    | The search or curation could not be completed |
  | 2    | The arguments were not valid                  |

## Endpoints

Queries go to QLever's Wikidata endpoint, and fall back to the Wikidata Query
Service when QLever is unreachable. A query that is *rejected* is not retried
elsewhere: WDQS would reject it too, and trying would only double the load.

To point the CLI at a different endpoint, set `LOTUS_QLEVER_ENDPOINT`; the
fallback pair is `LOTUS_WDQS_ENDPOINT` and `LOTUS_WDQS_SCHOLARLY_ENDPOINT`.
