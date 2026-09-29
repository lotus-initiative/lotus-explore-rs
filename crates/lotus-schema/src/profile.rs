// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The minimum and recommended properties, per Bioschemas profile.
//!
//! Transcribed from the SHACL shapes published at
//! <https://bioschemas.org/profiles/bioschemas_profiles_shacl.jsonld>, which is
//! the normative source. `required` is what a conforming document must carry;
//! `recommended` is what it should carry, and is reported as a warning rather
//! than an error so that a partially-populated result set is still publishable.

use std::collections::BTreeSet;

/// A Bioschemas profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Compounds.
    MolecularEntity,
    /// Organisms.
    Taxon,
    /// A result set, and the LOTUS source as a whole.
    Dataset,
    /// The catalogue the result set belongs to.
    DataCatalog,
    /// The software, as a Bioschemas `ComputationalTool`.
    SoftwareApplication,
    /// A cited paper.
    ScholarlyArticle,
}

impl Profile {
    /// The profile's `@id`, which is what `dct:conformsTo` points at.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::MolecularEntity => "https://bioschemas.org/profiles/MolecularEntity/0.5-RELEASE",
            Self::Taxon => "https://bioschemas.org/profiles/Taxon/1.0-RELEASE",
            Self::Dataset => "https://bioschemas.org/profiles/Dataset/1.0-RELEASE",
            Self::DataCatalog => {
                "https://bioschemas.org/profiles/DataCatalog/0.3-RELEASE-2019_07_01"
            }
            Self::SoftwareApplication => {
                "https://bioschemas.org/profiles/ComputationalTool/1.0-RELEASE"
            }
            Self::ScholarlyArticle => "https://bioschemas.org/profiles/ScholarlyArticle/0.3-DRAFT",
        }
    }

    /// The `schema.org` type this profile constrains.
    #[must_use]
    pub const fn rdf_type(self) -> &'static str {
        match self {
            Self::MolecularEntity => "MolecularEntity",
            Self::Taxon => "Taxon",
            Self::Dataset => "Dataset",
            Self::DataCatalog => "DataCatalog",
            Self::SoftwareApplication => "SoftwareApplication",
            Self::ScholarlyArticle => "ScholarlyArticle",
        }
    }

    /// Properties the profile marks `Violation` at `minCount` 1.
    #[must_use]
    pub const fn required(self) -> &'static [&'static str] {
        match self {
            Self::MolecularEntity => &["identifier", "name", "url"],
            Self::Taxon => &["name", "taxonRank"],
            Self::Dataset => &[
                "description",
                "identifier",
                "keywords",
                "license",
                "name",
                "url",
            ],
            Self::DataCatalog => &["description", "keywords", "name", "provider", "url"],
            Self::SoftwareApplication => &["description", "name", "url"],
            Self::ScholarlyArticle => &["identifier", "name"],
        }
    }

    /// Properties the profile marks `Warning` at `minCount` 1.
    #[must_use]
    pub const fn recommended(self) -> &'static [&'static str] {
        match self {
            Self::MolecularEntity => &[
                "inChI",
                "inChIKey",
                "molecularFormula",
                "molecularWeight",
                "smiles",
            ],
            Self::Taxon => &["parentTaxon", "sameAs", "url"],
            Self::Dataset => &[
                "citation",
                "creator",
                "datePublished",
                "distribution",
                "includedInDataCatalog",
                "measurementTechnique",
                "variableMeasured",
                "version",
            ],
            Self::DataCatalog => &[
                "about",
                "citation",
                "dataset",
                "dateCreated",
                "identifier",
                "license",
                "sourceOrganization",
            ],
            Self::SoftwareApplication => &[
                "applicationCategory",
                "applicationSubCategory",
                "author",
                "citation",
                "featureList",
                "license",
                "softwareVersion",
            ],
            Self::ScholarlyArticle => &[
                "author",
                "datePublished",
                "identifier",
                "license",
                "sameAs",
                "url",
            ],
        }
    }

    /// Every profile, for iteration by a test or a validator command.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::MolecularEntity,
            Self::Taxon,
            Self::Dataset,
            Self::DataCatalog,
            Self::SoftwareApplication,
            Self::ScholarlyArticle,
        ]
    }
}

/// A property a document is missing, or has where the profile forbids it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    /// The property, or `@type` when the document's type is wrong.
    pub property: String,
    /// `Violation` for a required property, `Warning` for a recommended one.
    pub severity: &'static str,
    /// Which profile wanted it, so a reader can look it up.
    pub message: String,
}

/// Check a JSON-LD document against a profile.
///
/// The `@context` and `dct:conformsTo` are assumed: a caller that produced the
/// document with this crate has set both, and re-checking them here would make
/// every caller pass the same constant back in.
#[must_use]
pub fn check(document: &serde_json::Value, profile: Profile) -> Vec<ValidationIssue> {
    let Some(object) = document.as_object() else {
        return vec![ValidationIssue {
            property: "@type".into(),
            severity: "Violation",
            message: format!("expected a JSON object, got {}", kind_of(document)),
        }];
    };

    let mut issues = Vec::new();

    if object.get("@type").and_then(|t| t.as_str()) != Some(profile.rdf_type()) {
        issues.push(ValidationIssue {
            property: "@type".into(),
            severity: "Violation",
            message: format!(
                "expected @type {} for the {} profile",
                profile.rdf_type(),
                profile.id()
            ),
        });
    }

    let present = property_names(object);
    for required in profile.required() {
        if !present.contains(*required) {
            issues.push(ValidationIssue {
                property: (*required).to_string(),
                severity: "Violation",
                message: format!("{} requires {required}", profile.id()),
            });
        }
    }
    for recommended in profile.recommended() {
        if !present.contains(*recommended) {
            issues.push(ValidationIssue {
                property: (*recommended).to_string(),
                severity: "Warning",
                message: format!("{} recommends {recommended}", profile.id()),
            });
        }
    }

    issues
}

/// Every property the document carries, following the aliases in `@context`.
///
/// An empty value counts as absent: a `keywords: []` satisfies nothing.
fn property_names(object: &serde_json::Map<String, serde_json::Value>) -> BTreeSet<String> {
    object
        .iter()
        .filter(|(key, value)| {
            !key.starts_with('@')
                && !matches!(value, serde_json::Value::Null)
                && !value.as_array().is_some_and(Vec::is_empty)
                && !value.as_str().is_some_and(str::is_empty)
        })
        .map(|(key, _)| key.clone())
        .collect()
}

const fn kind_of(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use serde_json::json;

    fn conforming(profile: Profile) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert("@type".into(), json!(profile.rdf_type()));
        for property in profile.required().iter().chain(profile.recommended()) {
            object.insert((*property).into(), json!("x"));
        }
        serde_json::Value::Object(object)
    }

    #[test]
    fn a_conforming_document_has_no_issues() {
        for profile in Profile::all() {
            let issues = check(&conforming(*profile), *profile);
            assert!(issues.is_empty(), "{}: {issues:?}", profile.id());
        }
    }

    #[test]
    fn a_missing_required_property_is_a_violation() {
        let mut document = conforming(Profile::Dataset);
        document
            .as_object_mut()
            .expect("an object")
            .remove("license");

        let issues = check(&document, Profile::Dataset);
        let licence = issues
            .iter()
            .find(|i| i.property == "license")
            .expect("reported");
        assert_eq!(licence.severity, "Violation");
    }

    #[test]
    fn a_missing_recommended_property_is_only_a_warning() {
        let mut document = conforming(Profile::MolecularEntity);
        document
            .as_object_mut()
            .expect("an object")
            .remove("smiles");

        let issues = check(&document, Profile::MolecularEntity);
        let smiles = issues
            .iter()
            .find(|i| i.property == "smiles")
            .expect("reported");
        assert_eq!(smiles.severity, "Warning");
        assert!(
            !issues.iter().any(|i| i.severity == "Violation"),
            "and nothing else is broken"
        );
    }

    #[test]
    fn an_empty_value_counts_as_missing() {
        // A `keywords: []` or an empty `url` satisfies nothing, and a document
        // that carries them is worse than one that admits the gap.
        let document = json!({
            "@type": "Dataset",
            "name": "x",
            "url": "",
            "keywords": [],
            "description": "d",
            "identifier": "i",
            "license": "l",
        });

        let issues = check(&document, Profile::Dataset);
        assert!(issues.iter().any(|i| i.property == "url"), "url is empty");
        assert!(
            issues.iter().any(|i| i.property == "keywords"),
            "keywords is empty"
        );
    }

    #[test]
    fn the_wrong_type_is_reported_against_the_profile() {
        let document = json!({ "@type": "Thing", "name": "x" });
        let issues = check(&document, Profile::Dataset);
        assert!(issues.iter().any(|i| i.property == "@type"));
    }

    #[test]
    fn a_non_object_document_is_reported_rather_than_panicking() {
        for bad in [json!(null), json!("x"), json!([]), json!(3)] {
            let issues = check(&bad, Profile::Dataset);
            assert!(!issues.is_empty(), "{bad}");
        }
    }
}
