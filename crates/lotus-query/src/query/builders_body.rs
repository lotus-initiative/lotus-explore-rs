// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Inner subquery assembly helpers for non-structure compound queries.

use super::Nomenclature;
use super::filters::sanitize_qid;
use super::nomenclature::taxon_ancestry_pattern_with_roots;
use super::prefixes::{COMPOUND_PROPERTIES, PREFIXES, REFERENCE_METADATA, SELECT_CLAUSE};

/// Whether an occurrence statement (`P703`) has to be present.
///
/// Two different questions: `Required` asks what was reported and where, `Optional` asks
/// what exists, including compounds nobody has tied to an organism. Those are the `*` case
/// and the nothing-provided case, and conflating them made asking for everything silently
/// answer the narrower question.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Occurrence {
    Required,
    Optional,
}

pub(super) fn compounds_query_with(
    compound_seed: Option<&[String]>,
    taxon_qid: Option<(&str, &Nomenclature)>,
    occurrence: Occurrence,
) -> String {
    compounds_query_with_closure(compound_seed, taxon_qid, occurrence, &[])
}

pub(super) fn compounds_query_with_closure(
    compound_seed: Option<&[String]>,
    taxon_qid: Option<(&str, &Nomenclature)>,
    occurrence: Occurrence,
    roots: &[String],
) -> String {
    let ancestry =
        taxon_qid
            .filter(|(qid, _)| *qid != "*")
            .map_or_else(String::new, |(qid, nomenclature)| {
                format!(
                    "\n          {}",
                    taxon_ancestry_pattern_with_roots(qid, *nomenclature, roots)
                )
            });

    // What makes `?c` a chemical entity rather than anything else with a
    // `found in taxon` statement.
    //
    // `P703` is not specific to metabolites: Wikidata uses it for genes and proteins too,
    // so a taxon search with no compound test returns `Q21629845` -- a protein-coding gene
    // for an oxidoreductase subunit -- beside the metabolites, with nothing in the row
    // saying which it is. `P235`, the InChIKey, discriminates: that gene has neither `P235`
    // nor `P233` (checked), a metabolite has both. The required-occurrence branch already
    // carried this triple, so only the optional-occurrence and compound-seeded shapes leaked
    // genes.
    //
    // `P233` stays optional: requiring both would drop real compounds with a canonical
    // SMILES and no InChIKey, and dropping a metabolite to keep the shape tidy is the
    // wrong trade in the other direction.
    let chemical = "?c wdt:P235 ?compound_inchikey .";

    let Some(qid) = compound_seed else {
        return match occurrence {
            Occurrence::Required => match taxon_qid {
                Some((q, _)) if q != "*" => taxon_compounds_query_body(&ancestry),
                _ => all_required_compounds_query_body(),
            },
            Occurrence::Optional => optional_compounds_query_body(chemical, &ancestry),
        };
    };

    let (core, outer_ref_metadata) =
        seeded_compounds_core(qid, taxon_qid.is_some(), chemical, &ancestry);

    format!(
        "{PREFIXES}
{SELECT_CLAUSE}
WHERE {{
  {{
    SELECT *
    WHERE {{
      {{
        SELECT *
        WHERE {{
          {core}
        }}
      }}
      {outer_ref_metadata}
      {COMPOUND_PROPERTIES}
    }}
  }}
}}"
    )
}

pub(super) fn all_required_compounds_query_body() -> String {
    format!(
        "{PREFIXES}
{SELECT_CLAUSE}
WHERE {{
  {{
    SELECT *
    WHERE {{
      {{
        SELECT *
        WHERE {{
          {{
            SELECT *
            WHERE {{
              ?c wdt:P235 ?compound_inchikey .
              ?c p:P703 ?statement_id .
              ?statement_id ps:P703 ?t ;
                            wikibase:rank ?_stmt_rank .
              FILTER(?_stmt_rank != wikibase:DeprecatedRank)
              ?t wdt:P225 ?taxon_name .
            }}
          }}
          ?statement_id prov:wasDerivedFrom ?ref_node .
          ?ref_node pr:P248 ?r .
        }}
      }}
      {COMPOUND_PROPERTIES}
    }}
  }}
  {REFERENCE_METADATA}
}}"
    )
}

fn taxon_compounds_query_body(ancestry: &str) -> String {
    format!(
        "{PREFIXES}
{SELECT_CLAUSE}
WHERE {{
  {{
    SELECT *
    WHERE {{
      {{
        SELECT *
        WHERE {{
          {{
            SELECT DISTINCT ?t
            WHERE {{{ancestry}
            }}
          }}
          ?t wdt:P225 ?taxon_name .
          ?c wdt:P235 ?compound_inchikey ;
             p:P703 ?statement_id .
          ?statement_id ps:P703 ?t ;
                        wikibase:rank ?_stmt_rank .
          FILTER(?_stmt_rank != wikibase:DeprecatedRank)
        }}
      }}
      ?statement_id prov:wasDerivedFrom ?ref_node .
      ?ref_node pr:P248 ?r .
      {COMPOUND_PROPERTIES}
    }}
  }}
  {REFERENCE_METADATA}
}}"
    )
}

pub(super) fn optional_compounds_query_body(chemical: &str, ancestry: &str) -> String {
    format!(
        "{PREFIXES}
{SELECT_CLAUSE}
WHERE {{
  {{
    SELECT *
    WHERE {{
      {chemical}
      OPTIONAL {{
        {{
          SELECT *
          WHERE {{
            {{
              SELECT *
              WHERE {{
                {{
                  SELECT *
                  WHERE {{
                    ?c wdt:P235 ?compound_inchikey ;
                       p:P703 ?statement_id .
                    ?statement_id ps:P703 ?t ;
                                  wikibase:rank ?_stmt_rank .
                    FILTER(?_stmt_rank != wikibase:DeprecatedRank)
                    ?t wdt:P225 ?taxon_name .{ancestry}
                  }}
                }}
                ?statement_id prov:wasDerivedFrom ?ref_node .
                ?ref_node pr:P248 ?r .
              }}
            }}
            {REFERENCE_METADATA}
          }}
        }}
      }}
    }}
  }}
  {COMPOUND_PROPERTIES}
}}"
    )
}

fn seeded_compounds_core(
    qid: &[String],
    has_taxon: bool,
    chemical: &str,
    ancestry: &str,
) -> (String, &'static str) {
    // The occurrence triple becomes optional here because a compound with no
    // occurrence data is exactly what a name search looks for; see
    // `exact_compound_query`.
    //
    // The block is wrapped in its own subquery with the seed repeated inside,
    // because `OPTIONAL` costs this endpoint twenty times what the same patterns
    // cost alone. Measured on Q3613679: 4.3s as a bare `OPTIONAL`, 0.2s as a
    // seeded subquery, same 20 rows. The block is what makes it slow, not the
    // patterns -- `?c p:P703 ?statement_id . ?statement_id ps:P703 ?t` and the three hops
    // after run in 0.1s outside an `OPTIONAL` -- so the subquery hands the planner a
    // left side of its own size instead of one it has to assume. Without the
    // repeated `VALUES` inside, the subquery inherits the binding and the slow plan
    // comes back.
    let mut sanitized: Vec<String> = qid
        .iter()
        .filter_map(|q| sanitize_qid(q))
        .map(|q| format!("wd:{q}"))
        .collect();
    if sanitized.is_empty() {
        sanitized.push("wd:Q0".to_string());
    }
    let seeded = sanitized.join(" ");
    // A taxon here is a real constraint, so the occurrence block stops being optional.
    //
    // Leaving it optional with the taxon ancestry inside constrains nothing:
    // `VALUES ?c` at the top level is unconditional, and the OPTIONAL only
    // decides which occurrences are *shown*. `--structure Q23118
    // --structure-search exact --taxon Gentiana` then returned Q23118 with an
    // empty taxon cell, and would have returned a compound reported only in some
    // other taxon while still listing that other taxon. A taxon asked for and not
    // applied is worse than a taxon not given, because the answer looks filtered.
    //
    // Optional stays right for the taxon-less case, hence branching on the taxon
    // rather than removing the branch. Once a taxon is named the question is
    // "reported *in* this taxon", and a compound with no such occurrence is not an
    // answer -- it is nothing, and the empty result says so.
    let occurrences = format!(
        "?c p:P703 ?statement_id .\
              ?statement_id ps:P703 ?t ;\
                            wikibase:rank ?_stmt_rank ;\
                            prov:wasDerivedFrom ?ref_node .\
              FILTER(?_stmt_rank != wikibase:DeprecatedRank) \
              ?ref_node pr:P248 ?r .\
              ?t wdt:P225 ?taxon_name .{ancestry}"
    );
    if has_taxon {
        let taxon_closure = if ancestry.is_empty() {
            String::new()
        } else {
            format!(
                "\n          {{\n            SELECT DISTINCT ?t\n            WHERE {{{ancestry}\n            }}\n          }}"
            )
        };
        (
            format!(
                "VALUES ?c {{ {seeded} }}
          {chemical} \
          ?c p:P703 ?statement_id .\
          ?statement_id ps:P703 ?t ;\
                        wikibase:rank ?_stmt_rank ;\
                        prov:wasDerivedFrom ?ref_node .\
          FILTER(?_stmt_rank != wikibase:DeprecatedRank) \
          ?ref_node pr:P248 ?r .\
          ?t wdt:P225 ?taxon_name .{taxon_closure}"
            ),
            REFERENCE_METADATA,
        )
    } else {
        (
            format!(
                "VALUES ?c {{ {seeded} }}
          {chemical} \
          OPTIONAL {{
            {{ SELECT * WHERE {{
              VALUES ?c {{ {seeded} }}
              {occurrences}
              {REFERENCE_METADATA}
            }} }}
          }}"
            ),
            "",
        )
    }
}
