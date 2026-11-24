// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `lotus search` argument parser types and bounds.

use crate::output::Format;
use clap::ValueEnum;

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
pub struct SearchArgs {
    /// Taxon name, scientific name, QID, or `*` for every compound with an occurrence.
    #[arg(short, long, default_value = "")]
    pub taxon: String,

    /// Match the taxon's accepted name alone, ignoring its synonyms.
    #[arg(long, default_value_t = false)]
    pub no_accepted_synonyms: bool,

    /// Ignore the basionym, the name the taxon was first described under.
    #[arg(long, default_value_t = false)]
    pub no_basionyms: bool,

    /// Ignore the original combination, the binomial as first published.
    #[arg(long, default_value_t = false)]
    pub no_protonyms: bool,

    /// Ignore a replacement name (nomen novum) and the name it replaced.
    #[arg(long, default_value_t = false)]
    pub no_replacements: bool,

    /// Compound name, `InChIKey`, Wikidata QID, SMILES, or MDL molfile (V2000/V3000).
    #[arg(short, long, default_value = "")]
    pub structure: String,

    /// How to search: exact (that compound; a pasted structure can also match its stereoisomers), substructure, or similarity.
    #[arg(long, value_enum, default_value_t = StructureSearch::Exact)]
    pub structure_search: StructureSearch,

    /// Tanimoto cutoff for a similarity search, 0 to 1, scored by the IDSM structure service.
    #[arg(long, default_value_t = lotus_model::DEFAULT_STRUCTURE_THRESHOLD)]
    pub threshold: f64,

    /// Lowest molecular mass, in daltons.
    #[arg(long, default_value_t = 0.0)]
    pub mass_min: f64,

    /// Highest molecular mass, in daltons.
    #[arg(long, default_value_t = 10_000.0)]
    pub mass_max: f64,

    /// Restrict to compounds a reference reports. A DOI (`10.1021/JF60160A010`)
    /// or a Wikidata QID; both are resolved before the search runs, the same way
    /// the browser resolves them.
    #[arg(long, value_name = "DOI_OR_QID")]
    pub reference: Option<String>,

    /// Earliest publication year.
    #[arg(long, default_value_t = 1800)]
    pub year_min: u16,

    /// Latest publication year. Defaults to this year.
    #[arg(long)]
    pub year_max: Option<u16>,

    /// Restrict to a molecular formula, e.g. `C17H12O7`. Subscripts are fine.
    #[arg(long, value_name = "FORMULA")]
    pub formula: Option<String>,

    /// Atom-count bounds for an element: `--carbon 5..20`, or `--carbon ..20`.
    #[arg(long = "carbon", value_name = "MIN..MAX")]
    pub carbon: Option<Range>,
    #[arg(long = "hydrogen", value_name = "MIN..MAX")]
    pub hydrogen: Option<Range>,
    #[arg(long = "nitrogen", value_name = "MIN..MAX")]
    pub nitrogen: Option<Range>,
    #[arg(long = "oxygen", value_name = "MIN..MAX")]
    pub oxygen: Option<Range>,
    #[arg(long = "phosphorus", value_name = "MIN..MAX")]
    pub phosphorus: Option<Range>,
    #[arg(long = "sulfur", value_name = "MIN..MAX")]
    pub sulfur: Option<Range>,

    /// Halogen presence: required, excluded, or allowed.
    #[arg(long = "fluorine", value_enum, default_value_t = Presence::Allowed)]
    pub fluorine: Presence,
    #[arg(long = "chlorine", value_enum, default_value_t = Presence::Allowed)]
    pub chlorine: Presence,
    #[arg(long = "bromine", value_enum, default_value_t = Presence::Allowed)]
    pub bromine: Presence,
    #[arg(long = "iodine", value_enum, default_value_t = Presence::Allowed)]
    pub iodine: Presence,

    /// How many rows to return. Unlimited by default, which is what a filter you
    /// piped somewhere wants; pass a number when the result is large and you only
    /// want the top of it. Note that an unlimited search buffers every row in
    /// memory, so an unfiltered one is hundreds of thousands.
    #[arg(short, long)]
    pub limit: Option<usize>,

    /// How the rows are written to stdout.
    #[arg(short, long, value_enum, default_value_t = Format::Table)]
    pub format: Format,

    /// Print the SPARQL that would be sent, and nothing else. No network.
    #[arg(long)]
    pub explain: bool,

    /// Log to stderr: error, warn, info, debug.
    #[arg(long, value_enum, default_value_t = LogLevel::Error)]
    pub log: LogLevel,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StructureSearch {
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
pub enum Presence {
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
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

/// An inclusive `MIN..MAX`, either end optional.
#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub min: u16,
    pub max: u16,
}

impl std::str::FromStr for Range {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bad = || format!("expected MIN..MAX, MIN.. or ..MAX, got {value:?}");
        match value.split_once("..") {
            Some((min, "")) => Ok(Self {
                min: min.parse().map_err(|_| bad())?,
                max: u16::MAX,
            }),
            Some(("", max)) => Ok(Self {
                min: 0,
                max: max.parse().map_err(|_| bad())?,
            }),
            Some((min, max)) => {
                let min: u16 = min.parse().map_err(|_| bad())?;
                let max: u16 = max.parse().map_err(|_| bad())?;
                Ok(Self { min, max })
            }
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
