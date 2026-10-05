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
    /// Taxa.
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
            Self::Taxon => &["name"],
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
            Self::Taxon => &["sameAs", "taxonRank", "url"],
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

    /// Every property name this profile knows about: required, recommended, and
    /// whatever the type itself brings.
    ///
    /// Used by a test that asserts the emitters stay inside the profile. The
    /// failure it exists to prevent is quiet: an extra property does not break
    /// validation, it just asserts something nobody checked.
    #[must_use]
    pub fn declared_properties(self) -> Vec<&'static str> {
        let mut all: Vec<&'static str> = self
            .required()
            .iter()
            .chain(self.recommended().iter())
            .copied()
            .collect();
        all.sort_unstable();
        all.dedup();
        all
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
/// `@context` and `dct:conformsTo` are checked rather than assumed. Assuming them
/// because this crate's builders always set them is true and useless: the
/// interesting case is a document from somewhere else, and one that does not
/// declare its profile is the one most worth complaining about. Bioschemas
/// requires the property for that reason.
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

    // `property_names` deliberately skips `@`-prefixed keys, because they are
    // JSON-LD syntax rather than schema.org properties. These two are the
    // exception: they are the document's own metadata and they are checked here.
    if object.get("@context").is_none() {
        issues.push(ValidationIssue {
            property: "@context".into(),
            severity: "Violation",
            message: "JSON-LD needs a @context to mean anything".into(),
        });
    }

    match object.get("dct:conformsTo") {
        None => issues.push(ValidationIssue {
            property: "dct:conformsTo".into(),
            severity: "Violation",
            message: format!(
                "a document claiming {} must say so with dct:conformsTo",
                profile.id()
            ),
        }),
        Some(declared) if declared.as_str() == Some(profile.id()) => {}
        Some(declared) => issues.push(ValidationIssue {
            property: "dct:conformsTo".into(),
            severity: "Violation",
            message: format!(
                "claims profile {declared} but was checked against {}",
                profile.id()
            ),
        }),
    }

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
#[path = "profile/tests.rs"]
mod tests;
