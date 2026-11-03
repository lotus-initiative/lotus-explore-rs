// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! [EDAM ontology](https://edamontology.org/) terms used across the LOTUS
//! Bioschemas and `CodeMeta` descriptions.
//!
//! Each constant is the canonical persistent HTTP IRI (`http://edamontology.org/<branch>_<id>`)
//! expected by Bioschemas `ComputationalTool`, `FormalParameter`, `Dataset`, and
//! `MolecularEntity` consumers (such as bio.tools and ELIXIR registries).

/// The canonical EDAM namespace prefix.
pub const NAMESPACE: &str = "http://edamontology.org/";

/// EDAM `topic_0154` — Small molecules.
pub const TOPIC_SMALL_MOLECULES: &str = "http://edamontology.org/topic_0154";

/// EDAM `topic_2258` — Cheminformatics.
pub const TOPIC_CHEMINFORMATICS: &str = "http://edamontology.org/topic_2258";

/// EDAM `topic_3172` — Metabolomics.
pub const TOPIC_METABOLOMICS: &str = "http://edamontology.org/topic_3172";

/// EDAM `topic_0637` — Taxonomy.
pub const TOPIC_TAXONOMY: &str = "http://edamontology.org/topic_0637";

/// EDAM `topic_0089` — Ontology and terminology.
pub const TOPIC_ONTOLOGY_AND_TERMINOLOGY: &str = "http://edamontology.org/topic_0089";

/// EDAM `operation_2421` — Database search.
pub const OPERATION_DATABASE_SEARCH: &str = "http://edamontology.org/operation_2421";

/// EDAM `operation_0339` — Structure database search.
pub const OPERATION_STRUCTURE_DATABASE_SEARCH: &str = "http://edamontology.org/operation_0339";

/// EDAM `operation_3460` — Taxonomic classification.
pub const OPERATION_TAXONOMIC_CLASSIFICATION: &str = "http://edamontology.org/operation_3460";

/// EDAM `operation_2422` — Data retrieval.
pub const OPERATION_DATA_RETRIEVAL: &str = "http://edamontology.org/operation_2422";

/// EDAM `data_1463` — Small molecule structure.
pub const DATA_SMALL_MOLECULE_STRUCTURE: &str = "http://edamontology.org/data_1463";

/// EDAM `data_1086` — Compound identifier.
pub const DATA_COMPOUND_ID: &str = "http://edamontology.org/data_1086";

/// EDAM `data_1872` — Taxonomic classification.
pub const DATA_TAXONOMIC_CLASSIFICATION: &str = "http://edamontology.org/data_1872";

/// EDAM `data_0970` — Citation.
pub const DATA_CITATION: &str = "http://edamontology.org/data_0970";

/// EDAM `format_1196` — SMILES.
pub const FORMAT_SMILES: &str = "http://edamontology.org/format_1196";

/// EDAM `format_1197` — `InChI`.
pub const FORMAT_INCHI: &str = "http://edamontology.org/format_1197";

/// EDAM `format_1198` — `InChIKey`.
pub const FORMAT_INCHIKEY: &str = "http://edamontology.org/format_1198";

/// EDAM `format_3815` — Molfile.
pub const FORMAT_MOLFILE: &str = "http://edamontology.org/format_3815";

/// EDAM `format_3752` — CSV.
pub const FORMAT_CSV: &str = "http://edamontology.org/format_3752";

/// EDAM `format_3475` — TSV.
pub const FORMAT_TSV: &str = "http://edamontology.org/format_3475";

/// EDAM `format_3464` — JSON.
pub const FORMAT_JSON: &str = "http://edamontology.org/format_3464";

/// EDAM `format_3749` — JSON-LD.
pub const FORMAT_JSON_LD: &str = "http://edamontology.org/format_3749";

/// EDAM `format_3255` — Turtle.
pub const FORMAT_TURTLE: &str = "http://edamontology.org/format_3255";

/// EDAM `format_3790` — SPARQL.
pub const FORMAT_SPARQL: &str = "http://edamontology.org/format_3790";

#[cfg(test)]
#[path = "edam/tests.rs"]
mod tests;
