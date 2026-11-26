// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Server-side filter injection, QID sanitization, and `LIMIT` appending.

use std::fmt::Write as _;

use lotus_model::{ElementState, SearchCriteria};

use super::rewrite::{escape_sparql_string, normalize_digits_expr};

/// Append a `LIMIT`.
#[must_use]
pub fn limit_query(base: &str, limit: usize) -> String {
    // `usize::MAX` is how a caller says "no limit". Emitting `LIMIT 18446744073709551615`
    // instead would ask the endpoint for a bound it cannot honour, and would show up
    // in the query the `--explain` flag prints.
    if limit == usize::MAX {
        return base.trim_end().to_string();
    }
    format!("{}\nLIMIT {limit}", base.trim_end())
}

/// The Wikidata local name a reference field names, and `None` when it names
/// none.
///
/// The single gate between that field and every `wd:` interpolation in this
/// module. `None` for a blank field (no constraint, which is what blank means)
/// and for anything that is not `Q` followed by at least one ASCII digit --
/// a DOI, prose, or a string carrying SPARQL syntax.
///
/// Callers that want a bad reference *refused* rather than ignored ask the model
/// crate instead: [`lotus_model::looks_like_reference_qid`] to accept, and their
/// own message to reject. This one exists because a query builder cannot report
/// an error, so it has to be safe on its own.
pub(super) fn sanitize_qid(value: &str) -> Option<String> {
    let trimmed = value.trim();
    lotus_model::looks_like_reference_qid(trimmed).then(|| trimmed.to_ascii_uppercase())
}

pub(super) fn sanitize_qid_or_sentinel(value: &str) -> String {
    sanitize_qid(value).unwrap_or_else(|| "Q0".to_string())
}

/// Apply the active filters to a base query.
///
/// The fragments go after the base query's closing brace, not inside the
/// innermost subquery. That is where `inject` puts them and where the endpoint
/// has always been asked to evaluate them; moving them would change the query
/// plan, and the plan is tuned.
#[must_use]
pub fn with_filters(base: &str, criteria: &SearchCriteria, year_max: u16) -> String {
    let mut required = String::new();
    let mut filters = String::new();

    if criteria.has_mass_filter() {
        let _ = write!(
            filters,
            "FILTER(?compound_mass >= {:.6} && ?compound_mass <= {:.6})",
            criteria.mass_min, criteria.mass_max
        );
        let _ = writeln!(required, "?c wdt:P2067 ?compound_mass .");
    }

    // The year filter constrains `?ref_date`, which the middle subquery already
    // projects. So it needs no triple of its own and no join: just a FILTER on a
    // variable that is already in scope, guarded by BOUND.
    //
    // The earlier form reached the date through a required `?r wdt:P577 ?_year_date` at the
    // outermost level, wrong twice over. `?r` is bound by the occurrence block, optional in
    // the taxon-free shapes, so a *required* triple on an unbound `?r` goes fresh and asks
    // for every dated reference in Wikidata -- 30 s and zero rows, the timeout reading as
    // "nothing matched". And even with a sentinel in front, a narrower range returned
    // nothing while a wider one returned hundreds, which a correct engine cannot do for two
    // queries differing in one digit.
    //
    // BOUND matters as the sentinel did: with no reference the metadata OPTIONAL leaves
    // `?ref_date` unbound, and `YEAR()` of an unbound value is an error that drops the row.
    // Returning nothing for a compound with no reference is the right answer; enumerating
    // Wikidata to find that out is not.
    if criteria.has_year_filter(year_max) {
        let _ = write!(
            filters,
            "FILTER(BOUND(?ref_date) && YEAR(?ref_date) >= {} && YEAR(?ref_date) <= {})",
            criteria.year_min, criteria.year_max
        );
    }

    // A reference constrains which *reported* compounds are wanted, so it binds
    // `?r` rather than filtering a projected value -- the same shape the year
    // filter above uses, and for the same reason: `?r` is the item the reference
    // is.
    //
    // It is a `VALUES` and not a `FILTER(?r = wd:Q…)` because that is the
    // indexed form. It does join a row whose `?r` is unbound -- a compound with
    // no occurrence -- so the callers pick a base query that requires the
    // occurrence when a reference is the only constraint.
    //
    // Only a QID is emitted, and only after [`looks_like_reference_qid`] has said
    // the field is one. `wd:` takes an IRI local name, so `{ } . # ;` are syntax
    // there, not text -- and `escape_sparql_string` escapes none of them,
    // because none of them are syntax inside `"…"`. Splicing the field through
    // that escaper let a caller close the `VALUES`, append triples of its own and
    // comment out the rest of the query: an injected query run against a public
    // endpoint, and echoed back in `SearchResponse.query`.
    //
    // Refusing is the caller's job and stays there, because only the caller can
    // tell the reader why: the API turns a bad reference into a 400, the browser
    // into a form error, and this crate builds queries rather than running them.
    // What this guarantees is the half a caller cannot repair afterwards -- no
    // reference, from any caller, becomes query syntax. A reference this drops is
    // an ignored constraint, which is the smaller failure.
    //
    // Uppercased here for the same reason the taxon field is: a QID's local name
    // is case-sensitive in SPARQL, so `wd:q23118` is a different item from
    // `wd:Q23118`, and every gate that admits a reference admits a lowercase one.
    if let Some(qid) = sanitize_qid(&criteria.reference) {
        let _ = writeln!(required, "VALUES ?r {{ wd:{qid} }}");
        let _ = writeln!(filters, "FILTER(BOUND(?statement_id))");
    }

    formula_filter(criteria, &mut required, &mut filters);

    // A filter that binds nothing of its own is legitimate: the year filter constrains
    // `?ref_date`, already projected by the middle subquery, and adding a `?r wdt:P577
    // ?_year_date` triple to satisfy the old rule made the query join against a variable that
    // goes fresh when the occurrence block matched nothing. The invariant is therefore not
    // "every filter binds" but "every filter names a variable the query already binds" --
    // a property of the text, checked by the contract tests, invisible to these buffers.
    if required.is_empty() && filters.is_empty() {
        return base.to_string();
    }

    // The base query's own closing brace becomes the filters' closing brace, so
    // the fragments land in its outermost WHERE rather than in a new block.
    let Some(without_closing_brace) = base.trim_end().strip_suffix('}') else {
        return format!("{base}\n{required}{filters}");
    };
    format!("{without_closing_brace}\n{required}{filters}\n}}")
}

/// The formula filter: normalise the formula once, then count elements against
/// the normalised string.
///
/// Takes `required` as well as `out` because the filter needs a binding that has
/// to survive into the counts query, and the counts query is built by deleting
/// text from this one -- see [`counts_query`](super::counts_query).
fn formula_filter(criteria: &SearchCriteria, required: &mut String, out: &mut String) {
    if !criteria.has_formula_filter() {
        return;
    }

    // Without the BOUND guard, an unbound `?compound_formula` makes every comparison
    // below evaluate against an error rather than a value.
    //
    // The variable names are part of the query text, and a query's bytes are what a cache
    // key and a shared link derive from, so they are not renamed for tidiness.
    //
    // This triple is why the counts were wrong. `?compound_formula` is bound by an
    // `OPTIONAL` inside `COMPOUND_PROPERTIES`, which `counts_query` deletes as expensive and
    // display-only -- but with a formula filter active it is neither. The counts query kept
    // `FILTER(BOUND(?compound_formula))` over a variable nothing bound, matched no rows,
    // and every card on the page read zero for any search with a formula filter. Binding it
    // here, outside the deleted block and as the mass and year filters bind their own
    // variables, is what makes it survive.
    let _ = writeln!(required, "?c wdt:P274 ?compound_formula .");

    let _ = writeln!(out, "FILTER(BOUND(?compound_formula))");
    let _ = writeln!(out, "BIND(STR(?compound_formula) AS ?_formula_raw)");
    let _ = writeln!(
        out,
        r#"BIND(REPLACE(?_formula_raw, " ", "") AS ?_formula_nospace)"#
    );
    let _ = writeln!(
        out,
        "BIND({} AS ?_formula_norm)",
        normalize_digits_expr("?_formula_nospace")
    );
    // Split before each capital so element symbols become addressable tokens.
    let _ = writeln!(
        out,
        r#"BIND(REPLACE(?_formula_norm, "([A-Z])", "|$1") AS ?_formula_tokens)"#
    );

    // Every BIND first, then every FILTER: a group pattern's FILTERs see the
    // BINDs regardless of order, but interleaving them makes the query harder
    // to read and to diff.
    let mut ranged = Vec::new();
    for (symbol, min, max, default_max) in criteria.element_ranges() {
        if min > 0 || max < default_max {
            let var = bind_element_count(out, symbol);
            ranged.push(format!("FILTER({var} >= {min} && {var} <= {max})"));
        }
    }

    let mut halogens = Vec::new();
    for (symbol, state) in criteria.halogen_states() {
        if state == ElementState::Allowed {
            continue;
        }
        let var = bind_element_count(out, symbol);
        halogens.push(match state {
            ElementState::Required => format!("FILTER({var} > 0)"),
            ElementState::Excluded => format!("FILTER({var} = 0)"),
            ElementState::Allowed => unreachable!("the `Allowed` case is skipped above"),
        });
    }

    for filter in ranged.iter().chain(&halogens) {
        let _ = writeln!(out, "{filter}");
    }

    if let Some(exact) = normalized_exact_formula(criteria) {
        let _ = writeln!(out, "FILTER(?_formula_norm = \"{exact}\")");
    }
}

/// The exact formula to compare against, with subscripts folded to ASCII and
/// whitespace removed. The comparison happens against the endpoint's normalised
/// formula, so a subscript in the input would never match.
fn normalized_exact_formula(criteria: &SearchCriteria) -> Option<String> {
    let without_whitespace: String = criteria
        .formula_exact
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let normalized = lotus_model::normalize_formula_subscripts(&without_whitespace);
    (!normalized.is_empty()).then(|| escape_sparql_string(&normalized))
}

/// Append a `BIND` yielding `symbol`'s count in the tokenised formula, or `0`,
/// and return the variable it binds.
///
/// The token separator is emitted as `\\|` because the pattern goes into a SPARQL string
/// literal, where a backslash is itself escaped.
///
/// Tokenising inserts `|` before *every* capital, so `C6H5COOH` becomes `|C6|H5|C|O|O|H`.
/// Counting the leading `\\|C([0-9]*)` greedily reads the **last** `|C` rather than the
/// first, which is wrong whenever a bare `C` follows a digit-bearing one: benzoic acid
/// tokenises with a trailing bare `|C`, the capture comes back empty, and the count collapses
/// to 1. Measured over the 3,301 distinct formulas on a real taxon, that miscounted
/// `C₂H₅OOCH`.
///
/// Hence a digit-bearing token is preferred, and the greedy match is kept only for formulas
/// with none -- where a single bare token does mean one atom (`CH4`), which the old
/// empty-capture branch already got right.
/// Counts require digits whenever any counted token exists; bare uncounted symbols
/// (`|N|`, `|H)`, end-of-string) only reach the second branch and evaluate to `1`.
/// Terminators allow `|`, end-of-string, or any character outside `[a-z0-9]` so
/// charged ions (`O4⁺`), hydrates (`N2·2`), and parenthesized groups (`H4(`) match
/// without matching two-letter element names (`Cl`, `Ca`).
fn bind_element_count(out: &mut String, symbol: &str) -> String {
    let var = format!("?_count_{}", symbol.to_ascii_lowercase());
    let pattern = format!(r"\\|{symbol}(\\||$|[^a-z0-9])");
    let with_count = format!(r"\\|{symbol}([0-9]+)(\\||$|[^a-z0-9])");
    let counted = format!(r#"REPLACE(?_formula_tokens, "^.*?{with_count}.*", "$1")"#);
    let _ = writeln!(
        out,
        "BIND(IF(REGEX(?_formula_tokens, \"{with_count}\"), xsd:integer({counted}), \
         IF(REGEX(?_formula_tokens, \"{pattern}\"), 1, 0)) AS {var})"
    );
    var
}
