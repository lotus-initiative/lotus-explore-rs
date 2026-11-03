// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The LOTUS PubChem tree generator: fetch Wikidata tables, build biological
//! / chemical / NPClassifier trees, serialize JSON.
//!
//! Mirrors `lotus_pubchem_tree.py`: it fetches the same 12 SPARQL tables,
//! indexes them by QID, builds three hierarchical trees, and writes JSON in
//! either a rich "full" form or PubChem's compact name-as-key form.
//!
//! The trees are typed (the `nodes` module), so each holds only what its document
//! carries; JSON exists only where a document is rendered.
//!
//! No clock: the timestamp comes from the caller. No UI: the CLI writes the
//! files. Wasm-clean: all I/O goes through the injected [`lotus_search::Http`] transport.

mod compact;
mod document;
mod index;
mod nodes;
mod npclassifier;
mod trees;

use lotus_search::{Endpoint, Http, HttpResponse};
use serde::Serialize;

use self::document::{FullDocument, TreeKind};
use self::index::{Index, fetch_all_pubchem_data};
use self::npclassifier::{SmilesIndex, classify, npclassifier_tree};
use self::trees::{biological_tree, chemical_tree};
use super::pipeline::{ExportError, read_csv_rows};

/// The `NPClassifier` cache CSV URL.
const NPCLASSIFIER_CACHE_URL: &str = "https://media.githubusercontent.com/media/adafede/marimo/main/apps/public/npclassifier/npclassifier_cache.csv";

/// The app version carried in every metadata document.
const APP_VERSION: &str = "0.1.0";
/// The generator name carried in every metadata document.
const APP_NAME: &str = "LOTUS PubChem Tree Generator";

/// Get an arbitrary URL's text body (`NPClassifier` cache CSV).
async fn http_get_text<H: Http>(http: &H, url: &str) -> Result<String, ExportError> {
    let response = http.get(url, "text/csv").await?;
    let text = response
        .text()
        .await
        .map_err(|e| ExportError::Fetch(lotus_search::FetchError::Network(e.to_string())))?;
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
/// Returns (`biological_full`, `chemical_full`, `npclassifier_full`,
/// `biological_pubchem`, `chemical_pubchem`, `npclassifier_pubchem`) as JSON strings.
/// `npclassifier_*` are empty strings when the cache could not be read.
///
/// # Errors
///
/// Returns an error if fetching from the endpoint or cache fails.
pub async fn run_pubchem_tree<H: Http>(
    http: &H,
    endpoint: Endpoint,
    now: &str,
) -> Result<(String, String, String, String, String, String), ExportError> {
    let data = fetch_all_pubchem_data(http, &endpoint).await?;
    let index = Index::new(&data);

    let biological = biological_tree(&data, &index);
    let chemical = chemical_tree(&data, &index);

    let npclassifier_rows = http_get_text(http, NPCLASSIFIER_CACHE_URL)
        .await
        .ok()
        .and_then(|text| read_csv_rows(text.as_bytes()).ok())
        .unwrap_or_default();
    let npclassifier = npclassifier_tree(
        &classify(&npclassifier_rows),
        &SmilesIndex::new(&data, &index),
        &index,
    );

    let (npclassifier_full, npclassifier_pubchem) = if npclassifier.is_empty() {
        (String::new(), String::new())
    } else {
        (
            render(&FullDocument::new(
                TreeKind::NpClassifier,
                &npclassifier,
                now,
            ))?,
            render(&compact::npclassifier(&npclassifier))?,
        )
    };

    Ok((
        render(&FullDocument::new(TreeKind::Biological, &biological, now))?,
        render(&FullDocument::new(TreeKind::Chemical, &chemical, now))?,
        npclassifier_full,
        render(&compact::biological(&biological))?,
        render(&compact::chemical(&chemical))?,
        npclassifier_pubchem,
    ))
}

#[cfg(test)]
#[path = "pubchem_tree/tests.rs"]
mod tests;
