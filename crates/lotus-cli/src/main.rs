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
}

#[derive(Debug, clap::Args)]
struct CompletionsArgs {
    /// The shell to generate for.
    #[arg(value_enum)]
    shell: clap_complete::Shell,
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
    }
}

/// The criteria the flags describe.
///
/// Split out of `search` so it can be tested without a network, a terminal and
/// a running binary. It is not a cosmetic extraction: the four `--no-*`
/// nomenclatural flags are negations, and nothing downstream can tell the
/// difference between "not passed" and "passed, meaning the opposite" unless
/// this function is the thing under test. With the mapping inlined in `search`,
/// a dropped `!` turned `--no-basionyms` into a way of *enabling* the basionym
/// search, and every test in the crate still passed.
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
        // The point of `--explain` is to see the query without waiting for an
        // endpoint, so it must not touch the network even to resolve a taxon.
        //
        // Which is why the resolved QID cannot simply be `None`. It used to be,
        // and that made `--explain` print the query for a *different request* than
        // the one being asked about: `--taxon Q21754 --explain` printed the
        // no-taxon query, with no `P171*` anywhere in it, and said nothing. The
        // whole purpose of the flag is to be looked at when a result surprises
        // someone, so a wrong query is worse than no flag.
        //
        // A bare QID needs no lookup, so it is passed straight through and the
        // printed query is the real one. `*` and an empty box are `None`, which is
        // correct: `build_base_query` reads that distinction out of
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
        // A structure input is resolved to a compound before an exact search runs,
        // and resolution needs the network this flag exists to avoid. So the query
        // printed here is the pre-resolution one, which for an exact structure
        // search is not the query that will run.
        //
        // Refusing instead would be more honest and would also remove the flag for
        // its most ordinary use, so it prints and says so on stderr, where it does
        // not pollute a pipeline reading the query on stdout. The taxon's own
        // resolution is handled the other way -- by refusing -- because there the
        // unresolved query is a *different question* (`P171*` missing entirely),
        // not merely a different spelling of the same one.
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

    // The limit used to be 100 and nothing said so: 100 rows came back looking
    // like a complete answer, which is the same failure as a query that times out
    // and reports no rows. Both are a partial answer that reads as a whole one.
    //
    // Two conditions, because `result.truncated` alone is not enough. It means the
    // *parser* dropped rows, and the limit is pushed to the endpoint, so a capped
    // result arrives already capped and the parser sees exactly as many rows as
    // were asked for. Hitting the limit is therefore the only signal the CLI has,
    // and it is a weaker one: it says there may be more, not that there are.
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
/// Howard Hinnant's `civil_from_days`, keeping only the year. Split from the
/// clock read so the arithmetic can be checked against dates whose answers are
/// known, rather than only against "the year is roughly now" -- which is the one
/// assertion that would not notice a wrong answer for another eleven months.
#[must_use]
pub fn civil_year_from_days(days: i64) -> u16 {
    // Shift the epoch to 0000-03-01, so a leap day lands at the end of the
    // shifted year and the era is exactly 400 years of whole days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    // Day of the era's year, 0..=365. The published form of this line also has
    // `+ doe / 36_524 - doe / 146_096`, and those two terms are dropped here on
    // purpose: the truncating divisions that follow cancel them exactly, so
    // they cannot change the result for any input. Mutation testing found them
    // -- it reported both as surviving, which is what an equivalent mutant looks
    // like -- and keeping them would mean carrying two dead terms forever to
    // satisfy a formula transcribed from somewhere else.
    let yoe = (doe - doe / 1_460) / 365; // [0, 399]
    // The year the shifted year starts in, which is the year *after* January and
    // February belong to: on this calendar a shifted year runs from 1 March.
    let shifted = yoe + era * 400;
    // Day of the shifted year, counted from its 1 March.
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    // Month within the shifted year, March first and so zero-based. The last two
    // are January and February of the next civil year.
    let month = (5 * doy + 2) / 153;
    let year = if month < 10 { shifted } else { shifted + 1 };

    // The reference year is a u16 throughout the crate, so a clock set before
    // 1970 saturates rather than wrapping to a plausible-looking recent year.
    u16::try_from(year).unwrap_or(u16::MAX)
}

/// Whether the command line asked for formula filtering.
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

#[cfg(test)]
#[path = "main/civil_year_tests.rs"]
mod civil_year_tests;

#[cfg(test)]
#[path = "main/nomenclature_flag_tests.rs"]
mod nomenclature_flag_tests;
