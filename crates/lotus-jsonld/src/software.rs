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
    /// The concept DOI, when the project has minted one.
    pub doi: Option<&'static str>,
    /// The released version.
    pub version: &'static str,
    /// The SPDX licence identifier.
    pub license: &'static str,
    /// Subject terms, for discovery.
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
        let paper = lotus_paper("10.1000/lotus");
        assert_eq!(paper["@type"], "ScholarlyArticle");
        assert_eq!(paper["identifier"], "https://doi.org/10.1000/lotus");
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
