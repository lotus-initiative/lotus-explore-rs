// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `lotus` — search the LOTUS knowledge graph from a terminal.
//!
//! A thin layer over `lotus-model` (filters), `lotus-search` (the search
//! sequence) and `lotus-jsonld` (JSON-LD). Every semantic decision lives in
//! those crates, so the CLI and the web app cannot drift apart.

// `cargo test` builds this binary twice: once as the shipped artifact, and once
// with `--test` so the integration tests can find it. In the second build the
// dev-dependencies are linked in and the binary itself uses none of them, so
// `unused_crate_dependencies` cannot be satisfied under `--all-targets`. The
// real dependencies are checked in `src/` by not being listed unless used.
#![allow(
    unused_crate_dependencies,
    reason = "the `--test` build links the dev-dependencies without the binary using them"
)]
// This is a binary: there is nothing outside the crate for `pub` to reach, so
// `unreachable_pub` reports every module and item that cross a module boundary.
// The items are `pub` only so that `main` can name them.
#![allow(
    unreachable_pub,
    reason = "a binary has no external consumer, so every `pub` item crosses a module boundary for nothing"
)]

use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use lotus_explore_core::export::pubchem_tree::run_pubchem_tree;
use lotus_model::SmilesSearchType;

mod curate;
mod output;

use crate::output::{Format, write_rows};

/// Search LOTUS: chemical compounds reported in organisms, from Wikidata.
#[derive(Debug, Parser)]
#[command(
    name = "lotus",
    version,
    about = "Search the LOTUS natural-products knowledge graph over SPARQL",
    long_about = "Search the LOTUS knowledge graph of chemical compounds, the organisms \
                  they occur in, and the references that report them. The data is the \
                  Wikidata projection of the LOTUS database, queried over SPARQL.\n\n\
                  Every filter the web explorer offers is available here, and the \
                  output formats are the same, so a result set can be moved between \
                  the two without changing anything but the command line."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Find compounds, taxa and the references that report them.
    Search(SearchArgs),
    /// Check what Wikidata already has for a set of compounds, and emit the
    /// edits needed to complete it.
    Curate(curate::CurateArgs),
    /// Print a shell completion script to stdout.
    Completions(CompletionsArgs),
    /// Write the manual page to stdout.
    Man,
    /// Export the full LOTUS compound-taxon-reference graph as CSV files.
    Export(ExportArgs),
    /// Build `PubChem`-format trees (biological, chemical, `NPClassifier`) as JSON.
    ExportPubchemTree(ExportPubchemTreeArgs),
    /// Archive LOTUS SMILES from Wikidata as a versioned tar.gz with manifest and checksums.
    Archive(ArchiveArgs),
}

#[derive(Debug, clap::Args)]
struct CompletionsArgs {
    /// The shell to generate for.
    #[arg(value_enum)]
    shell: clap_complete::Shell,
}

/// Where the export files are written, and the year and timestamp they carry.
#[derive(Debug, clap::Args)]
pub struct ExportArgs {
    /// Directory to write `frozen.csv` and `frozen_metadata.csv` into.
    /// Defaults to the current working directory.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<std::path::PathBuf>,

    /// Latest publication year to include. Defaults to this year.
    #[arg(long)]
    pub year: Option<u16>,

    /// ISO-8601 timestamp for the export metadata. Defaults to now.
    #[arg(long)]
    pub now: Option<String>,
}

/// Where the `PubChem` tree `JSON` files are written, and the year they carry.
#[derive(Debug, clap::Args)]
pub struct ExportPubchemTreeArgs {
    /// Directory to write the six tree JSON files into.
    /// Defaults to the current working directory.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<std::path::PathBuf>,

    /// Latest publication year to include. Defaults to this year.
    #[arg(long)]
    pub year: Option<u16>,
}

/// Archive LOTUS SMILES from Wikidata as a tar.gz with manifest and checksums.
///
/// By default the archive is written to disk and validated without contacting
/// Zenodo (`--dry-run`). Real publishing requires `--publish` **and** the
/// `ZENODO_TOKEN` environment variable; CI never sets either.
#[derive(Debug, clap::Args)]
pub struct ArchiveArgs {
    /// Directory to write the archive files into. Defaults to the current
    /// working directory.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<std::path::PathBuf>,

    /// Validate the Zenodo metadata and upload plan without contacting Zenodo.
    /// This is the default; pass `--publish` without `--dry-run` to push.
    #[arg(long, requires = "publish")]
    pub dry_run: bool,

    /// Publish the completed archive to Zenodo using `ZENODO_TOKEN`.
    /// Requires either `ZENODO_ROOT_DEPOSITION_ID` (new version) or nothing
    /// (initial record). CI must not set this flag.
    #[arg(long, default_value_t = false)]
    pub publish: bool,

    /// Creator name required by Zenodo metadata.
    #[arg(long, default_value = "The LOTUS Initiative")]
    pub creator: String,

    /// ISO-8601 timestamp for the archive manifest. Defaults to now.
    #[arg(long)]
    pub now: Option<String>,
}

/// Every filter the web explorer exposes, and the limits a terminal wants.
///
/// The four `--no-*` nomenclatural flags make this the fifth boolean here. A clap
/// args struct is a flat mirror of the command line, one field per flag, and
/// folding four independent switches into a nested struct would mean the flag
/// names stop being the field names -- which is the property that keeps
/// `docs_in_sync` worth having.
#[allow(
    clippy::struct_excessive_bools,
    reason = "clap derives one field per flag, and the field names are what `docs_in_sync` checks"
)]
#[derive(Debug, clap::Args)]
struct SearchArgs {
    /// Taxon name, scientific name, QID, or `*` for every compound with an occurrence.
    #[arg(short, long, default_value = "")]
    taxon: String,

    /// Match the taxon's accepted name alone, ignoring its synonyms.
    #[arg(long, default_value_t = false)]
    no_accepted_synonyms: bool,

    /// Ignore the basionym, the name the taxon was first described under.
    #[arg(long, default_value_t = false)]
    no_basionyms: bool,

    /// Ignore the original combination, the binomial as first published.
    #[arg(long, default_value_t = false)]
    no_protonyms: bool,

    /// Ignore a replacement name (nomen novum) and the name it replaced.
    #[arg(long, default_value_t = false)]
    no_replacements: bool,

    /// SMILES or an MDL molfile (V2000/V3000) to search by structure.
    #[arg(short, long, default_value = "")]
    structure: String,

    /// How to search: exact (that compound only), substructure, or similarity.
    #[arg(long, value_enum, default_value_t = StructureSearch::Exact)]
    structure_search: StructureSearch,

    /// Tanimoto cutoff for a similarity search, 0 to 1.
    #[arg(long, default_value_t = lotus_model::DEFAULT_STRUCTURE_THRESHOLD, value_parser = parse_threshold)]
    threshold: f64,

    /// Lowest molecular mass, in daltons.
    #[arg(long, default_value_t = 0.0)]
    mass_min: f64,

    /// Highest molecular mass, in daltons.
    #[arg(long, default_value_t = 10_000.0)]
    mass_max: f64,

    /// Restrict to compounds a reference reports. A DOI (`10.1021/JF60160A010`)
    /// or a Wikidata QID; both are resolved before the search runs, the same way
    /// the browser resolves them.
    #[arg(long, value_name = "DOI_OR_QID")]
    reference: Option<String>,

    /// Earliest publication year.
    #[arg(long, default_value_t = 1800)]
    year_min: u16,

    /// Latest publication year. Defaults to this year.
    #[arg(long)]
    year_max: Option<u16>,

    /// Restrict to a molecular formula, e.g. `C17H12O7`. Subscripts are fine.
    #[arg(long, value_name = "FORMULA")]
    formula: Option<String>,

    /// Atom-count bounds for an element: `--carbon 5..20`, or `--carbon ..20`.
    #[arg(long = "carbon", value_name = "MIN..MAX")]
    carbon: Option<Range>,
    #[arg(long = "hydrogen", value_name = "MIN..MAX")]
    hydrogen: Option<Range>,
    #[arg(long = "nitrogen", value_name = "MIN..MAX")]
    nitrogen: Option<Range>,
    #[arg(long = "oxygen", value_name = "MIN..MAX")]
    oxygen: Option<Range>,
    #[arg(long = "phosphorus", value_name = "MIN..MAX")]
    phosphorus: Option<Range>,
    #[arg(long = "sulfur", value_name = "MIN..MAX")]
    sulfur: Option<Range>,

    /// Halogen presence: required, excluded, or allowed.
    #[arg(long = "fluorine", value_enum, default_value_t = Presence::Allowed)]
    fluorine: Presence,
    #[arg(long = "chlorine", value_enum, default_value_t = Presence::Allowed)]
    chlorine: Presence,
    #[arg(long = "bromine", value_enum, default_value_t = Presence::Allowed)]
    bromine: Presence,
    #[arg(long = "iodine", value_enum, default_value_t = Presence::Allowed)]
    iodine: Presence,

    /// How many rows to return. Unlimited by default, which is what a filter you
    /// piped somewhere wants; pass a number when the result is large and you only
    /// want the top of it. Note that an unlimited search buffers every row in
    /// memory, so an unfiltered one is hundreds of thousands.
    #[arg(short, long)]
    limit: Option<usize>,

    /// How the rows are written to stdout.
    #[arg(short, long, value_enum, default_value_t = Format::Table)]
    format: Format,

    /// Print the SPARQL that would be sent, and nothing else. No network.
    #[arg(long)]
    explain: bool,

    /// Log to stderr: error, warn, info, debug.
    #[arg(long, value_enum, default_value_t = LogLevel::Error)]
    log: LogLevel,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum StructureSearch {
    Exact,
    Substructure,
    Similarity,
}

impl From<StructureSearch> for lotus_model::SmilesSearchType {
    fn from(value: StructureSearch) -> Self {
        match value {
            StructureSearch::Exact => Self::Exact,
            StructureSearch::Substructure => Self::Substructure,
            StructureSearch::Similarity => Self::Similarity,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Presence {
    Allowed,
    Required,
    Excluded,
}

impl From<Presence> for lotus_model::ElementState {
    fn from(value: Presence) -> Self {
        match value {
            Presence::Allowed => Self::Allowed,
            Presence::Required => Self::Required,
            Presence::Excluded => Self::Excluded,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

/// An inclusive `MIN..MAX`, either end optional.
#[derive(Debug, Clone, Copy)]
struct Range {
    min: u16,
    max: u16,
}

impl std::str::FromStr for Range {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bad = || format!("expected MIN..MAX, MIN.. or ..MAX, got {value:?}");
        match value.split_once("..") {
            Some((min, "")) => Ok(Self {
                min: 0,
                max: min.parse().map_err(|_| bad())?,
            }),
            Some(("", max)) => Ok(Self {
                min: 0,
                max: max.parse().map_err(|_| bad())?,
            }),
            Some((min, max)) => Ok(Self {
                min: min.parse().map_err(|_| bad())?,
                max: max.parse().map_err(|_| bad())?,
            }),
            None => {
                // A bare number is an upper bound: `--carbon 5` reads as "at
                // most 5 carbons", which is the bound a chemist narrows with.
                // A lower bound alone selects almost nothing, so `20..` is how
                // you ask for that.
                let max: u16 = value.parse().map_err(|_| bad())?;
                Ok(Self { min: 0, max })
            }
        }
    }
}

/// A Tanimoto cutoff is a similarity, so anything outside 0..=1 is a typo.
fn parse_threshold(value: &str) -> Result<f64, String> {
    let parsed: f64 = value
        .parse()
        .map_err(|_| format!("{value:?} is not a number"))?;
    if (0.0..=1.0).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(format!("{parsed} is outside 0..=1"))
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("lotus: could not start the async runtime: {err}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run(cli)) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("lotus: {err:#}");
            let _ = io::stderr().flush();
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::Search(args) => search(args).await,
        Command::Curate(args) => curate::run(&args).await,
        Command::Completions(args) => {
            let mut command = <Cli as clap::CommandFactory>::command();
            let name = command.get_name().to_string();
            clap_complete::generate(args.shell, &mut command, name, &mut io::stdout().lock());
            Ok(ExitCode::SUCCESS)
        }
        Command::Man => {
            clap_mangen::Man::new(<Cli as clap::CommandFactory>::command())
                .render(&mut io::stdout().lock())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Export(args) => export(args).await,
        Command::ExportPubchemTree(args) => export_pubchem_tree(args).await,
        Command::Archive(args) => Box::pin(archive(args)).await,
    }
}

/// The criteria the flags describe.
///
/// Split out of `search` so it is testable without a network, a terminal or a
/// running binary. Not a cosmetic extraction: the four `--no-*` nomenclatural
/// flags are negations, and nothing downstream can tell "not passed" from
/// "passed, meaning the opposite" unless this is the thing under test. With the
/// mapping inlined, a dropped `!` turned `--no-basionyms` into a way of
/// *enabling* the basionym search and every test still passed.
fn criteria_from_args(args: &SearchArgs, year_max: u16) -> lotus_model::SearchCriteria {
    let mut criteria = lotus_model::SearchCriteria::up_to_year(year_max);

    criteria.taxon.clone_from(&args.taxon);
    // The model defaults all four to on and each flag is a negation, so these
    // are unconditional assignments rather than defaults the flags turn off.
    criteria.taxon_names = lotus_model::TaxonNomenclature {
        accepted_synonyms: !args.no_accepted_synonyms,
        basionyms: !args.no_basionyms,
        protonyms: !args.no_protonyms,
        replacements: !args.no_replacements,
    };
    criteria.structure.clone_from(&args.structure);
    criteria.structure_search = args.structure_search.into();
    criteria.structure_threshold = args.threshold;
    criteria.mass_min = args.mass_min;
    criteria.mass_max = args.mass_max;
    criteria.reference = args.reference.clone().unwrap_or_default();
    criteria.year_min = args.year_min;
    criteria.year_max = year_max;
    criteria.formula_exact = args.formula.clone().unwrap_or_default();
    apply_ranges(
        &mut criteria,
        args.carbon,
        args.hydrogen,
        args.nitrogen,
        args.oxygen,
        args.phosphorus,
        args.sulfur,
    );
    criteria.f_state = args.fluorine.into();
    criteria.cl_state = args.chlorine.into();
    criteria.br_state = args.bromine.into();
    criteria.i_state = args.iodine.into();

    criteria.formula_enabled = flags_ask_for_formula(&criteria);

    criteria
}

async fn search(args: SearchArgs) -> anyhow::Result<ExitCode> {
    let year_max = args.year_max.unwrap_or_else(current_year);
    let criteria = criteria_from_args(&args, year_max);
    lotus_model::validate_criteria(&criteria, year_max)?;

    let request = lotus_search::SearchRequest::new(criteria.clone(), year_max)
        .with_limit(args.limit.unwrap_or(usize::MAX));

    if args.explain {
        // `--explain` exists to print the query without waiting for an endpoint, so
        // it must not touch the network even to resolve a taxon. That is why the
        // resolved QID cannot be `None` for a bare QID: `--taxon Q21754 --explain`
        // once printed the no-taxon query, with no `P171*` anywhere, and said
        // nothing. A wrong query is worse than no flag.
        //
        // `*` and an empty box stay `None`, which `build_base_query` reads out of
        // `criteria.taxon` precisely because neither resolves to a QID.
        let trimmed = args.taxon.trim();
        let resolved = if trimmed.is_empty() || trimmed == "*" {
            None
        } else if lotus_search::is_qid(trimmed) {
            Some(trimmed.to_ascii_uppercase())
        } else {
            // A name has to be looked up to become a QID, and looking it up is
            // the network call this flag exists to avoid. So it refuses rather
            // than printing the query for the un-resolved request, which is the
            // bug being fixed here.
            anyhow::bail!(
                "--explain does not resolve taxon names, because that needs the \
                 network. Pass a Wikidata QID (--taxon Q21754), or '*', or drop \
                 --explain to run the search that resolves it."
            );
        };
        // The reference has the same shape problem and a worse failure mode, so it
        // refuses for the same reason the taxon does: a DOI has to be resolved to
        // its QID to be usable, and that is a network call this flag avoids.
        //
        // It is a refusal rather than a warning, which the structure below is not,
        // because an un-resolved reference does not change the spelling of the
        // query -- it removes the constraint. `with_filters` only emits a QID, so
        // printing the query would print a search for every reported compound and
        // present it as the query for the reference that was typed.
        let reference = criteria.reference.trim();
        if !reference.is_empty() && !lotus_search::is_qid(reference) {
            anyhow::bail!(
                "--explain does not resolve a DOI to its QID, because that needs \
                 the network. Pass a Wikidata QID (--reference Q23118), or drop \
                 --explain to run the search that resolves it."
            );
        }
        // An exact structure search resolves the structure to a compound first, and
        // that resolution needs the network this flag avoids, so the printed query is
        // the pre-resolution one. It prints anyway and warns on stderr rather than
        // refusing, which would remove the flag for its most ordinary use. The taxon
        // case refuses instead because there the unresolved query is a *different
        // question* (`P171*` missing entirely), not a different spelling.
        let structure = lotus_search::normalize_structure(&criteria.structure);
        if !structure.is_empty() && criteria.structure_search == SmilesSearchType::Exact {
            eprintln!(
                "lotus: this is the query before the structure is resolved. An exact \
                 structure search resolves {structure:?} to a compound first and runs \
                 a query seeded on that compound, so this is not the query that will \
                 run."
            );
        }

        let query = lotus_search::build_execution_query(&request, resolved.as_deref());
        println!("{query}");
        return Ok(ExitCode::SUCCESS);
    }

    let http = lotus_search::reqwest_client::ReqwestClient::new()?;
    let result = lotus_search::search(&http, &request).await?;

    if let Some(taxon) = &result.taxon {
        for note in &taxon.notes {
            eprintln!("lotus: {note}");
        }
    }

    // Both conditions, because `result.truncated` alone is not enough. It means the
    // *parser* dropped rows, while the limit is pushed to the endpoint, so a capped
    // result arrives already capped and the parser sees exactly the rows asked for.
    // Hitting the limit is the only signal the CLI has, and a weak one: there may be
    // more, not that there is.
    if let Some(limit) = args.limit
        && (result.truncated || result.rows.len() >= limit)
    {
        eprintln!(
            "lotus: showing {} rows, which is the limit you set; there may be more. \
             Raise --limit, or drop it for all of them.",
            result.rows.len()
        );
    }

    let stdout = io::stdout().lock();
    write_rows(stdout, &result, args.format, args.explain)?;
    let _ = io::stderr().flush();
    Ok(ExitCode::SUCCESS)
}

/// Export the full LOTUS compound–taxon–reference graph as `frozen.csv` and
/// `frozen_metadata.csv`, written to a directory the caller picks.
///
/// No filters: this is the bulk exporter, the same source as
/// `lotus_exporter.py`, and it writes everything it can fetch. The year and
/// timestamp the files carry come from the flags (or the clock), and this
/// function is where the native file I/O lives — `run_lotus_exporter` itself
/// performs none, so it stays wasm-clean.
async fn export(args: ExportArgs) -> anyhow::Result<ExitCode> {
    let year = args.year.unwrap_or_else(current_year);
    let now = args.now.unwrap_or_else(now_iso_utc);

    let http = lotus_search::reqwest_client::ReqwestClient::new()?;
    let endpoint = lotus_search::Endpoint::new(lotus_search::Service::Qlever);

    let (frozen_csv, metadata_csv, summary) =
        lotus_explore_core::export::pipeline::run_lotus_exporter(&http, endpoint, &now, year)
            .await?;

    let output_dir = args
        .output
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(output_dir)?;

    let frozen_path = output_dir.join("frozen.csv");
    let metadata_path = output_dir.join("frozen_metadata.csv");
    std::fs::write(&frozen_path, frozen_csv)?;
    std::fs::write(&metadata_path, metadata_csv)?;

    eprintln!(
        "lotus: wrote {} rows ({} compounds, {} taxa, {} references) to {} and {}",
        summary.rows_written,
        summary.compounds,
        summary.taxa,
        summary.references,
        frozen_path.display(),
        metadata_path.display(),
    );
    Ok(ExitCode::SUCCESS)
}

/// Build `PubChem`-format trees (biological, chemical, `NPClassifier`) as `JSON`,
/// writing six files to an output directory. No network transport beyond the
/// SPARQL endpoint and the `NPClassifier` cache fetch.
async fn export_pubchem_tree(args: ExportPubchemTreeArgs) -> anyhow::Result<ExitCode> {
    let now = now_iso_utc();

    let http = lotus_search::reqwest_client::ReqwestClient::new()?;
    let endpoint = lotus_search::Endpoint::new(lotus_search::Service::Qlever);

    let (bio_full, chem_full, npc_full, bio_pc, chem_pc, npc_pc) =
        run_pubchem_tree(&http, endpoint, &now).await?;

    let output_dir = args
        .output
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(output_dir)?;

    let write = |name: &str, content: &str| -> anyhow::Result<()> {
        let path = output_dir.join(name);
        std::fs::write(&path, content)?;
        eprintln!("lotus: wrote {} to {}", content.len(), path.display());
        Ok(())
    };

    write("lotus_biological_tree.json", &bio_full)?;
    write("lotus_chemical_tree.json", &chem_full)?;
    write("lotus_npclassifier_tree.json", &npc_full)?;
    write("lotus_biological_tree_pubchem.json", &bio_pc)?;
    write("lotus_chemical_tree_pubchem.json", &chem_pc)?;
    write("lotus_npclassifier_tree_pubchem.json", &npc_pc)?;

    Ok(ExitCode::SUCCESS)
}

/// Validate `SOURCE_CODE_URL` for the source-code related identifier Zenodo
/// expects: an `https` URL with a `/commit/<40-hex-SHA>` suffix.
///
/// Returns the canonical string or a clap-style error ready for `anyhow`.
fn validate_source_code_url(url: &str) -> anyhow::Result<String> {
    if !url.starts_with("https://") {
        anyhow::bail!("SOURCE_CODE_URL must be an https URL");
    }
    let Some((_, revision)) = url.rsplit_once("/commit/") else {
        anyhow::bail!("SOURCE_CODE_URL must have a /commit/<sha> suffix");
    };
    if revision.len() != 40 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        anyhow::bail!("SOURCE_CODE_URL revision must be a 40-character commit SHA");
    }
    Ok(url.to_string())
}

/// Build the Zenodo `DepositMetadataUpdate` for a LOTUS archive.
fn zenodo_metadata(
    title: &str,
    creator: &str,
    publication_date: chrono::NaiveDate,
    source_code_url: &str,
) -> anyhow::Result<zenodo_rs::DepositMetadataUpdate> {
    let metadata = zenodo_rs::DepositMetadataUpdate::builder()
        .title(title)
        .upload_type(zenodo_rs::UploadType::Dataset)
        .publication_date(publication_date)
        .description_html(
            "<p>InChIKey-deduplicated Wikidata SMILES for chemical structures with P703 \
             (found in taxon), fetched from QLever. Isomeric P2017 values take \
             precedence over canonical P233 values. The archive includes a \
             smiles-rs parse-error report and query provenance.</p>",
        )
        .creator_named(creator)
        .access_right(zenodo_rs::AccessRight::Open)
        .license("cc-by-4.0")
        .keyword("LOTUS")
        .keyword("Wikidata")
        .keyword("SMILES")
        .related_identifier(
            zenodo_rs::RelatedIdentifier::builder()
                .identifier(source_code_url)
                .relation("isDerivedFrom")
                .scheme("url")
                .resource_type("software")
                .build()
                .map_err(|e| anyhow::anyhow!("build related identifier: {e}"))?,
        )
        .community_identifier("the-lotus-initiative")
        .build()
        .map_err(|e| anyhow::anyhow!("build Zenodo metadata: {e}"))?;
    Ok(metadata)
}

/// Run the Wikidata archive: fetch, deduplicate, package as tar.gz, and
/// optionally publish to Zenodo. By default only writes files to disk; real
/// publishing requires `--publish` and `ZENODO_TOKEN`, and `--dry-run`
/// validates the plan without contacting Zenodo.
async fn archive(args: ArchiveArgs) -> anyhow::Result<ExitCode> {
    let now = args.now.unwrap_or_else(now_iso_utc);
    let fetched_at = parse_iso_utc(&now)?;

    let http = lotus_search::reqwest_client::ReqwestClient::new()?;
    let endpoint = lotus_search::Endpoint::new(lotus_search::Service::Qlever);

    let (artifacts, summary) = Box::pin(lotus_explore_core::export::archive::run_archive(
        &http, endpoint, &now,
    ))
    .await
    .map_err(|e| anyhow::anyhow!("archive failed: {e}"))?;

    let (archive_path, manifest_path, checksums_path) =
        write_archive_files(args.output.as_ref(), fetched_at, &artifacts)?;

    eprintln!(
        "lotus: wrote archive ({} source, {} deduplicated, {} missing-key, {} parse errors) to {}",
        summary.source_records,
        summary.records,
        summary.missing_inchi_key_records,
        summary.parse_errors,
        archive_path.display(),
    );

    if args.publish {
        let root_id = root_deposition_id()?;
        let title = format!("LOTUS Wikidata SMILES — {}", fetched_at.date_naive());
        if args.dry_run {
            // Validate the metadata + upload plan without contacting Zenodo.
            let _metadata = zenodo_metadata(
                &title,
                &args.creator,
                fetched_at.date_naive(),
                &source_code_url_env()?,
            )?;
            dry_run_zenodo(&archive_path, &manifest_path, &checksums_path, root_id);
        } else {
            Box::pin(publish_to_zenodo(
                &title,
                &args.creator,
                fetched_at.date_naive(),
                root_id,
                &archive_path,
                &manifest_path,
                &checksums_path,
            ))
            .await?;
        }
    }

    Ok(ExitCode::SUCCESS)
}

/// Write the archive tar.gz, manifest JSON, and SHA256SUMS file to `output_dir`.
fn write_archive_files(
    output: Option<&std::path::PathBuf>,
    fetched_at: chrono::DateTime<chrono::Utc>,
    artifacts: &lotus_explore_core::export::archive::ArchiveArtifacts,
) -> anyhow::Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> {
    let output_dir: &std::path::Path =
        output.map_or_else(|| std::path::Path::new("."), |p| p.as_path());
    std::fs::create_dir_all(output_dir)?;

    let filename = format!(
        "lotus-wikidata-smiles-{}.tar.gz",
        fetched_at.format("%Y%m%dT%H%M%SZ")
    );
    let archive_path = output_dir.join(&filename);
    let manifest_path = output_dir.join("lotus-wikidata-smiles.manifest.json");
    let checksums_path = output_dir.join("lotus-wikidata-smiles.SHA256SUMS");

    std::fs::write(&archive_path, &artifacts.archive)?;
    std::fs::write(&manifest_path, &artifacts.manifest)?;
    std::fs::write(&checksums_path, &artifacts.checksums)?;

    Ok((archive_path, manifest_path, checksums_path))
}

/// Resolve the root deposition ID from `ZENODO_ROOT_DEPOSITION_ID`, if set.
fn root_deposition_id() -> anyhow::Result<Option<zenodo_rs::DepositionId>> {
    match std::env::var("ZENODO_ROOT_DEPOSITION_ID") {
        Ok(value) => parse_deposition_id(&value),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => anyhow::bail!("cannot read ZENODO_ROOT_DEPOSITION_ID: {e}"),
    }
}

/// Parse a raw string as a Zenodo root deposition ID.
fn parse_deposition_id(value: &str) -> anyhow::Result<Option<zenodo_rs::DepositionId>> {
    let id: u64 = value
        .parse()
        .map_err(|_| anyhow::anyhow!("ZENODO_ROOT_DEPOSITION_ID is not a valid deposition ID"))?;
    if id == 0 {
        anyhow::bail!("ZENODO_ROOT_DEPOSITION_ID must be non-zero");
    }
    Ok(Some(zenodo_rs::DepositionId(id)))
}

/// Validate the Zenodo metadata + upload plan without writing to disk or
/// contacting Zenodo.
fn dry_run_zenodo(
    archive_path: &std::path::Path,
    manifest_path: &std::path::Path,
    checksums_path: &std::path::Path,
    root_id: Option<zenodo_rs::DepositionId>,
) {
    let files = match archive_upload_paths(archive_path, manifest_path, checksums_path) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("lotus: Zenodo dry run failed — upload spec: {e}");
            return;
        }
    };
    let plan = root_id.map_or_else(
        || "publish the initial Zenodo record".to_string(),
        |root_id| format!("publish a new version from deposition {}", root_id.0),
    );
    eprintln!(
        "lotus: Zenodo dry run OK — metadata valid, {} files ready, plan: {plan}",
        files.len()
    );
}

/// Actually publish the archive to Zenodo. Tries the sandbox token first, then
/// falls back to the production token.
async fn publish_to_zenodo(
    title: &str,
    creator: &str,
    publication_date: chrono::NaiveDate,
    root_id: Option<zenodo_rs::DepositionId>,
    archive_path: &std::path::Path,
    manifest_path: &std::path::Path,
    checksums_path: &std::path::Path,
) -> anyhow::Result<()> {
    let source_code_url = source_code_url_env()?;
    let metadata = zenodo_metadata(title, creator, publication_date, &source_code_url)?;
    let upload_specs = archive_upload_paths(archive_path, manifest_path, checksums_path)?;

    let published = if let Ok(client) = zenodo_rs::ZenodoClient::from_sandbox_env() {
        eprintln!("lotus: publishing to Zenodo sandbox (ZENODO_SANDBOX_TOKEN)");
        Box::pin(publish_archive(&client, root_id, &metadata, upload_specs)).await
    } else {
        let client = zenodo_rs::ZenodoClient::from_env()
            .map_err(|e| anyhow::anyhow!("read ZENODO_TOKEN: {e}"))?;
        eprintln!("lotus: publishing to Zenodo production (ZENODO_TOKEN)");
        Box::pin(publish_archive(&client, root_id, &metadata, upload_specs)).await
    }?;

    let record_id = published.record.id;
    eprintln!("lotus: published to Zenodo, record ID: {}", record_id.0);
    Ok(())
}

/// Run the publish workflow (ensure draft, upload, publish) for one client and
/// one set of args, branching on whether a root deposition ID is present.
async fn publish_archive(
    client: &zenodo_rs::ZenodoClient,
    root_id: Option<zenodo_rs::DepositionId>,
    metadata: &zenodo_rs::DepositMetadataUpdate,
    upload_specs: Vec<zenodo_rs::UploadSpec>,
) -> Result<zenodo_rs::PublishedRecord, anyhow::Error> {
    if let Some(root_id) = root_id {
        let draft = client
            .ensure_editable_draft(root_id)
            .await
            .map_err(|e| anyhow::anyhow!("ensure editable draft: {e}"))?;
        client
            .publish_dataset(draft.id, metadata, upload_specs)
            .await
            .map_err(|e| anyhow::anyhow!("publish to Zenodo: {e}"))
    } else {
        client
            .create_and_publish_dataset(metadata, upload_specs)
            .await
            .map_err(|e| anyhow::anyhow!("create and publish to Zenodo: {e}"))
    }
}

fn apply_ranges(
    criteria: &mut lotus_model::SearchCriteria,
    carbon: Option<Range>,
    hydrogen: Option<Range>,
    nitrogen: Option<Range>,
    oxygen: Option<Range>,
    phosphorus: Option<Range>,
    sulfur: Option<Range>,
) {
    let set = |range: Option<Range>, min: &mut u16, max: &mut u16, default_max: u16| {
        if let Some(range) = range {
            *min = range.min;
            *max = if range.max == u16::MAX {
                default_max
            } else {
                range.max
            };
        }
    };
    set(
        carbon,
        &mut criteria.c_min,
        &mut criteria.c_max,
        lotus_model::element_max::C,
    );
    set(
        hydrogen,
        &mut criteria.h_min,
        &mut criteria.h_max,
        lotus_model::element_max::H,
    );
    set(
        nitrogen,
        &mut criteria.n_min,
        &mut criteria.n_max,
        lotus_model::element_max::N,
    );
    set(
        oxygen,
        &mut criteria.o_min,
        &mut criteria.o_max,
        lotus_model::element_max::O,
    );
    set(
        phosphorus,
        &mut criteria.p_min,
        &mut criteria.p_max,
        lotus_model::element_max::P,
    );
    set(
        sulfur,
        &mut criteria.s_min,
        &mut criteria.s_max,
        lotus_model::element_max::S,
    );
}

/// Days since the Unix epoch to the civil (year, month, day) containing that day.
///
/// Howard Hinnant's `civil_from_days`, returning the full date. Split from the
/// clock read so the arithmetic can be checked against dates whose answers are
/// known, rather than only against "the year is roughly now".
#[must_use]
pub fn civil_from_days(days: i64) -> (u16, u8, u8) {
    // Shift the epoch to 0000-03-01, so a leap day lands at the end of the
    // shifted year and the era is exactly 400 years of whole days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    // Day of the era's year, 0..=365. The published form of this line also has
    // `+ doe / 36_524 - doe / 146_096`, and those two terms are dropped here on
    // purpose: the truncating divisions that follow cancel them exactly, so
    // they cannot change the result for any input.
    let yoe = (doe - doe / 1_460) / 365; // [0, 399]
    // The year the shifted year starts in, which is the year *after* January and
    // February belong to: on this calendar a shifted year runs from 1 March.
    let shifted = yoe + era * 400;
    // Day of the shifted year, counted from its 1 March.
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    // Month within the shifted year, March first and so zero-based. The last two
    // are January and February of the next civil year.
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if mp < 10 { shifted } else { shifted + 1 };

    // The reference types are u16/u8 throughout the CLI, so a clock set before
    // 1970 saturates rather than wrapping to a plausible-looking recent year.
    (
        u16::try_from(year).unwrap_or(u16::MAX),
        u8::try_from(month).unwrap_or(u8::MAX),
        u8::try_from(day).unwrap_or(u8::MAX),
    )
}

/// Unix seconds to the civil year containing them.
///
/// Whole days, floored: a second before midnight is still the day it started in,
/// and the year turns over at the first of January rather than at the first of
/// the day after.
#[must_use]
pub fn year_from_unix_seconds(seconds: i64) -> u16 {
    civil_year_from_days(seconds.div_euclid(86_400))
}

/// Days since the Unix epoch to the civil year containing that day.
///
/// Delegates to [`civil_from_days`] and keeps only the year, so the arithmetic
/// lives in one place and the two functions cannot drift apart.
#[must_use]
pub fn civil_year_from_days(days: i64) -> u16 {
    civil_from_days(days).0
}

/// The current calendar year, for the default upper year bound.
///
/// `lotus-model` takes the year as an argument rather than reading a clock, so
/// that it stays pure; a binary is where a clock belongs.
#[must_use]
pub fn current_year() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().cast_signed());
    year_from_unix_seconds(seconds)
}

/// The current UTC time as an ISO-8601 stamp (`YYYY-MM-DDTHH:MM:SSZ`), for the
/// export metadata document.
///
/// No `chrono` dependency: the same civil-calendar arithmetic that
/// [`civil_from_days`] uses, extended to hour, minute and second.
#[must_use]
pub fn now_iso_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = duration.as_secs().cast_signed();
    let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
    let time_of_day = seconds.rem_euclid(86_400);
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}
///
/// `--formula` and `--carbon` both filter by formula, and only the first sets
/// `formula_enabled` on the criteria. This is not
/// [`lotus_model::SearchCriteria::has_formula_filter`], which reports false
/// until `formula_enabled` is set -- asking it here would be circular.
#[must_use]
fn flags_ask_for_formula(criteria: &lotus_model::SearchCriteria) -> bool {
    use lotus_model::ElementState;
    !criteria.formula_exact.trim().is_empty()
        || criteria
            .element_ranges()
            .iter()
            .any(|(_, min, max, default_max)| *min > 0 || *max < *default_max)
        || criteria.f_state != ElementState::Allowed
        || criteria.cl_state != ElementState::Allowed
        || criteria.br_state != ElementState::Allowed
        || criteria.i_state != ElementState::Allowed
}

/// Parse an ISO-8601 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`) into a `DateTime`.
///
/// The archive needs a `DateTime` for the filename; the existing `now_iso_utc`
/// produces one without `chrono`, so this closes the loop without a round-trip
/// through string formatting.
fn parse_iso_utc(s: &str) -> anyhow::Result<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| anyhow::anyhow!("--now must be an ISO-8601 UTC timestamp: {e}"))
}

/// Read and validate `SOURCE_CODE_URL` for the source-code related identifier
/// Zenodo expects: an `https` URL with a `/commit/<40-hex-SHA>` suffix.
fn source_code_url_env() -> anyhow::Result<String> {
    let url = std::env::var("SOURCE_CODE_URL")
        .map_err(|_| anyhow::anyhow!("SOURCE_CODE_URL is not set"))?;
    validate_source_code_url(&url)
}

/// Build `UploadSpec`s from three file paths for the Zenodo upload.
fn archive_upload_paths(
    archive: &std::path::Path,
    manifest: &std::path::Path,
    checksums: &std::path::Path,
) -> anyhow::Result<Vec<zenodo_rs::UploadSpec>> {
    let specs = zenodo_rs::UploadSpec::from_named_paths([
        (
            archive.file_name().map_or_else(
                || "lotus-wikidata-smiles.tar.gz".to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            archive,
        ),
        (
            manifest.file_name().map_or_else(
                || "lotus-wikidata-smiles.manifest.json".to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            manifest,
        ),
        (
            checksums.file_name().map_or_else(
                || "lotus-wikidata-smiles.SHA256SUMS".to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            checksums,
        ),
    ])
    .map_err(|e| anyhow::anyhow!("prepare Zenodo upload specs: {e}"))?;
    Ok(specs)
}

#[cfg(test)]
#[path = "main/range_tests.rs"]
mod range_tests;

#[cfg(test)]
#[path = "main/civil_year_tests.rs"]
mod civil_year_tests;

#[cfg(test)]
#[path = "main/nomenclature_flag_tests.rs"]
mod nomenclature_flag_tests;

#[cfg(test)]
#[path = "main/archive_tests.rs"]
mod archive_tests;
