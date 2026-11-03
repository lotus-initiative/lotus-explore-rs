// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::*;

#[test]
fn every_edam_constant_is_a_canonical_four_digit_edam_iri() {
    for (branch, terms) in [
        (
            "topic_",
            &[
                TOPIC_SMALL_MOLECULES,
                TOPIC_CHEMINFORMATICS,
                TOPIC_METABOLOMICS,
                TOPIC_TAXONOMY,
                TOPIC_ONTOLOGY_AND_TERMINOLOGY,
            ][..],
        ),
        (
            "operation_",
            &[
                OPERATION_DATABASE_SEARCH,
                OPERATION_STRUCTURE_DATABASE_SEARCH,
                OPERATION_TAXONOMIC_CLASSIFICATION,
                OPERATION_DATA_RETRIEVAL,
            ][..],
        ),
        (
            "data_",
            &[
                DATA_SMALL_MOLECULE_STRUCTURE,
                DATA_COMPOUND_ID,
                DATA_TAXONOMIC_CLASSIFICATION,
                DATA_CITATION,
            ][..],
        ),
        (
            "format_",
            &[
                FORMAT_SMILES,
                FORMAT_INCHI,
                FORMAT_INCHIKEY,
                FORMAT_MOLFILE,
                FORMAT_CSV,
                FORMAT_TSV,
                FORMAT_JSON,
                FORMAT_JSON_LD,
                FORMAT_TURTLE,
                FORMAT_SPARQL,
            ][..],
        ),
    ] {
        for &iri in terms {
            let local = iri
                .strip_prefix(NAMESPACE)
                .and_then(|rest| rest.strip_prefix(branch));
            assert!(
                local.is_some_and(
                    |digits| digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit())
                ),
                "{iri} must be {NAMESPACE}{branch}<4 digits>"
            );
        }
    }
}
