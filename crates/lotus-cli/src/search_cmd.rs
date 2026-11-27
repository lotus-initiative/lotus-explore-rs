// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Handler for `lotus search`.

use std::io::{self, Write};
use std::process::ExitCode;

use lotus_model::SmilesSearchType;

use crate::args::{LogLevel, SearchArgs};
use crate::clock::current_year;
use crate::criteria::criteria_from_args;
use crate::output::write_rows;

pub async fn search(args: SearchArgs) -> anyhow::Result<ExitCode> {
    if !(0.0..=1.0).contains(&args.threshold) {
        anyhow::bail!("--threshold {} is outside 0..=1", args.threshold);
    }
    let clock_year = current_year();
    let criteria = criteria_from_args(&args, clock_year);
    lotus_model::validate_criteria(&criteria, clock_year)?;

    let request = lotus_search::SearchRequest::new(criteria.clone(), clock_year)
        .with_limit(args.limit.unwrap_or(usize::MAX));

    if args.explain {
        return explain_search(&args.taxon, &criteria, &request);
    }

    if matches!(args.log, LogLevel::Info | LogLevel::Debug) {
        eprintln!(
            "lotus: [info] executing search (year_max={}, limit={:?})",
            criteria.year_max, args.limit
        );
    }

    let http = lotus_search::reqwest_client::ReqwestClient::new()?;
    let result = lotus_search::search(&http, &request).await?;

    if args.log == LogLevel::Debug {
        eprintln!("lotus: [debug] SPARQL:\n{}", result.query);
    }
    if matches!(args.log, LogLevel::Info | LogLevel::Debug) {
        eprintln!(
            "lotus: [info] returned {} row(s) (truncated={})",
            result.rows.len(),
            result.truncated
        );
    }

    if let Some(taxon) = &result.taxon {
        for note in &taxon.notes {
            eprintln!("lotus: {note}");
        }
    }

    // The search asks the endpoint for one row past the limit, so `truncated` is
    // exact: it is true only when there were more rows than were kept.
    if result.truncated {
        eprintln!(
            "lotus: showing {} rows, which is the limit you set; there are more. \
             Raise --limit, or drop it for all of them.",
            result.rows.len()
        );
    }

    let stdout = io::stdout().lock();
    write_rows(stdout, &result, args.format, args.explain)?;
    let _ = io::stderr().flush();
    Ok(ExitCode::SUCCESS)
}

fn explain_search(
    raw_taxon: &str,
    criteria: &lotus_model::SearchCriteria,
    request: &lotus_search::SearchRequest,
) -> anyhow::Result<ExitCode> {
    // `--explain` exists to print the query without waiting for an endpoint, so
    // it must not touch the network even to resolve a taxon. That is why the
    // resolved QID cannot be `None` for a bare QID: `--taxon Q21754 --explain`
    // once printed the no-taxon query, with no `P171*` anywhere, and said
    // nothing. A wrong query is worse than no flag.
    //
    // `*` and an empty box stay `None`, which `build_base_query` reads out of
    // `criteria.taxon` precisely because neither resolves to a QID.
    let resolved = resolve_explain_taxon(raw_taxon)?;

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
    if !reference.is_empty() && !lotus_model::is_qid(reference) {
        anyhow::bail!(
            "--explain does not resolve a DOI to its QID, because that needs \
             the network. Pass a Wikidata QID (--reference Q23118), or drop \
             --explain to run the search to resolve it."
        );
    }

    // An exact structure search resolves a non-QID structure to a compound first,
    // and that resolution needs the network this flag avoids, so the printed
    // query is the pre-resolution one. Bare QIDs seed `VALUES ?c` directly.
    let structure = lotus_model::normalize_structure(&criteria.structure);
    let mut resolved_inputs = lotus_search::ResolvedInputs::default();
    if !structure.is_empty() && criteria.structure_search == SmilesSearchType::Exact {
        if lotus_model::is_qid(&structure) {
            resolved_inputs
                .compounds
                .push(structure.to_ascii_uppercase());
        } else {
            eprintln!(
                "lotus: this is the query before the structure is resolved. An exact \
                 structure search resolves {structure:?} to a compound first and runs \
                 a query seeded on that compound, so this is not the query that will \
                 run."
            );
        }
    } else if !structure.is_empty() && lotus_model::names_a_compound(&structure) {
        eprintln!(
            "lotus: this is the query before the structure identifier is resolved. A \
             substructure or similarity search resolves {structure:?} to its SMILES \
             first, so this is not the query that will run."
        );
    }

    let query =
        lotus_search::build_execution_query_with(request, resolved.as_deref(), &resolved_inputs);
    println!("{query}");
    Ok(ExitCode::SUCCESS)
}

fn resolve_explain_taxon(raw_taxon: &str) -> anyhow::Result<Option<String>> {
    let trimmed = raw_taxon.trim();
    if trimmed.is_empty() || trimmed == "*" {
        Ok(None)
    } else if lotus_model::is_qid(trimmed) {
        Ok(Some(trimmed.to_ascii_uppercase()))
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
    }
}
