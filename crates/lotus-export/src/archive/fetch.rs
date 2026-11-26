// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Paginated `QLever` SMILES SPARQL client and binding decoders.

use lotus_query::{
    ARCHIVE_PAGE_SIZE, ARCHIVE_SMILES_QUERY, ARCHIVE_SMILES_WITHOUT_INCHIKEY_QUERY, archive_page,
};
use lotus_search::{Endpoint, Http};
use serde::Deserialize;

use super::{ArchiveError, MissingInchiKeyRecord, SmilesKind, SmilesRecord};

/// SPARQL JSON response shape for the archive queries.
#[derive(Deserialize)]
struct SparqlResponse {
    results: SparqlResults,
}

#[derive(Deserialize)]
struct SparqlResults {
    bindings: Vec<SparqlBinding>,
}

#[derive(Deserialize)]
pub(super) struct SparqlBinding {
    compound: SparqlValue,
    #[serde(rename = "inchiKey")]
    inchi_key: Option<SparqlValue>,
    smiles: SparqlValue,
    #[serde(rename = "smilesKind")]
    smiles_kind: SparqlValue,
}

#[derive(Deserialize)]
pub(super) struct SparqlValue {
    value: String,
}

/// HTTP client for fetching LOTUS SMILES from `QLever`.
#[derive(Clone, Debug)]
pub(super) struct QleverClient<'a, H: Http + 'a> {
    http: &'a H,
    endpoint: Endpoint,
}

impl<'a, H: Http + 'a> QleverClient<'a, H> {
    /// Constructs a client for the supplied `QLever` SPARQL endpoint.
    pub(super) const fn new(http: &'a H, endpoint: Endpoint) -> Self {
        Self { http, endpoint }
    }

    /// Executes every page of [`ARCHIVE_SMILES_QUERY`] (ordered by `?compound
    /// ?smilesKind ?smiles` and excluding deprecated `P703` ranks) and returns
    /// the duplicate-free source records.
    ///
    /// # Errors
    /// Returns [`ArchiveError::Fetch`] when the endpoint fails to answer.
    pub(super) async fn fetch_lotus_smiles(&self) -> Result<Vec<SmilesRecord>, ArchiveError> {
        decode_sparql_bindings(self.fetch_bindings(ARCHIVE_SMILES_QUERY).await?)
    }

    /// Retrieves source values that cannot be deduplicated because `P235` is absent.
    ///
    /// # Errors
    /// Returns [`ArchiveError::Fetch`] when the endpoint fails to answer.
    pub(super) async fn fetch_missing_inchi_key_smiles(
        &self,
    ) -> Result<Vec<MissingInchiKeyRecord>, ArchiveError> {
        decode_missing_inchi_key_bindings(
            self.fetch_bindings(ARCHIVE_SMILES_WITHOUT_INCHIKEY_QUERY)
                .await?,
        )
    }

    pub(super) async fn fetch_bindings(
        &self,
        base_query: &str,
    ) -> Result<Vec<SparqlBinding>, ArchiveError> {
        let mut bindings = Vec::new();
        let mut offset = 0;

        loop {
            let page = self.fetch_page(base_query, offset).await?;
            let page_len = page.len();
            bindings.extend(page);

            if page_len < ARCHIVE_PAGE_SIZE {
                return Ok(bindings);
            }
            offset = offset.checked_add(ARCHIVE_PAGE_SIZE).ok_or_else(|| {
                ArchiveError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "QLever page offset overflow",
                ))
            })?;
        }
    }

    async fn fetch_page(
        &self,
        base_query: &str,
        offset: usize,
    ) -> Result<Vec<SparqlBinding>, ArchiveError> {
        let query = archive_page(base_query, offset);
        let answer = lotus_search::execute(
            self.http,
            self.endpoint.clone(),
            &query,
            lotus_search::ResponseFormat::SparqlJson,
        )
        .await
        .map_err(ArchiveError::Fetch)?;
        let text = answer.text().map_err(ArchiveError::Fetch)?;
        let response: SparqlResponse = serde_json::from_str(&text).map_err(ArchiveError::Json)?;
        Ok(response.results.bindings)
    }
}

fn decode_sparql_bindings(bindings: Vec<SparqlBinding>) -> Result<Vec<SmilesRecord>, ArchiveError> {
    bindings
        .into_iter()
        .map(|binding| {
            let wikidata_id = wikidata_id(&binding.compound.value)?;
            let inchi_key = binding
                .inchi_key
                .ok_or(ArchiveError::MissingColumn {
                    table: "smiles",
                    column: "inchiKey",
                })?
                .value;
            Ok(SmilesRecord {
                inchi_key,
                wikidata_id,
                smiles_kind: SmilesKind::parse(&binding.smiles_kind.value)?,
                smiles: binding.smiles.value,
            })
        })
        .collect()
}

fn decode_missing_inchi_key_bindings(
    bindings: Vec<SparqlBinding>,
) -> Result<Vec<MissingInchiKeyRecord>, ArchiveError> {
    bindings
        .into_iter()
        .map(|binding| {
            if binding.inchi_key.is_some() {
                return Err(ArchiveError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "QLever missing-InChIKey result unexpectedly includes P235",
                )));
            }
            Ok(MissingInchiKeyRecord {
                wikidata_id: wikidata_id(&binding.compound.value)?,
                smiles_kind: SmilesKind::parse(&binding.smiles_kind.value)?,
                smiles: binding.smiles.value,
            })
        })
        .collect()
}

fn wikidata_id(uri: &str) -> Result<String, ArchiveError> {
    uri.rsplit('/')
        .next()
        .filter(|id| id.starts_with('Q'))
        .ok_or_else(|| {
            ArchiveError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "QLever compound is not a Wikidata entity URI",
            ))
        })
        .map(ToOwned::to_owned)
}
