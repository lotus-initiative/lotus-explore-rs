// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

/// What `Wikidata` holds for a compound, as far as curation cares.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct WikidataCompound {
    /// The compound's `Wikidata` item.
    pub qid: String,
    /// The `SMILES` Wikidata holds, which may differ from what was submitted.
    pub canonical_smiles: Option<String>,
    /// The stereochemistry-preserving `SMILES`.
    pub isomeric_smiles: Option<String>,
    /// The `InChI`, if Wikidata has one.
    pub inchi: Option<String>,
    /// The molecular formula as `Wikidata` records it.
    pub formula: Option<String>,
    /// The exact mass `Wikidata` records.
    pub mass: Option<f64>,
}

/// The taxon and reference a row depends on, and any that are still missing.
#[derive(Debug, Default)]
pub struct DependencyResolution {
    /// The organism, if it was found.
    pub taxon_qid: Option<String>,
    /// The reference, if it was found.
    pub reference_qid: Option<String>,
    /// Statements that must be submitted before the row's own.
    pub dependency_blocks: Vec<String>,
    /// What is still missing, in words a curator can act on.
    pub pending_messages: Vec<String>,
}

/// A canonicalised exact mass, and a note about why it might not be trustworthy.
#[derive(Debug, Default)]
pub struct MassResolution {
    /// The canonicalised exact mass.
    pub exact_mass: Option<f64>,
    /// Why the mass might not be trustworthy.
    pub warning: Option<String>,
}
