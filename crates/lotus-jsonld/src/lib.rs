// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod compound;
mod dataset;
mod profile;
mod software;
mod taxon;

pub use compound::compound_jsonld;
pub use dataset::{ResultSet, dataset_jsonld, result_set_jsonld};
pub use profile::{Profile, ValidationIssue, check};
pub use software::{Software, citation_cff, codemeta, software_jsonld};
pub use taxon::taxon_jsonld;

use lotus_model::CompoundEntry;

/// The `@context` every document here shares.
pub const CONTEXT: &str = "https://schema.org/";

/// Stamps `@context` and `dct:conformsTo` onto a node.
///
/// Both together: a document carrying a context but not saying which profile it
/// claims is not checkable, since a validator cannot know which required
/// properties apply. Bioschemas lists `dct:conformsTo` as mandatory for that
/// reason, and this crate once emitted the context without it.
///
/// The value is the profile's own `@id`, so a consumer resolves the exact profile
/// version instead of guessing from the document's shape.
pub(crate) fn stamp(mut node: serde_json::Value, profile: Profile) -> serde_json::Value {
    // A no-op for a document that is not an object, rather than a panic:
    // indexing a `Value` panics on a non-object, and this is called on
    // whatever a caller built.
    if let Some(object) = node.as_object_mut() {
        object.insert("@context".into(), CONTEXT.into());
        object.insert("dct:conformsTo".into(), profile.id().into());
    }
    node
}

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
    format!("https://lotus.nprod.net/lotus-explore-rs/dataset/{name_slug}/{query_hash}")
}

/// The project itself, for a page that describes the software.
pub const SOFTWARE: crate::software::Software = crate::software::Software {
    name: "LOTUS Explorer",
    description: "A linked open data explorer for the LOTUS compound-taxon-reference \
                  knowledge graph in Wikidata, queried over SPARQL.",
    url: "https://lotus.nprod.net/lotus-explore-rs",
    repository: env!("CARGO_PKG_REPOSITORY"),
    issue_tracker: concat!(env!("CARGO_PKG_REPOSITORY"), "/issues"),
    doi: Some("10.7554/eLife.70780"),
    version: env!("CARGO_PKG_VERSION"),
    license: "AGPL-3.0-only",
    license_url: "https://www.gnu.org/licenses/agpl-3.0.html",
    language: "Rust",
    language_url: "https://www.rust-lang.org",
    requires_rust: ">= 1.97",
    // The first commit in this repository; there is no release tag to date from yet.
    date_published: "2026-09-22",
    application_category: "scientific data analysis",
    application_subcategory: "SPARQL client",
    features: &[
        "SPARQL search over the LOTUS projection in Wikidata",
        "substructure and similarity structure search",
        "taxon, mass, year and molecular formula filters",
        "Bioschemas-compliant JSON-LD export",
        "QuickStatements curation for compounds Wikidata is missing",
    ],
    keywords: &[
        "natural products",
        "linked open data",
        "SPARQL",
        "bioinformatics",
        "chemical compound",
        "natural product",
        "taxonomy",
        "chemotaxonomy",
        "curation",
        "rust",
    ],
    paper: Some(crate::software::Paper {
        title: "The LOTUS initiative for open knowledge management in natural products research",
        doi: "10.7554/eLife.70780",
        year: 2022,
    }),
};

/// The `MolecularEntity` for a result row, or `None` if the row has no QID.
///
/// # Errors
/// Never, today. It returns `Result` so that a future profile check can fail
/// without changing every call site's type.
pub fn entity_for(entry: &CompoundEntry) -> Result<serde_json::Value, serde_json::Error> {
    Ok(compound_jsonld(entry))
}

#[cfg(test)]
mod uri_tests;

#[cfg(test)]
mod profile_bounds_tests;
