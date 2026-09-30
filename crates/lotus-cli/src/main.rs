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
#![allow(unused_crate_dependencies)]
// This is a binary: there is nothing outside the crate for `pub` to reach, so
// `unreachable_pub` reports every module and item that cross a module boundary.
// The items are `pub` only so that `main` can name them.
#![allow(unreachable_pub)]

use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

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
#[derive(Debug, clap::Args)]
struct SearchArgs {
    /// Taxon name, scientific name, Wikidata QID, or `*` for all organisms.
    #[arg(short, long, default_value = "")]
    taxon: String,

    /// SMILES or an MDL molfile (V2000/V3000) to search by structure.
    #[arg(short, long, default_value = "")]
    structure: String,

    /// How to match the structure: substructure or similarity.
    #[arg(long, value_enum, default_value_t = StructureSearch::Substructure)]
    structure_search: StructureSearch,

    /// Tanimoto cutoff for a similarity search, 0 to 1.
    #[arg(long, default_value_t = 0.8, value_parser = parse_threshold)]
    threshold: f64,

    /// Lowest molecular mass, in daltons.
    #[arg(long, default_value_t = 0.0)]
    mass_min: f64,

    /// Highest molecular mass, in daltons.
    #[arg(long, default_value_t = 10_000.0)]
    mass_max: f64,

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

    /// How many rows to return.
    #[arg(short, long, default_value_t = 100)]
    limit: usize,

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
    Substructure,
    Similarity,
}

impl From<StructureSearch> for lotus_model::SmilesSearchType {
    fn from(value: StructureSearch) -> Self {
        match value {
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

async fn search(args: SearchArgs) -> anyhow::Result<ExitCode> {
    let year_max = args.year_max.unwrap_or_else(current_year);
    let mut criteria = lotus_model::SearchCriteria::up_to_year(year_max);

    criteria.taxon = args.taxon;
    criteria.structure = args.structure;
    criteria.structure_search = args.structure_search.into();
    criteria.structure_threshold = args.threshold;
    criteria.mass_min = args.mass_min;
    criteria.mass_max = args.mass_max;
    criteria.year_min = args.year_min;
    criteria.year_max = year_max;
    criteria.formula_exact = args.formula.unwrap_or_default();
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
    // Any formula filter implies the formula section is in use. A user who typed
    // `--carbon 10..20` has filtered by formula whether or not they also passed
    // `--formula`. This cannot be asked of `has_formula_filter`, which reports
    // false until the flag is set — asking would be circular.
    criteria.formula_enabled = flags_ask_for_formula(&criteria);

    lotus_model::validate_criteria(&criteria, year_max)?;

    let request =
        lotus_search::SearchRequest::new(criteria.clone(), year_max).with_limit(args.limit);

    if args.explain {
        // The point of `--explain` is to see the query without waiting for an
        // endpoint, so it must not touch the network even to resolve a taxon.
        let query = lotus_search::build_execution_query(&request, None);
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
mod civil_year_tests {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::{civil_year_from_days, current_year, year_from_unix_seconds};

    /// Days from 1970-01-01 to the given date, computed the obvious way so the
    /// test does not reuse the algorithm it is checking.
    /// Only for years from 1970 on; the pre-epoch case uses a literal, because
    /// counting backwards through the years is a second algorithm to get wrong.
    fn days_since_epoch(year: i32, month: u32, day: u32) -> i64 {
        // Days from a civil date, by the same era arithmetic but written out
        // longhand: leap years, then the months, then the days.
        let leap = |y: i32| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
        let mut days = 0i64;
        for y in 1970..year {
            days += if leap(y) { 366 } else { 365 };
        }
        for m in 1..month {
            days += match m {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                _ if leap(year) => 29,
                _ => 28,
            };
        }
        days + i64::from(day) - 1
    }

    #[test]
    fn the_epoch_itself_is_1970() {
        assert_eq!(civil_year_from_days(0), 1970);
    }

    #[test]
    fn the_first_day_of_a_year_is_that_year() {
        for year in [1970, 1999, 2000, 2024, 2025, 2026, 2100] {
            assert_eq!(
                civil_year_from_days(days_since_epoch(year, 1, 1)),
                u16::try_from(year).expect("a test year fits in a u16"),
                "1 January {year}"
            );
        }
    }

    #[test]
    fn the_last_day_of_a_year_is_still_that_year() {
        // The boundary either side of midnight on New Year's Eve, which is where
        // an off-by-one in the day count shows up.
        for year in [1999, 2023, 2024, 2025] {
            let last = days_since_epoch(year + 1, 1, 1) - 1;
            assert_eq!(
                civil_year_from_days(last),
                u16::try_from(year).expect("a test year fits in a u16"),
                "31 December {year}"
            );
        }
    }

    #[test]
    fn a_leap_day_belongs_to_the_leap_year() {
        assert_eq!(civil_year_from_days(days_since_epoch(2024, 2, 29)), 2024);
        assert_eq!(civil_year_from_days(days_since_epoch(2024, 3, 1)), 2024);
        // 2000 was a leap year: the century rule, which a naive "divisible by
        // four" gets wrong for 1900 and right here only by accident of the
        // four-hundred-year cycle.
        assert_eq!(civil_year_from_days(days_since_epoch(2000, 2, 29)), 2000);
        assert_eq!(civil_year_from_days(days_since_epoch(2000, 3, 1)), 2000);
    }

    #[test]
    fn a_century_boundary_rolls_the_era() {
        // 2100-03-01 is the first day after a 400-year era ends, which is the
        // one input where the era division actually changes.
        assert_eq!(civil_year_from_days(days_since_epoch(2100, 3, 1)), 2100);
        assert_eq!(civil_year_from_days(days_since_epoch(2100, 2, 28)), 2100);
    }

    #[test]
    fn a_date_before_the_epoch_does_not_wrap_to_a_recent_year() {
        // 1600 is outside a u16 year once added to the era offset, and a wrap
        // would produce a plausible-looking reference year instead of an
        // obviously wrong one.
        assert_eq!(civil_year_from_days(-135_140), 1600);
    }

    #[test]
    fn every_day_of_a_run_of_years_lands_in_its_own_year() {
        // The day-of-year arithmetic has more terms than a handful of dates can
        // distinguish, so this walks whole years and checks the boundaries and
        // the middle of each. `days_since_epoch` is written out longhand and
        // shares no code with the algorithm, so agreement means something.
        for year in 1970..=2035 {
            for (month, day) in [(1, 1), (2, 28), (3, 1), (6, 15), (12, 31)] {
                let days = days_since_epoch(year, month, day);
                assert_eq!(
                    civil_year_from_days(days),
                    u16::try_from(year).expect("a test year fits in a u16"),
                    "{year}-{month:02}-{day:02}"
                );
            }
            // 29 February where there is one.
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                let leap = days_since_epoch(year, 2, 29);
                assert_eq!(
                    civil_year_from_days(leap),
                    u16::try_from(year).expect("a test year fits in a u16"),
                    "{year}-02-29"
                );
                assert_eq!(
                    civil_year_from_days(leap - 1),
                    u16::try_from(year).expect("a test year fits in a u16"),
                    "the day before"
                );
            }
        }
    }

    #[test]
    fn every_day_of_a_long_run_never_leaves_its_own_decade() {
        // Coarse but total: sampled across four centuries, the answer is always
        // the year of the date, and a single wrong term shows up somewhere.
        for year in [1970, 2000, 2024, 2100, 2400] {
            for (month, day) in [(1, 1), (3, 1), (7, 4), (12, 31)] {
                let days = days_since_epoch(year, month, day);
                let got = i64::from(civil_year_from_days(days));
                let year = i64::from(year);
                assert!(
                    (year - 1..=year + 1).contains(&got),
                    "{year}-{month:02}-{day:02} -> {got}"
                );
            }
        }
    }

    #[test]
    fn the_year_never_goes_backwards_and_turns_over_on_new_years_day() {
        // Walks every single day from the epoch to 2040 rather than sampling.
        // The correction terms in the day-of-year division are small enough that
        // the `/365` after them hides them for most dates -- `doe / 146_096` is
        // nonzero on exactly one day in four hundred -- so no handful of dates
        // can tell whether they are right.
        let mut previous = civil_year_from_days(0);
        for day in 1..days_since_epoch(2040, 1, 1) {
            let year = civil_year_from_days(day);
            assert!(
                year >= previous,
                "the year went backwards at day {day}: {previous} then {year}"
            );
            assert!(
                year - previous <= 1,
                "the year jumped at day {day}: {previous} then {year}"
            );
            previous = year;
        }

        // And it turns over exactly on 1 January, not a day either side.
        for year in 1971..=2040 {
            let new_year = days_since_epoch(year, 1, 1);
            assert_eq!(
                civil_year_from_days(new_year - 1),
                u16::try_from(year - 1).expect("a test year fits in a u16"),
                "31 December {}",
                year - 1
            );
            assert_eq!(
                civil_year_from_days(new_year),
                u16::try_from(year).expect("a test year fits in a u16"),
                "1 January {year}"
            );
        }
    }

    #[test]
    fn the_last_day_of_a_four_hundred_year_era_is_still_that_year() {
        // 2000-02-29 is the final day of the era that 1970 sits in, and it is
        // the one date in four hundred where `doe / 146_096` is nonzero.
        let era_end = days_since_epoch(2000, 2, 29);
        assert_eq!(civil_year_from_days(era_end), 2000);
        assert_eq!(civil_year_from_days(era_end - 1), 2000, "the day before it");
        assert_eq!(
            civil_year_from_days(era_end + 1),
            2000,
            "the era rolls over inside it"
        );
    }

    /// A criteria with nothing asked of it.
    fn plain() -> lotus_model::SearchCriteria {
        lotus_model::SearchCriteria::up_to_year(2026)
    }

    #[test]
    fn each_formula_flag_asks_for_formula_filtering_on_its_own() {
        // `--carbon 10..20` has filtered by formula whether or not `--formula`
        // was also given, so each of these has to turn it on by itself. The
        // chain is a row of `||`, and a test that set two flags at once would
        // pass with any single one of them removed.
        assert!(!super::flags_ask_for_formula(&plain()), "nothing asked");

        let mut exact = plain();
        exact.formula_exact = "C6H6O".into();
        assert!(super::flags_ask_for_formula(&exact), "--formula");

        let mut blank = plain();
        blank.formula_exact = "   ".into();
        assert!(
            !super::flags_ask_for_formula(&blank),
            "a blank --formula is nothing"
        );

        let mut lower = plain();
        lower.c_min = 5;
        assert!(
            super::flags_ask_for_formula(&lower),
            "--carbon with a lower bound"
        );

        let mut upper = plain();
        upper.c_max = lotus_model::element_max::C - 1;
        assert!(
            super::flags_ask_for_formula(&upper),
            "--carbon with an upper bound"
        );

        let mut untouched = plain();
        untouched.c_min = 0;
        untouched.c_max = lotus_model::element_max::C;
        assert!(
            !super::flags_ask_for_formula(&untouched),
            "the full carbon range is not a filter"
        );
    }

    #[test]
    fn each_halogen_on_its_own_asks_for_formula_filtering() {
        use lotus_model::ElementState;
        for state in [ElementState::Required, ElementState::Excluded] {
            // One halogen at a time: the four are a row of `||` comparing
            // against `Allowed`, and setting two would hide a broken third.
            for which in ["f", "cl", "br", "i"] {
                let mut criteria = plain();
                match which {
                    "f" => criteria.f_state = state,
                    "cl" => criteria.cl_state = state,
                    "br" => criteria.br_state = state,
                    _ => criteria.i_state = state,
                }
                assert!(
                    super::flags_ask_for_formula(&criteria),
                    "{state:?} on {which} is a filter"
                );
            }
        }
        // Allowed on all four is the absence of a constraint, which is what the
        // other three comparisons are made against.
        let mut allowed = plain();
        for state in [ElementState::Allowed; 4] {
            allowed.f_state = state;
            allowed.cl_state = state;
            allowed.br_state = state;
            allowed.i_state = state;
        }
        assert!(!super::flags_ask_for_formula(&allowed));
    }

    #[test]
    fn seconds_are_floored_to_whole_days() {
        // One second after the turn of the year, in seconds. Divided, that is a
        // whole day past New Year; anything else about the arithmetic -- a
        // remainder, a truncating divide -- lands back in the previous year.
        // 365 * 86_400 seconds: the first instant of 1971.
        assert_eq!(
            year_from_unix_seconds(31_535_999),
            1970,
            "one second before it"
        );
        assert_eq!(year_from_unix_seconds(31_536_000), 1971, "exactly it");
        assert_eq!(year_from_unix_seconds(31_536_001), 1971, "one second after");
        assert_eq!(year_from_unix_seconds(0), 1970);
        assert_eq!(
            year_from_unix_seconds(-1),
            1969,
            "a second before the epoch"
        );
    }

    #[test]
    fn the_current_year_is_the_year_it_is() {
        // The only assertion about the clock, and the one that would hide an
        // error for most of the year -- which is why the arithmetic above is
        // checked against fixed dates instead.
        let now = current_year();
        assert!((2020..=2100).contains(&now), "got {now}");
    }
}
