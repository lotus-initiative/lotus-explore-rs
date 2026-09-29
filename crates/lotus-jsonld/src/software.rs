// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The project itself, as a Bioschemas `SoftwareApplication`, plus the two
//! citation files that say the same thing in the formats registries read.

use crate::{Profile, doi_uri, property_value};
use serde_json::{Value, json};

/// What the software is called, and how to cite it.
#[derive(Debug, Clone, Copy)]
pub struct Software {
    pub name: &'static str,
    pub description: &'static str,
    pub url: &'static str,
    pub repository: &'static str,
    pub doi: Option<&'static str>,
    pub version: &'static str,
    pub license: &'static str,
    pub keywords: &'static [&'static str],
}

/// The project as a conforming `SoftwareApplication`.
#[must_use]
pub fn software_jsonld(software: &Software) -> Value {
    let mut object = serde_json::Map::new();
    object.insert("@type".into(), json!("SoftwareApplication"));
    object.insert("@id".into(), json!(software.url));
    object.insert("name".into(), json!(software.name));
    object.insert("description".into(), json!(software.description));
    object.insert("url".into(), json!(software.url));
    object.insert("license".into(), json!(software.license));
    object.insert("softwareVersion".into(), json!(software.version));
    object.insert(
        "applicationCategory".into(),
        json!("scientific data analysis"),
    );
    object.insert("applicationSubCategory".into(), json!("SPARQL client"));
    object.insert("codeRepository".into(), json!(software.repository));
    object.insert("programmingLanguage".into(), json!("Rust"));
    object.insert(
        "featureList".into(),
        json!([
            "SPARQL search over the LOTUS projection in Wikidata",
            "substructure and similarity structure search",
            "taxon, mass, year and molecular formula filters",
            "Bioschemas-compliant JSON-LD export",
        ]),
    );
    object.insert(
        "identifier".into(),
        json!([property_value(
            "repository",
            software.repository,
            "uri",
            "https://identifiers.org/url",
        )]),
    );
    object.insert("sameAs".into(), json!([software.repository]));

    if let Some(doi) = software.doi {
        object.insert("citation".into(), lotus_paper(doi));
    }

    crate::stamp(Value::Object(object), Profile::SoftwareApplication)
}

/// The LOTUS paper, as a `ScholarlyArticle` citation.
fn lotus_paper(doi: &str) -> Value {
    json!({
        "@type": "ScholarlyArticle",
        "name": "The LOTUS initiative for open knowledge management in natural products research",
        "identifier": doi_uri(doi),
        "url": doi_uri(doi),
    })
}

/// The `codemeta.json` a repository is indexed by: a document in the
/// `CodeMeta` vocabulary, version 2.0.
#[must_use]
pub fn codemeta(software: &Software) -> Value {
    let mut object = serde_json::Map::new();
    object.insert(
        "@context".into(),
        json!("https://doi.org/10.5063/schema/codemeta-2.0"),
    );
    object.insert("@type".into(), json!("SoftwareSourceCode"));
    object.insert("name".into(), json!(software.name));
    object.insert("description".into(), json!(software.description));
    object.insert("codeRepository".into(), json!(software.repository));
    object.insert("url".into(), json!(software.url));
    object.insert("license".into(), json!(software.license));
    object.insert("version".into(), json!(software.version));
    object.insert("programmingLanguage".into(), json!("Rust"));
    object.insert("keywords".into(), json!(software.keywords));
    object.insert("operatingSystem".into(), json!("any"));
    object.insert(
        "applicationCategory".into(),
        json!("scientific data analysis"),
    );
    object.insert(
        "buildInstructions".into(),
        json!("cargo build --release -p lotus-cli"),
    );
    if let Some(doi) = software.doi {
        object.insert("identifier".into(), json!(doi_uri(doi)));
    }
    Value::Object(object)
}

/// A CITATION.cff body, which is what `cffconvert` and GitHub's citation widget
/// read.
///
/// Hand-built rather than templated: a CFF document is a fixed, small shape, and
/// a template engine would be more code than the output.
#[must_use]
pub fn citation_cff(software: &Software) -> String {
    let mut fields = vec![
        format!("cff-version: 1.2.0"),
        "message: If you use this software, please cite it as below.".to_string(),
        format!("title: {}", yaml_scalar(software.name)),
        format!("abstract: {}", yaml_scalar(software.description)),
        format!("version: {}", yaml_scalar(software.version)),
        format!("repository-code: {}", yaml_scalar(software.repository)),
        format!("url: {}", yaml_scalar(software.url)),
        format!("license: {}", yaml_scalar(software.license)),
        "type: software".to_string(),
    ];

    if let Some(doi) = software.doi {
        fields.push(format!("doi: {}", yaml_scalar(&doi_uri(doi))));
    }

    for keyword in software.keywords {
        fields.push(format!("keywords:\n  - {}", yaml_scalar(keyword)));
    }

    let authors = "authors:\n  - name: \"The LOTUS consortium\"\n    website: \"https://www.wikidata.org/wiki/Q104225190\"";
    fields.push(authors.to_string());

    format!("{}\n", fields.join("\n"))
}

/// Quote a scalar only when it needs it, so an ordinary word stays readable.
fn yaml_scalar(value: &str) -> String {
    let needs_quotes = value.contains(':')
        || value.contains('#')
        || value.contains('\n')
        || value.starts_with(' ')
        || value.ends_with(' ')
        || value.is_empty();
    if needs_quotes {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use crate::Profile;
    use crate::profile::check;

    #[test]
    fn the_software_conforms_on_the_bioschemas_profile() {
        let issues = check(
            &software_jsonld(&crate::SOFTWARE),
            Profile::SoftwareApplication,
        );
        assert!(
            !issues.iter().any(|i| i.severity == "Violation"),
            "violations: {issues:?}"
        );
    }

    #[test]
    fn codemeta_names_the_repository_and_the_version() {
        let node = codemeta(&crate::SOFTWARE);
        assert_eq!(node["@type"], json!("SoftwareSourceCode"));
        assert_eq!(node["codeRepository"], json!(crate::SOFTWARE.repository));
        assert_eq!(node["version"], json!(crate::SOFTWARE.version));
    }

    #[test]
    fn the_citation_file_is_valid_cff() {
        let cff = citation_cff(&crate::SOFTWARE);
        assert!(cff.starts_with("cff-version: 1.2.0\n"), "got: {cff}");
        assert!(cff.contains("type: software"));
        assert!(cff.contains("authors:"));
        // A value with a colon has to be quoted or it parses as a mapping.
        assert!(cff.contains("doi: \"https://doi.org/10.7554/eLife.70780\""));
    }

    #[test]
    fn a_scalar_with_a_colon_is_quoted_and_one_without_is_not() {
        assert_eq!(yaml_scalar("LOTUS Explorer"), "LOTUS Explorer");
        assert_eq!(
            yaml_scalar("https://example.org"),
            "\"https://example.org\""
        );
        assert_eq!(yaml_scalar("a: b"), "\"a: b\"");
        assert_eq!(yaml_scalar(""), "\"\"");
    }

    #[test]
    fn the_citation_names_the_lotus_paper_when_there_is_one() {
        assert!(citation_cff(&crate::SOFTWARE).contains("10.7554/eLife.70780"));
    }
}
