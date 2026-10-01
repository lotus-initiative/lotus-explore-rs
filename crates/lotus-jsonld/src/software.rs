// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The project itself, as a Bioschemas `SoftwareApplication`, plus the two
//! citation files that say the same thing in the formats registries read.

use crate::{Profile, doi_uri, property_value};
use serde_json::{Value, json};

/// What the software is called, and how to cite it.
///
/// The source for all three citation formats, so a version bump is one edit.
#[derive(Debug, Clone, Copy)]
pub struct Software {
    /// The project's name.
    pub name: &'static str,
    /// One sentence on what it is for.
    pub description: &'static str,
    /// Where the software is used.
    pub url: &'static str,
    /// Where the source is.
    pub repository: &'static str,
    /// Where defects are reported.
    pub issue_tracker: &'static str,
    /// The concept DOI, when the project has minted one.
    pub doi: Option<&'static str>,
    /// The released version.
    pub version: &'static str,
    /// The SPDX licence identifier, for the vocabularies that want one.
    ///
    /// Separate from `license_url` because the two answer different questions:
    /// one is the canonical machine-readable name, the other is where a reader
    /// reads the terms.
    pub license: &'static str,
    /// Where a reader reads the licence.
    pub license_url: &'static str,
    /// The language the project is written in.
    pub language: &'static str,
    /// The canonical project page for that language, for a `ComputerLanguage`
    /// node to point at.
    pub language_url: &'static str,
    /// The minimum toolchain this build needs.
    pub requires_rust: &'static str,
    /// When the project was first committed, for the `datePublished` a registry
    /// shows.
    ///
    /// The repository's own first commit, so it is checkable. There is no release
    /// tag yet, so this is when the work began rather than when a version shipped;
    /// when the first release is tagged, this becomes that tag's date.
    pub date_published: &'static str,
    /// What the project is, in schema.org's controlled vocabulary.
    pub application_category: &'static str,
    /// The narrower label under it.
    pub application_subcategory: &'static str,
    /// What the software can do, as a list a reader can check off.
    pub features: &'static [&'static str],
    /// Subject terms, for discovery.
    pub keywords: &'static [&'static str],
    /// The LOTUS paper this tool exists to serve, with its title.
    pub paper: Option<Paper>,
}

/// The LOTUS paper, cited alongside the software.
#[derive(Debug, Clone, Copy)]
pub struct Paper {
    /// The paper's title.
    pub title: &'static str,
    /// The paper's DOI, without a resolver.
    pub doi: &'static str,
    /// The year it was published.
    pub year: u16,
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
    object.insert(
        "programmingLanguage".into(),
        json!({
            "@type": "ComputerLanguage",
            "name": software.language,
            "url": software.language_url,
        }),
    );
    object.insert("featureList".into(), json!(software.features));
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

    if let Some(paper) = software.paper {
        object.insert("citation".into(), lotus_paper(&paper));
    }

    crate::stamp(Value::Object(object), Profile::SoftwareApplication)
}

/// The LOTUS paper, as a `ScholarlyArticle` citation.
fn lotus_paper(paper: &Paper) -> Value {
    json!({
        "@type": "ScholarlyArticle",
        "name": paper.title,
        "identifier": doi_uri(paper.doi),
        "url": doi_uri(paper.doi),
        "datePublished": paper.year.to_string(),
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
    object.insert("issueTracker".into(), json!(software.issue_tracker));
    object.insert("url".into(), json!(software.url));
    object.insert("license".into(), json!(software.license_url));
    object.insert("version".into(), json!(software.version));
    // A `ComputerLanguage` node rather than a bare string: a registry that
    // renders the language links it, and the plain form has nothing to link to.
    object.insert(
        "programmingLanguage".into(),
        json!({
            "@type": "ComputerLanguage",
            "name": software.language,
            "url": software.language_url,
        }),
    );
    object.insert("keywords".into(), json!(software.keywords));
    object.insert("operatingSystem".into(), json!("any"));
    object.insert(
        "applicationCategory".into(),
        json!(software.application_category),
    );
    object.insert(
        "applicationSubCategory".into(),
        json!(software.application_subcategory),
    );
    object.insert(
        "buildInstructions".into(),
        json!("cargo build --release -p lotus-cli"),
    );
    // The toolchain is a real requirement -- the workspace pins 1.97 and uses
    // edition 2024 -- so it is stated rather than left to be discovered by a
    // failed build.
    object.insert(
        "softwareRequirements".into(),
        json!([{
            "@type": "SoftwareApplication",
            "name": "Rust",
            "version": software.requires_rust,
            "url": software.language_url,
        }]),
    );
    object.insert("featureList".into(), json!(software.features));
    object.insert(
        "readme".into(),
        json!(format!("{}/blob/main/README.md", software.repository)),
    );
    object.insert(
        "contIntegration".into(),
        json!([format!("{}/actions", software.repository)]),
    );
    object.insert(
        "developmentStatus".into(),
        json!("https://www.w3.org/TR/sw-life-cycle/#active-development"),
    );
    object.insert("datePublished".into(), json!(software.date_published));
    // The pages that describe the software, so a registry can show a gallery
    // rather than one bare URL.
    object.insert(
        "relatedLink".into(),
        json!([
            software.url,
            software.issue_tracker,
            format!("{}/blob/main/CONTRIBUTING.md", software.repository),
        ]),
    );
    if let Some(doi) = software.doi {
        object.insert("identifier".into(), json!(doi_uri(doi)));
    }
    // The paper is a separate work from the tool, so it goes in `citation` with
    // its own type rather than being folded into `identifier`.
    if let Some(paper) = software.paper {
        object.insert(
            "citation".into(),
            json!([{
                "@type": "ScholarlyArticle",
                "name": paper.title,
                "identifier": doi_uri(paper.doi),
                "url": doi_uri(paper.doi),
                "datePublished": paper.year.to_string(),
            }]),
        );
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
        "cff-version: 1.2.0".to_string(),
        format!(
            "message: {}",
            yaml_scalar("If you use this software, please cite it as below.")
        ),
        "type: software".to_string(),
        format!("title: {}", yaml_scalar(software.name)),
        format!("abstract: {}", yaml_scalar(software.description)),
        format!("version: {}", yaml_scalar(software.version)),
        // The SPDX identifier, not the URL: `cffconvert --validate` and GitHub's
        // widget both read this as a licence *name*. The URL goes alongside it as
        // an identifier, where a machine can dereference it.
        format!("license: {}", yaml_scalar(software.license)),
        format!("repository-code: {}", yaml_scalar(software.repository)),
        format!("url: {}", yaml_scalar(software.url)),
    ];

    if let Some(doi) = software.doi {
        // The bare DOI, not the resolver URL. CFF's `doi` field is defined as
        // the identifier without a prefix, and a resolver URL in it is either
        // rejected by `cffconvert --validate` or silently turned into
        // `https://doi.org/https://doi.org/...` by a consumer that prepends one.
        fields.push(format!("doi: {}", yaml_scalar(doi)));
    }

    // Both dereferenceable forms of the project, so a consumer that wants a URL
    // has one whether it reads `identifiers` or resolves `doi` itself.
    let mut identifiers = format!(
        "identifiers:\n  - type: url\n    value: {}\n",
        yaml_scalar(software.url)
    );
    if let Some(doi) = software.doi {
        use std::fmt::Write as _;
        let _ = writeln!(
            identifiers,
            "  - type: doi\n    value: {}",
            yaml_scalar(&doi_uri(doi))
        );
    }
    fields.push(identifiers.trim_end().to_string());

    // One `keywords:` key with every value under it. Emitting a key per keyword
    // parses as valid YAML and silently keeps only the last one, so a document
    // listing five keywords described the software by one of them.
    if !software.keywords.is_empty() {
        // The key carries its own newline, so the first list item starts on its
        // own line rather than continuing the key's.
        let mut list = String::from("keywords:\n");
        for keyword in software.keywords {
            use std::fmt::Write as _;
            // Writing into the buffer rather than allocating a `String` per
            // keyword: this runs on every CI check.
            let _ = writeln!(list, "  - {}", yaml_scalar(keyword));
        }
        fields.push(list);
    }

    // The consortium is a Wikidata item, not a person, so it is named with a
    // `name` and an `alias` rather than invented given and family names. A
    // citation that attributes the software to a person who did not write it is
    // worse than one that attributes it to the group that did.
    fields.push(
        "authors:\n  - name: \"The LOTUS consortium\"\n    website: \"https://www.wikidata.org/wiki/Q104225190\""
            .to_string(),
    );

    // The paper this tool exists to serve. `references` rather than
    // `preferred-citation`: the citable work is the software, and the LOTUS
    // paper is what it queries, so asking a reader to cite that instead would
    // point them at the wrong artefact.
    if let Some(paper) = software.paper {
        use std::fmt::Write as _;
        let mut reference = format!(
            "references:\n  - type: article\n    title: {}\n    journal: eLife\n    year: '{}'\n    doi: {}\n    url: {}\n    authors:\n      - name: \"{}\"",
            yaml_scalar(paper.title),
            paper.year,
            yaml_scalar(paper.doi),
            yaml_scalar(&doi_uri(paper.doi)),
            "The LOTUS consortium",
        );
        let _ = writeln!(reference);
        fields.push(reference.trim_end().to_string());
    }

    format!("{}\n", fields.join("\n"))
}

/// Quote a scalar only when it needs it, so an ordinary word stays readable.
fn yaml_scalar(value: &str) -> String {
    let needs_quotes = value.contains(':')
        || value.contains('#')
        || value.contains('\n')
        || value.starts_with(' ')
        || value.ends_with(' ')
        // A leading quote is not just a quote: in a plain scalar it opens one, and
        // the value ends up as a quoted scalar with no terminator. Anywhere else a
        // `"` is an ordinary character and the value stays readable.
        || value.starts_with('"')
        || value.starts_with('\'')
        || value.is_empty();
    if needs_quotes {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
#[path = "software/tests.rs"]
mod tests;
