// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for a live curation lookup, kept in their own file so the module
//! itself stays about the module.
//!
//! The fixtures are SPARQL JSON results as `QLever` and the WDQS both return
//! them: `results` is an object holding a `bindings` array. They are recorded
//! bytes rather than constructed values, so a change in the wire shape has to be
//! made here deliberately, where the mistake would otherwise be invisible.
//!
//! Nothing here touches the network. Every fixture is a recorded answer and the
//! transport is scripted, so there is nothing to refresh: `cargo test -p
//! lotus-curation`.

mod fixtures {
    pub(super) const COMPOUND_FOUND: &str = r#"{
        "head": { "vars": [ "compound", "canonical", "iso", "inchi", "formula", "mass" ] },
        "results": { "bindings": [ {
            "compound": { "type": "uri", "value": "http://www.wikidata.org/entity/Q16521" },
            "canonical": { "type": "literal", "value": "CCO" },
            "formula": { "type": "literal", "value": "C2H6O" },
            "mass": { "type": "literal", "value": "46.07" }
        } ] }
    }"#;

    pub(super) const COMPOUND_LABEL_INSTEAD_OF_QID: &str = r#"{
        "results": { "bindings": [ {
            "compound": { "type": "literal", "value": "Quercetin" }
        } ] }
    }"#;

    pub(super) const TAXON_FOUND: &str = r#"{
        "results": { "bindings": [ {
            "taxon": { "type": "uri", "value": "http://www.wikidata.org/entity/Q16521" }
        } ] }
    }"#;

    pub(super) const REFERENCE_FOUND: &str = r#"{
        "results": { "bindings": [ {
            "ref": { "type": "uri", "value": "http://www.wikidata.org/entity/Q100" }
        } ] }
    }"#;

    pub(super) const NOTHING_FOUND: &str = r#"{
        "head": { "vars": [ "compound" ] },
        "results": { "bindings": [] }
    }"#;

    // `QLever` and the WDQS both answer an `ASK` in the results format, wrapped.
    // The bare literal is what a mirror answers with, and both must be readable.
    pub(super) const ASK_TRUE: &str = r#"{"head":{},"boolean":true}"#;
    pub(super) const ASK_FALSE: &str = r#"{"head":{},"boolean":false}"#;
    pub(super) const ASK_TRUE_BARE: &str = "true";
    pub(super) const ASK_FALSE_BARE: &str = "false";
}

mod unit {
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::super::*;
    use crate::parse_tsv;

    fn row(name: &str, smiles: &str, taxon: Option<&str>, doi: Option<&str>) -> CurationInputRow {
        CurationInputRow {
            name: name.into(),
            smiles: smiles.into(),
            taxon: taxon.map(std::borrow::ToOwned::to_owned),
            doi: doi.map(std::borrow::ToOwned::to_owned),
        }
    }

    fn known() -> crate::ConvertedStructure {
        crate::ConvertedStructure {
            inchikey: Some("LFQSCWFLJHTTHZ-UHFFFAOYSA-N".into()),
            canonical_smiles: Some("CCO".into()),
            ..crate::ConvertedStructure::default()
        }
    }

    fn compound_qid(qid: &str) -> WikidataLookup {
        WikidataLookup {
            compound: Some(WikidataCompound {
                qid: qid.into(),
                ..WikidataCompound::default()
            }),
            taxon_qid: Some("Q16521".into()),
            reference_qid: None,
            has_occurrence: Some(false),
        }
    }

    #[test]
    fn a_row_with_no_key_is_not_reported_as_new() {
        // The distinction that matters: "could not look it up" and "it is not
        // there" both produce no compound, and only one of them means a
        // statement should be written.
        let unknown = crate::ConvertedStructure::default();
        assert!(!unknown.is_known());
        let result = to_result_row(
            &row("X", "CCO", None, None),
            &unknown,
            &WikidataLookup::default(),
        );
        assert_eq!(result.status, CurationStatus::Error);
        assert!(
            result.note.contains("no InChIKey"),
            "the row must say why: {}",
            result.note
        );
    }

    #[test]
    fn a_compound_that_is_already_there_needs_nothing() {
        let mut lookup = compound_qid("Q1");
        lookup.has_occurrence = Some(true);
        lookup.taxon_qid = Some("Q16521".into());
        let result = to_result_row(
            &row("X", "CCO", Some("Gentiana lutea"), None),
            &known(),
            &lookup,
        );
        assert_eq!(result.status, CurationStatus::ExistingComplete);
        assert!(
            result.quickstatements.is_empty(),
            "nothing to submit: {:?}",
            result.quickstatements
        );
    }

    #[test]
    fn a_compound_with_no_taxon_in_the_row_needs_nothing() {
        // The compound is there and the row asks for nothing more, so there is
        // no gap to report.
        let lookup = WikidataLookup {
            compound: Some(WikidataCompound {
                qid: "Q1".into(),
                ..WikidataCompound::default()
            }),
            taxon_qid: None,
            reference_qid: None,
            has_occurrence: None,
        };
        let result = to_result_row(&row("X", "CCO", None, None), &known(), &lookup);
        assert_eq!(result.status, CurationStatus::ExistingComplete);
        assert!(
            result.quickstatements.is_empty(),
            "{:?}",
            result.quickstatements
        );
    }

    #[test]
    fn a_compound_that_is_there_but_lacks_the_occurrence_gets_one_statement() {
        let result = to_result_row(
            &row("X", "CCO", Some("Gentiana lutea"), None),
            &known(),
            &compound_qid("Q1"),
        );
        assert_eq!(result.status, CurationStatus::ExistingNeedsUpdates);
        assert_eq!(result.quickstatements.len(), 1);
        assert!(
            result.quickstatements[0].contains("wd:Q16521"),
            "{:?}",
            result.quickstatements
        );
    }

    #[test]
    fn a_taxon_wikidata_does_not_have_leaves_the_row_pending_not_complete() {
        // The row asked for an occurrence in a taxon that could not be resolved.
        // Calling that "complete" tells the curator to submit nothing, and the
        // finding is then lost without anybody noticing.
        let lookup = WikidataLookup {
            compound: Some(WikidataCompound {
                qid: "Q1".into(),
                ..WikidataCompound::default()
            }),
            taxon_qid: None,
            reference_qid: None,
            has_occurrence: None,
        };
        let result = to_result_row(
            &row("X", "CCO", Some("Gentiana lutea"), None),
            &known(),
            &lookup,
        );

        assert_eq!(result.status, CurationStatus::PendingDependencies);
        assert!(
            result.note.contains("no taxon"),
            "the row must say what is missing: {}",
            result.note
        );
        // And the taxon itself is offered as a dependency, because an occurrence
        // cannot be asserted against an item that does not exist.
        let dependencies = result.dependency_blocks.join("\n");
        assert!(dependencies.contains("P225"), "{dependencies}");
        assert!(dependencies.contains("Gentiana lutea"), "{dependencies}");
        // The occurrence is not offered: it would fail.
        assert!(
            !result.quickstatements.iter().any(|s| s.contains("P703")),
            "an occurrence against a missing taxon would fail: {:?}",
            result.quickstatements
        );
    }

    #[test]
    fn a_genus_is_reported_rather_than_resolved() {
        let lookup = WikidataLookup {
            compound: Some(WikidataCompound {
                qid: "Q1".into(),
                ..WikidataCompound::default()
            }),
            taxon_qid: None,
            reference_qid: None,
            has_occurrence: None,
        };
        let result = to_result_row(&row("X", "CCO", Some("Gentiana"), None), &known(), &lookup);
        assert!(
            result.note.contains("genus"),
            "an ambiguous taxon must be called out: {}",
            result.note
        );
        // Creating an item called "Gentiana" from an ambiguous name would be
        // creating a real thing out of a guess.
        assert!(
            result.dependency_blocks.is_empty(),
            "{:?}",
            result.dependency_blocks
        );
    }

    #[test]
    fn a_compound_that_is_not_there_is_new_and_gets_a_create() {
        // The one case where a `CREATE` is right: no item, and a structure that
        // identifies what the item would be.
        let result = to_result_row(
            &row("Ethanol", "CCO", None, None),
            &known(),
            &WikidataLookup::default(),
        );
        assert_eq!(result.status, CurationStatus::NewCompound);

        let statements = result.quickstatements.join("\n");
        assert!(statements.contains("CREATE"), "{statements}");
        assert!(
            statements.contains("P31"),
            "an item with no class is unfindable: {statements}"
        );
        assert!(
            statements.contains("LFQSCWFLJHTTHZ-UHFFFAOYSA-N"),
            "{statements}"
        );
        assert!(statements.contains(r#""Ethanol""#), "{statements}");
    }

    #[test]
    fn a_new_compound_whose_structure_is_unknown_gets_no_create() {
        // Without a key, a `CREATE` would make a second item for a molecule that
        // may already have one. The row says so instead of guessing.
        let result = to_result_row(
            &row("Unknown", "CCO", None, None),
            &crate::ConvertedStructure::default(),
            &WikidataLookup::default(),
        );
        assert_eq!(result.status, CurationStatus::Error);
        assert!(
            !result.quickstatements.join("\n").contains("CREATE"),
            "nothing identifiable, so nothing to create: {:?}",
            result.quickstatements
        );
    }

    #[test]
    fn a_row_without_a_doi_is_not_told_it_is_missing_one() {
        // The note is conditional on a DOI being present. Mutation testing
        // found the guard could be dropped, which made every row that simply
        // has no DOI claim that Wikidata is missing a reference for it.
        let lookup = compound_qid("Q153");

        for doi in [None, Some(""), Some("   ")] {
            let result = to_result_row(&row("ethanol", "CCO", None, doi), &known(), &lookup);
            assert!(
                !result.note.contains("no reference with that DOI"),
                "doi={doi:?} should not produce a missing-reference note: {}",
                result.note
            );
        }

        // With a DOI present and unresolved, the note is correct.
        let result = to_result_row(
            &row("ethanol", "CCO", None, Some("10.1/x")),
            &known(),
            &lookup,
        );
        assert!(
            result.note.contains("no reference with that DOI"),
            "a real, unresolved DOI should be reported: {}",
            result.note
        );
    }

    #[test]
    fn creation_statements_carry_both_smiles_and_the_name() {
        // Canonical and isomeric are both written, and for different reasons:
        // canonical drops stereochemistry, isomeric keeps it, and Wikidata
        // holds both. Emitting an empty string here would silently produce a
        // compound with no structure at all.
        let stmts = creation_statements(
            &row("Quercetin", "c1ccccc1", None, None),
            "C1=CC=CC=C1",
            "C1=CC=CC=C1",
        );
        assert!(
            !stmts.is_empty(),
            "creation statements must not be empty for a row that needs creating"
        );
        assert!(stmts.contains("Quercetin"), "the label is missing: {stmts}");
        assert!(
            stmts.contains("C1=CC=CC=C1"),
            "the structure is missing: {stmts}"
        );
        assert!(
            stmts.contains("P233") && stmts.contains("P2017"),
            "both the canonical (P233) and isomeric (P2017) structure properties \
             are required: {stmts}"
        );
        assert!(
            stmts.contains("P31"),
            "the instance-of property is missing: {stmts}"
        );
    }

    #[test]
    fn creation_statements_keep_both_stereochemistry_variants_distinct() {
        // The same compound, specified and unspecified. They must not collapse
        // to one statement, or the stereochemistry is lost on submission.
        let racemic = creation_statements(&row("lactic", "CC(O)C", None, None), "CC(O)C", "CC(O)C");
        let one_isomer = creation_statements(
            &row("lactic", "C[C@@H](O)C", None, None),
            "CC(O)C",
            "C[C@@H](O)C",
        );
        assert_ne!(
            racemic, one_isomer,
            "a specified and an unspecified structure must produce different statements"
        );
        assert!(
            one_isomer.contains("C[C@@H](O)C"),
            "the isomeric structure is missing: {one_isomer}"
        );
    }

    #[test]
    fn the_shipped_example_still_parses() {
        // The two front-ends read the same file; this is the one in the docs.
        let tsv = "name\tsmiles\ttaxon\tdoi\nQuercetin\tCCO\tGentiana lutea\t10.1/a\n";
        let rows = parse_tsv(tsv).expect("the example parses");
        assert_eq!(rows.len(), 1);
    }
}

mod networked {
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::super::*;
    use super::fixtures::*;
    use lotus_search::testing::Scripted;

    fn key() -> crate::ConvertedStructure {
        crate::ConvertedStructure {
            inchikey: Some("LFQSCWFLJHTTHZ-UHFFFAOYSA-N".into()),
            canonical_smiles: Some("CCO".into()),
            ..crate::ConvertedStructure::default()
        }
    }

    fn row() -> CurationInputRow {
        CurationInputRow {
            name: "Quercetin".into(),
            smiles: "CCO".into(),
            taxon: Some("Gentiana lutea".into()),
            doi: Some("10.1/a".into()),
        }
    }

    #[tokio::test]
    async fn a_compound_that_is_already_there_is_reported_as_complete() {
        // Four requests, in the order the row's dependencies are resolved:
        // compound, taxon, reference, then whether the occurrence is there.
        let http = Scripted::new(vec![
            (200, COMPOUND_FOUND),
            (200, TAXON_FOUND),
            (200, REFERENCE_FOUND),
            (200, ASK_TRUE),
        ]);

        let lookup = look_up(&http, &row(), &key())
            .await
            .expect("the lookups succeed");

        assert_eq!(
            lookup.compound.as_ref().map(|c| c.qid.as_str()),
            Some("Q16521")
        );
        assert_eq!(lookup.taxon_qid.as_deref(), Some("Q16521"));
        assert_eq!(lookup.reference_qid.as_deref(), Some("Q100"));
        assert_eq!(lookup.has_occurrence, Some(true));

        let result = to_result_row(&row(), &key(), &lookup);
        assert_eq!(result.status, CurationStatus::ExistingComplete);
        assert!(
            result.quickstatements.is_empty(),
            "{:?}",
            result.quickstatements
        );
    }

    #[tokio::test]
    async fn a_missing_occurrence_is_the_only_thing_written() {
        let http = Scripted::new(vec![
            (200, COMPOUND_FOUND),
            (200, TAXON_FOUND),
            (200, REFERENCE_FOUND),
            (200, ASK_FALSE),
        ]);

        let lookup = look_up(&http, &row(), &key())
            .await
            .expect("the lookups succeed");
        let result = to_result_row(&row(), &key(), &lookup);

        assert_eq!(result.status, CurationStatus::ExistingNeedsUpdates);
        assert_eq!(result.quickstatements.len(), 1, "one statement, not four");
        assert!(
            result.quickstatements[0].contains("LAST|P703|wd:Q16521"),
            "{:?}",
            result.quickstatements
        );
    }

    #[tokio::test]
    async fn a_compound_that_is_not_there_is_never_looked_up_for_an_occurrence() {
        // Nothing is there to hang an occurrence on, so the fourth request
        // never happens. Asserting the call count is what catches a query that
        // would run `ASK` against an unbound QID and get an error instead.
        let http = Scripted::new(vec![
            (200, NOTHING_FOUND),
            (200, TAXON_FOUND),
            (200, REFERENCE_FOUND),
        ]);

        let lookup = look_up(&http, &row(), &key())
            .await
            .expect("the lookups succeed");

        assert!(lookup.compound.is_none());
        assert_eq!(lookup.has_occurrence, None, "not asked, not answered");
        assert_eq!(
            http.call_count(),
            3,
            "no occurrence query for a missing compound"
        );
        assert_eq!(
            to_result_row(&row(), &key(), &lookup).status,
            CurationStatus::NewCompound
        );
    }

    #[tokio::test]
    async fn the_compound_is_matched_on_the_inchikey_and_nothing_else() {
        let http = Scripted::new(vec![(200, NOTHING_FOUND)]);
        let _ = look_up(&http, &row(), &key()).await;

        let asked = http.queries();
        let compound_query = &asked[0];
        assert!(compound_query.contains("wdt:P235"), "{compound_query}");
        assert!(
            compound_query.contains("LFQSCWFLJHTTHZ-UHFFFAOYSA-N"),
            "{compound_query}"
        );
        // The SMILES is a distractor: matching on it would call a compound new
        // whenever the row happened to write it in a different valid order.
        assert!(
            !compound_query.contains(r#""CCO""#),
            "must not match on SMILES: {compound_query}"
        );
    }

    #[tokio::test]
    async fn a_genus_is_not_looked_up_at_all() {
        // "Gentiana" matches hundreds of species, so no request is made and the
        // row is reported instead of guessed.
        let mut ambiguous = row();
        ambiguous.taxon = Some("Gentiana".into());
        let http = Scripted::new(vec![(200, COMPOUND_FOUND), (200, REFERENCE_FOUND)]);

        let lookup = look_up(&http, &ambiguous, &key())
            .await
            .expect("the lookups succeed");

        assert_eq!(lookup.taxon_qid, None);
        assert_eq!(http.call_count(), 2, "no taxon query for a genus");
        assert!(
            to_result_row(&ambiguous, &key(), &lookup)
                .note
                .contains("genus"),
        );
    }

    #[tokio::test]
    async fn a_row_with_no_inchikey_is_never_looked_up_as_a_compound() {
        // The honest answer is "cannot tell", and no request can improve on it:
        // nothing about a structure can be identified without a key for it. The
        // taxon and the DOI are independent of the structure, so they are still
        // resolved -- that is information a curator can act on either way.
        let http = Scripted::new(vec![(200, TAXON_FOUND), (200, REFERENCE_FOUND)]);

        let lookup = look_up(&http, &row(), &crate::ConvertedStructure::default())
            .await
            .expect("a row with no key is not a transport error");

        assert!(lookup.compound.is_none());
        let asked = http.queries();
        assert_eq!(asked.len(), 2, "the taxon and the reference only");
        assert!(
            asked.iter().all(|call| !call.contains("P235")),
            "no compound can be asked about without a key: {asked:?}"
        );
        assert_eq!(http.call_count(), 2);

        let result = to_result_row(&row(), &crate::ConvertedStructure::default(), &lookup);
        assert_eq!(result.status, CurationStatus::Error);
        assert!(result.note.contains("no InChIKey"), "{}", result.note);
    }

    #[tokio::test]
    async fn a_transport_failure_is_an_error_rather_than_a_new_compound() {
        // The dangerous failure mode: a network blip reads as "not in Wikidata",
        // and the run writes a duplicate for a compound that has been there for
        // years. Status 0 is a connection that never arrived.
        let http = Scripted::new(vec![(0, "")]);

        let err = look_up(&http, &row(), &key())
            .await
            .expect_err("a dead endpoint is not a lookup result");

        assert!(matches!(err, CurationError::Http(_)), "{err:?}");
        assert!(
            err.is_recoverable(),
            "a retry is exactly the right answer: {err:?}"
        );
    }

    #[tokio::test]
    async fn a_malformed_answer_is_a_parse_error_and_is_not_retried() {
        // The bytes arrived, so another request would be refused the same way.
        let http = Scripted::new(vec![(200, "not json at all")]);

        let err = look_up(&http, &row(), &key())
            .await
            .expect_err("a body that is not SPARQL JSON is not a result");

        assert!(matches!(err, CurationError::Parse(_)), "{err:?}");
        assert!(!err.is_recoverable(), "{err:?}");
    }

    #[tokio::test]
    async fn an_ask_that_is_not_a_boolean_is_a_parse_error() {
        // An `ASK` that answers with a result set is a different query, and
        // reading it as "false" would silently skip a real occurrence.
        let http = Scripted::new(vec![
            (200, COMPOUND_FOUND),
            (200, TAXON_FOUND),
            (200, REFERENCE_FOUND),
            (200, COMPOUND_FOUND),
        ]);

        let err = look_up(&http, &row(), &key())
            .await
            .expect_err("a SELECT is not an answer to an ASK");

        assert!(matches!(err, CurationError::Parse(_)), "{err:?}");
    }

    #[test]
    fn a_binding_that_is_not_a_uri_is_not_read_as_a_qid() {
        // A label where a QID was expected. Reading the label as an item name
        // would write a statement about a nonexistent item.
        let json: serde_json::Value =
            serde_json::from_str(COMPOUND_LABEL_INSTEAD_OF_QID).expect("the fixture parses");
        let bindings = first_bindings(&json).expect("there is a result");
        assert_eq!(binding_qid(bindings, "compound"), None);
    }

    #[tokio::test]
    async fn an_ask_is_read_in_both_shapes_an_endpoint_answers_it_in() {
        // The wrapped form is the SPARQL JSON results format and is what QLever
        // sends; the bare literal is what a mirror sends. Reading only one makes
        // every occurrence check against the other fail to parse, and an
        // unreadable answer is not a "no".
        for (reply, expected) in [
            (ASK_TRUE, true),
            (ASK_TRUE_BARE, true),
            (ASK_FALSE, false),
            (ASK_FALSE_BARE, false),
        ] {
            let http = Scripted::new(vec![(200, reply)]);
            assert_eq!(
                ask(&http, "ASK {}").await.expect("a boolean is a boolean"),
                expected,
                "reply: {reply}"
            );
        }
    }

    #[test]
    fn a_result_with_no_bindings_is_absent_rather_than_empty() {
        // `{"results": {"bindings": []}}` is how an endpoint says "no match",
        // and it must not be read as a match with blank fields.
        let json: serde_json::Value =
            serde_json::from_str(NOTHING_FOUND).expect("the fixture parses");
        assert!(first_bindings(&json).is_none());
    }

    #[test]
    fn the_result_shape_is_read_from_results_bindings() {
        // `results` is an object holding a `bindings` array. Reading it as an
        // array finds nothing, and "nothing found" is the one answer that must
        // never come from a parsing slip.
        let json: serde_json::Value =
            serde_json::from_str(COMPOUND_FOUND).expect("the fixture parses");
        assert!(
            json.get("results")
                .is_some_and(serde_json::Value::is_object)
        );
        assert!(json.get("results").and_then(|r| r.as_array()).is_none());
        assert!(first_bindings(&json).is_some());
    }
}
