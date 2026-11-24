// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The LOTUS `PubChem` tree generator: fetch Wikidata tables, build biological
//! / chemical / `NPClassifier` trees, serialize JSON.
//!
//! Mirrors `lotus_pubchem_tree.py`: it fetches the same 12 SPARQL tables,
//! indexes them by QID, builds three hierarchical trees, and writes JSON in
//! either a rich "full" form or `PubChem`'s compact name-as-key form.
//!
//! The trees are typed (the `nodes` module), so each holds only what its document
//! carries; JSON exists only where a document is rendered.
//!
//! No clock: the timestamp comes from the caller. No UI: the CLI writes the
//! files. Wasm-clean: all I/O goes through the injected [`lotus_search::Http`] transport.

mod compact;
mod document;
mod fetch;
mod index;
mod nodes;
mod npclassifier;
mod trees;

use lotus_search::{Endpoint, Http, HttpResponse};
use serde::Serialize;

use self::document::{FullDocument, TreeKind};
pub use self::fetch::PUBCHEM_TREE_QUERY_COUNT;
use self::fetch::fetch_all_pubchem_data;
use self::index::Index;
use self::npclassifier::{SmilesIndex, classify, npclassifier_tree};
use self::trees::{biological_tree, chemical_tree};
use crate::pipeline::{ExportError, read_csv_rows};

/// The `NPClassifier` cache CSV URL.
const NPCLASSIFIER_CACHE_URL: &str = "https://media.githubusercontent.com/media/adafede/marimo/main/apps/public/npclassifier/npclassifier_cache.csv";

/// The app version carried in every metadata document.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The generator name carried in every metadata document.
const APP_NAME: &str = "LOTUS PubChem Tree Generator";

/// Parameters controlling a [`run_pubchem_tree`] run.
#[derive(Debug, Clone)]
pub struct PubchemTreeOptions<'a> {
    /// SPARQL endpoint to query.
    pub endpoint: Endpoint,
    /// Caller-supplied ISO-8601 UTC timestamp written into document metadata.
    pub now: &'a str,
}

/// The six JSON documents produced by [`run_pubchem_tree`].
///
/// `npclassifier_full` and `npclassifier_pubchem` are empty strings only when
/// no loaded structure matches a row in the `NPClassifier` cache.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct PubchemTreeOutputs {
    /// Full biological taxonomy tree (`lotus_biological_tree.json`).
    pub biological_full: String,
    /// Full Wikidata chemical class tree (`lotus_chemical_tree.json`).
    pub chemical_full: String,
    /// Full `NPClassifier` tree (`lotus_npclassifier_tree.json`).
    pub npclassifier_full: String,
    /// Compact `PubChem`-format biological tree (`lotus_biological_tree_pubchem.json`).
    pub biological_pubchem: String,
    /// Compact `PubChem`-format chemical tree (`lotus_chemical_tree_pubchem.json`).
    pub chemical_pubchem: String,
    /// Compact `PubChem`-format `NPClassifier` tree (`lotus_npclassifier_tree_pubchem.json`).
    pub npclassifier_pubchem: String,
}

/// Get an arbitrary URL's text body (`NPClassifier` cache CSV).
async fn http_get_text<H: Http>(http: &H, url: &str) -> Result<String, ExportError> {
    let response = http.get(url, "text/csv").await?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| ExportError::Fetch(lotus_search::FetchError::Network(e.to_string())))?;
    if !(200..300).contains(&status) {
        return Err(ExportError::Fetch(lotus_search::FetchError::Http {
            status,
            message: text,
        }));
    }
    Ok(text)
}

/// Pretty-print one document. It goes through [`serde_json::Value`] because
/// that map keeps its keys sorted, which is the key order the documents have
/// always had; a struct written directly would keep its field order instead.
fn render(document: &impl Serialize) -> Result<String, ExportError> {
    serde_json::to_value(document)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))
}

/// Run the full `PubChem` tree pipeline: fetch, index, build three trees, and
/// serialize each in both full and `PubChem` compact formats.
///
/// # Errors
///
/// Returns an error if fetching or parsing any Wikidata table or the
/// `NPClassifier` cache CSV fails.
pub async fn run_pubchem_tree<H: Http>(
    http: &H,
    options: &PubchemTreeOptions<'_>,
    mut on_progress: impl FnMut(&'static str),
) -> Result<PubchemTreeOutputs, ExportError> {
    let data = fetch_all_pubchem_data(http, &options.endpoint, &mut on_progress).await?;
    let index = Index::new(&data);

    let biological = biological_tree(&data, &index);
    let chemical = chemical_tree(&data, &index);

    on_progress("npclassifier_cache");
    let npclassifier_text = http_get_text(http, NPCLASSIFIER_CACHE_URL).await?;
    let npclassifier_rows = read_csv_rows(npclassifier_text.as_bytes())?;
    let npclassifier = npclassifier_tree(
        &classify(&npclassifier_rows),
        &SmilesIndex::new(&data, &index),
        &index,
    );

    let sparql_endpoint = options.endpoint.url();
    let (npclassifier_full, npclassifier_pubchem) = if npclassifier.is_empty() {
        (String::new(), String::new())
    } else {
        (
            render(&FullDocument::new(
                TreeKind::NpClassifier,
                &npclassifier,
                options.now,
                sparql_endpoint,
            ))?,
            render(&compact::npclassifier(&npclassifier))?,
        )
    };

    Ok(PubchemTreeOutputs {
        biological_full: render(&FullDocument::new(
            TreeKind::Biological,
            &biological,
            options.now,
            sparql_endpoint,
        ))?,
        chemical_full: render(&FullDocument::new(
            TreeKind::Chemical,
            &chemical,
            options.now,
            sparql_endpoint,
        ))?,
        npclassifier_full,
        biological_pubchem: render(&compact::biological(&biological))?,
        chemical_pubchem: render(&compact::chemical(&chemical))?,
        npclassifier_pubchem,
    })
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
