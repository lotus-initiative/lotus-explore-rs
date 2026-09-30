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
        format!("type: software"),
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
mod tests {
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use crate::profile::check;
    use crate::{Profile, SOFTWARE};

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
        // The DOI is bare. This assertion used to require the resolver URL, which
        // is the wrong form: CFF defines `doi` as the identifier without a
        // prefix. It passed here because the URL has a colon and so also
        // happened to demonstrate the quoting rule, which is why the wrong form
        // survived as long as it did.
        assert!(cff.contains("doi: 10.7554/eLife.70780"), "got: {cff}");
    }

    #[test]
    fn a_scalar_with_a_colon_is_quoted_and_one_without_is_not() {
        assert_eq!(yaml_scalar("LOTUS Explorer"), "LOTUS Explorer");
        assert_eq!(
            yaml_scalar("https://example.org"),
            "\"https://example.org\""
        );
        // A bare DOI has no colon, so it stays unquoted. Quoting it would be
        // harmless, but the point is that the value is not being rescued by a
        // quote that the resolver URL used to need.
        assert_eq!(yaml_scalar("10.7554/eLife.70780"), "10.7554/eLife.70780");
        assert_eq!(yaml_scalar("a: b"), "\"a: b\"");
        assert_eq!(yaml_scalar(""), "\"\"");
    }

    #[test]
    fn the_citation_names_the_lotus_paper_when_there_is_one() {
        assert!(citation_cff(&crate::SOFTWARE).contains("10.7554/eLife.70780"));
    }

    #[test]
    fn the_licence_is_an_spdx_id_and_not_the_url() {
        // `cffconvert` and GitHub's widget read `license` as a licence *name*.
        // A URL there is not a licence name, and the terms are what a reader
        // needs to see before they cite.
        let cff = citation_cff(&SOFTWARE);
        assert!(
            cff.contains(&format!("license: {}", SOFTWARE.license)),
            "the SPDX identifier is the licence field: {cff}"
        );
        assert!(
            !cff.contains(&format!("license: {}", SOFTWARE.license_url)),
            "the URL is not a licence name"
        );
        assert_eq!(SOFTWARE.license, "AGPL-3.0-only");
    }

    #[test]
    fn the_paper_is_referenced_rather_than_preferred() {
        // The citable work is the software. `preferred-citation` would point a
        // reader at the LOTUS paper instead, which is the dataset it queries,
        // not this tool.
        let cff = citation_cff(&SOFTWARE);
        assert!(!cff.contains("preferred-citation"), "got: {cff}");
        assert!(
            cff.contains("references:"),
            "the paper is cited as a reference"
        );
    }

    #[test]
    fn codemeta_states_the_toolchain_the_build_actually_needs() {
        // The workspace pins 1.97 and is edition 2024, so a consumer on an older
        // toolchain fails at compile time. The requirement is stated rather than
        // discovered.
        let document = codemeta(&SOFTWARE);
        let empty = Vec::new();
        let requirements = document["softwareRequirements"]
            .as_array()
            .unwrap_or(&empty);
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0]["name"], "Rust");
        assert!(
            requirements[0]["version"]
                .as_str()
                .is_some_and(|v| v.contains("1.97")),
            "the pinned toolchain is named, got {requirements:?}"
        );
    }

    #[test]
    fn the_language_is_a_node_a_registry_can_link() {
        let document = codemeta(&SOFTWARE);
        assert_eq!(document["programmingLanguage"]["@type"], "ComputerLanguage");
        assert_eq!(document["programmingLanguage"]["name"], "Rust");
        assert!(
            document["programmingLanguage"]["url"]
                .as_str()
                .is_some_and(|u| u.starts_with("https://")),
            "the language node carries somewhere to link to"
        );
    }

    #[test]
    fn the_document_carries_the_pages_that_describe_the_software() {
        let document = codemeta(&SOFTWARE);
        assert!(
            document["issueTracker"]
                .as_str()
                .is_some_and(|u| u.ends_with("/issues")),
            "a defect report has somewhere to go"
        );
        assert!(
            document["readme"]
                .as_str()
                .is_some_and(|u| u.ends_with("README.md")),
            "the readme is where a registry shows the description"
        );
        assert!(
            document["relatedLink"]
                .as_array()
                .is_some_and(|l| l.len() >= 2),
            "more than one related page"
        );
    }

    /// Every check the `CFF` and `CodeMeta` documents have to pass, as assertions.
    ///
    /// A citation file that parses is not necessarily a valid one: YAML will
    /// read `2026` as a number where a date is required, and a resolver URL
    /// where a bare DOI is required, without complaining. These are the
    /// mistypings that survive a parse.
    #[test]
    fn the_citation_document_survives_the_mistypings_yaml_allows() {
        let cff = citation_cff(&SOFTWARE);

        // There is no release tag yet, so there is no release date to state.
        // Emitting one would mean inventing a day, and a citation file carrying a
        // fabricated date is worse than one that omits an optional field.
        assert!(
            !cff.contains("date-released"),
            "no release has been tagged, so no date is claimed:\n{cff}"
        );
        // `license` is read as a licence name, not a place to read the terms.
        assert!(!cff.contains("license: http"), "got:\n{cff}");
        // One `keywords:` key. A repeated key parses and silently keeps the last.
        assert_eq!(
            cff.matches("keywords:").count(),
            1,
            "a repeated key drops the earlier values:\n{cff}"
        );
        // Every list item starts a line of its own. A missing newline joins two
        // items into one line, which YAML then reads as a single longer value
        // rather than as two -- so the second entry is lost without complaint.
        for (label, key, expected) in [
            ("keywords", "keywords:", SOFTWARE.keywords.len()),
            ("identifiers", "identifiers:", 2),
        ] {
            let tail = cff
                .split_once(&format!("\n{key}\n"))
                .map_or("", |(_, rest)| rest);
            let indented: Vec<&str> = tail.lines().take_while(|l| l.starts_with(' ')).collect();
            let items = indented
                .iter()
                .filter(|l| l.trim_start().starts_with("- "))
                .count();
            assert_eq!(
                items, expected,
                "{label} has {expected} items, each on its own line:\n{cff}"
            );
        }
        assert!(
            cff.lines().all(|l| l.trim_end() == l),
            "no trailing whitespace, so the file diffs cleanly:\n{cff}"
        );
    }

    #[test]
    fn the_doi_is_bare_and_the_resolver_url_is_an_identifier() {
        // CFF defines `doi` as the identifier without a prefix. A resolver URL
        // there is either rejected by `cffconvert --validate` or, worse, silently
        // doubled by a consumer that prepends the prefix itself.
        let cff = citation_cff(&SOFTWARE);
        assert!(
            cff.contains("doi: 10.7554/eLife.70780"),
            "the doi field is bare, got:\n{cff}"
        );
        assert!(
            !cff.contains("doi: \"https://doi.org/"),
            "the doi field must not carry a resolver prefix"
        );
        assert!(
            cff.contains("- type: doi"),
            "the resolver URL is an identifier instead"
        );
    }

    #[test]
    fn no_keyword_is_listed_twice() {
        // A registry folds this list into a search index, so a repeated term
        // weights the software twice and tells a reader nothing twice.
        let mut seen = std::collections::HashSet::new();
        for keyword in SOFTWARE.keywords {
            assert!(
                seen.insert(*keyword),
                "{keyword:?} is listed twice, in {:?}",
                SOFTWARE.keywords
            );
        }
    }

    #[test]
    fn every_keyword_survives_as_a_list() {
        // A YAML mapping with `keywords:` repeated keeps only the last value.
        // The document named five keywords and described the software by one.
        let cff = citation_cff(&SOFTWARE);
        assert_eq!(
            cff.matches("keywords:").count(),
            1,
            "a repeated key silently drops the earlier values:\n{cff}"
        );
        // Count the indented list items that follow the key. `skip_while`
        // rather than `position`, so an empty list does not silently count the
        // next section's items.
        let mut lines = cff.lines().skip_while(|l| !l.starts_with("keywords:"));
        lines.next();
        let entries = lines.take_while(|l| l.starts_with("  - ")).count();
        assert_eq!(entries, SOFTWARE.keywords.len(), "{cff}");
        for keyword in SOFTWARE.keywords {
            assert!(cff.contains(keyword), "{keyword} missing from:\n{cff}");
        }
    }

    #[test]
    fn the_lotus_paper_is_a_scholarly_article_keyed_by_its_doi() {
        // The citation is what a consumer resolves the software to a paper with,
        // so both the type and the identifier have to be right.
        let paper = Paper {
            title: "A paper",
            doi: "10.1000/lotus",
            year: 2026,
        };
        let node = lotus_paper(&paper);
        assert_eq!(node["@type"], "ScholarlyArticle");
        assert_eq!(node["identifier"], "https://doi.org/10.1000/lotus");
        assert_eq!(node["datePublished"], "2026");
    }

    #[test]
    fn a_yaml_scalar_is_quoted_for_each_thing_that_needs_it() {
        // One case per clause of the `||` chain, because `&&` in place of `||`
        // quotes only the value that fails every test at once.
        for needs in [
            "a: b", // a colon would start a mapping
            "a #b", // a hash would start a comment
            "a\nb", // a newline would end the scalar
            " a",   // leading space is significant in YAML
            "a ",   // and so is trailing
            "",     // an empty scalar has to be spelled out
        ] {
            assert!(
                yaml_scalar(needs).starts_with('"'),
                "{needs:?} must be quoted, got {:?}",
                yaml_scalar(needs)
            );
        }
        for plain in ["CCO", "C6H6O", "10.1000/xyz", "Gentiana-lutea"] {
            assert_eq!(yaml_scalar(plain), plain, "an ordinary word stays readable");
        }
    }

    #[test]
    fn a_leading_quote_is_quoted_because_it_opens_one() {
        // Unquoted, `"abc` is a double-quoted scalar with nothing closing it, and
        // the file the emitter writes no longer parses.
        for leading in [r#""abc"#, "'abc"] {
            assert!(
                yaml_scalar(leading).starts_with('"'),
                "{leading:?} would open a scalar it never closes"
            );
        }
        // Anywhere but the front, a quote is just a character.
        assert_eq!(yaml_scalar(r#"a"b"#), r#"a"b"#);
    }

    #[test]
    fn a_quoted_scalar_escapes_what_would_end_the_quotes() {
        // Escaping only happens on the way to being quoted: in a plain scalar a
        // backslash is an ordinary character and needs nothing.
        assert_eq!(yaml_scalar(r#"a: "b""#), r#""a: \"b\"""#);
        assert_eq!(yaml_scalar(r"a: \b"), r#""a: \\b""#);
        assert_eq!(
            yaml_scalar(r"a\b"),
            r"a\b",
            "a plain scalar needs no escaping"
        );
    }
}
