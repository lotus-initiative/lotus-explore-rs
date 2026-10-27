// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! What an export looks like on disk: its name, its filters, and the metadata
//! document that describes it.

/// The Wikidata archive: fetch, deduplicate, package.
pub mod archive;
/// The name an export is written under.
pub mod filename;
/// The search's criteria as the metadata document's filter block.
pub mod filters;
/// The Schema.org document that describes an export.
pub mod metadata;
/// The LOTUS exporter pipeline: fetch, join, write.
pub mod pipeline;
/// The PubChem tree generator: biological / chemical / NPClassifier trees.
pub mod pubchem_tree;
