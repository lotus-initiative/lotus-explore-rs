// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Taxon lookup queries and CSV data-row detection.

use super::builders::compounds_by_taxon_query;
use super::dictionary::occurrences_query;
use super::filters::limit_query;
use super::rewrite::escape_sparql_string;

/// A `VALUES` lookup of taxa by scientific name (`P225`).
///
/// An exact match, no `FILTER` and no `LCASE`: `VALUES` is served from an index, and a
/// case-folding scan of every label is what would make this slow. Every taxon search takes
/// this path first, so it stays the fast one -- the common-name fallback is a second round
/// trip precisely so this one need not become a scan.
///
/// Known limit: `wdt:` keeps only the preferred-rank statement, so the 293 non-preferred
/// `P225` values are invisible here. A taxon has one accepted scientific name, so in practice
/// a rounding error -- a real gap rather than a guarantee. See
/// [`taxon_common_name_lookup_query`] for the far larger equivalent gap on `P1843`.
#[must_use]
pub fn taxon_lookup_query(name: &str) -> String {
    format!(
        "PREFIX wdt: <http://www.wikidata.org/prop/direct/>
SELECT ?taxon ?taxon_name ?matched_by
WHERE {{
  VALUES ?taxon_name {{ \"{}\" }}
  ?taxon wdt:P225 ?taxon_name .
  BIND(\"scientific\" AS ?matched_by)
}}",
        escape_sparql_string(name)
    )
}

/// One row when `taxon_qid` has a recorded compound, itself or below it, under
/// any linked name; none otherwise.
///
/// Asked of each taxon sharing a name, so a homonym resolves to the taxon that
/// has data: `Michelia` is a fossil genus with no compound and a plant genus
/// with many, and the lookup lists the fossil first.
#[must_use]
pub fn taxon_has_compounds_query(taxon_qid: &str) -> String {
    limit_query(&occurrences_query(&compounds_by_taxon_query(taxon_qid)), 1)
}

/// Whether a CSV answer holds a row after its header.
#[must_use]
pub fn has_data_row(csv: &[u8]) -> bool {
    csv.split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .nth(1)
        .is_some()
}

/// A lookup of taxa by **common** name (`P1843`).
///
/// The fallback for [`taxon_lookup_query`], which it cannot be shaped like. Three
/// requirements, each found by a name that returned nothing when it should not have:
///
/// **The statement path, not `wdt:`.** `wdt:P1843` collapses a taxon to its
/// preferred-rank value. *Gentiana lutea* (`Q158572`) carries 65 common names and
/// `wdt:` returns one, so a search for `bitterwort` -- one of the other 64 -- found
/// nothing. `p:P1843/ps:P1843` returns all 65. The same collapse costs `P225` 293
/// values out of four million, which is why the scientific path keeps the fast index
/// and only `P1843` pays for the statement walk.
///
/// **The language tag has to go.** `P225` values are untagged literals, so a bare
/// `"Gentiana lutea"` matches; every `P1843` value is tagged (`bitterwort@en`) and a bare
/// literal is not equal to a tagged one in SPARQL. Matching `STR()` against the lexical
/// form drops the tag, which is the point: a common name is the word a reader reaches for
/// in *their* language, so restricting to `@en` would break the feature for most of its
/// users.
///
/// **Case has to go too.** `P1843` stores whatever a curator typed, so the same taxon
/// carries `bitterwort`, `Common wormwood` and `Bijvoet`; `LCASE` on both sides is
/// indifferent to that.
///
/// The cost is a scan of every `P1843` statement -- measured ~2.5s on `QLever` against
/// ~0.3s for the indexed scientific lookup. Hence a fallback rather than a second branch:
/// a search that hits a scientific name never pays it.
#[must_use]
pub fn taxon_common_name_lookup_query(name: &str) -> String {
    format!(
        "PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX p: <http://www.wikidata.org/prop/>
PREFIX ps: <http://www.wikidata.org/prop/statement/>
SELECT ?taxon ?taxon_name ?matched_by
WHERE {{
  ?taxon p:P1843/ps:P1843 ?taxon_name .
  FILTER(LCASE(STR(?taxon_name)) = LCASE(\"{}\"))
  BIND(\"common\" AS ?matched_by)
}}",
        escape_sparql_string(name)
    )
}
