// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! JSON-LD for LOTUS results, following the Bioschemas profiles.
//!
//! The point of emitting it is findability: Google Dataset Search and the
//! Bioschemas validator both read the document, and neither can read a page
//! that carries no markup. So the shapes here are checked against the profiles
//! rather than written from memory — see [`profiles`].

#![warn(missing_docs)]

mod compound;
mod dataset;
mod profile;
mod software;
mod taxon;

pub use compound::compound_jsonld;
pub use dataset::{dataset_jsonld, result_set_jsonld};
pub use profile::{Profile, ValidationIssue, check};
pub use software::{citation_cff, codemeta, software_jsonld};
pub use taxon::taxon_jsonld;

use lotus_core::CompoundEntry;

/// The `@context` every document here shares.
pub const CONTEXT: &str = "https://schema.org/";

/// The Wikidata entity URI for a QID.
#[must_use]
pub fn wikidata_uri(qid: &str) -> String {
    format!("http://www.wikidata.org/entity/{qid}")
}

/// The canonical URL for a LOTUS concept, as a `sameAs` target.
#[must_use]
pub fn pubchem_uri(inchikey: &str) -> String {
    format!("https://pubchem.ncbi.nlm.nih.gov/compound/{inchikey}")
}

/// The `doi.org` URL for a bare DOI.
#[must_use]
pub fn doi_uri(doi: &str) -> String {
    format!("https://doi.org/{doi}")
}

/// A `PropertyValue` pairing a value with the scheme that identifies it.
///
/// Wikidata's `P235` and friends have no schema.org equivalent, so the
/// identifier goes in `identifier` with its own vocabulary, which is what the
/// Bioschemas `MolecularEntity` profile asks for.
#[must_use]
pub fn property_value(
    property: &str,
    value: &str,
    value_type: &str,
    scheme: &str,
) -> serde_json::Value {
    serde_json::json!({
        "@type": "PropertyValue",
        "propertyID": property,
        "value": value,
        "valueType": value_type,
        "identifier": scheme,
    })
}

/// The `Dataset` every result belongs to, given a result set's identity.
#[must_use]
pub fn result_dataset_url(name_slug: &str, query_hash: &str) -> String {
    format!("https://lotus.naturalproducts.net/dataset/{name_slug}/{query_hash}")
}

/// The project itself, for a page that describes the software.
pub const SOFTWARE: crate::software::Software = crate::software::Software {
    name: "LOTUS Explorer",
    description: "A linked open data explorer for the LOTUS compound-taxon-reference \
                  knowledge graph in Wikidata, queried over SPARQL.",
    url: "https://lotus.naturalproducts.net",
    repository: "https://github.com/lotusnprod/lotus-explore-rs",
    doi: Some("10.7554/eLife.70780"),
    version: env!("CARGO_PKG_VERSION"),
    license: "https://www.gnu.org/licenses/agpl-3.0.html",
    keywords: &[
        "natural products",
        "linked open data",
        "SPARQL",
        "bioinformatics",
        "chemical structure",
    ],
};

/// The `MolecularEntity` for a result row, or `None` if the row has no QID.
///
/// # Errors
/// Never, today. It returns `Result` so that a future profile check can fail
/// without changing every call site's type.
pub fn entity_for(entry: &CompoundEntry) -> Result<serde_json::Value, serde_json::Error> {
    Ok(compound_jsonld(entry))
}
