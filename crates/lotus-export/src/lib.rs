// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
// The export pipelines are generic over `lotus_search::Http`. On wasm32 the
// browser promise is `!Send`, so async functions awaiting it are `!Send` there.
#![cfg_attr(
    target_arch = "wasm32",
    allow(
        clippy::future_not_send,
        reason = "browser HTTP futures hold Rc and run on one thread"
    )
)]

//! Bulk export pipelines for the LOTUS Wikidata knowledge graph.

/// The Wikidata SMILES archive: fetch, deduplicate, package.
pub mod archive;
/// The LOTUS exporter pipeline: fetch, join, write.
pub mod pipeline;
/// The `PubChem` tree generator: biological / chemical / `NPClassifier` trees.
pub mod pubchem_tree;
