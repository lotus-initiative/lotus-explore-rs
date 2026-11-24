// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL [`Service`], resolved [`Endpoint`], and [`Answer`] / [`StreamAnswer`] types.

use crate::client::ChunkedBody;
use crate::error::FetchError;
use lotus_model::{QLEVER_WIKIDATA, WDQS_SCHOLARLY, WDQS_WIKIDATA};

/// Which service answered, and therefore what the provenance should say.
///
/// The three variants are the public services. Any of them can be pointed
/// elsewhere by an environment variable, which is how someone runs the CLI
/// against their own `QLever` or a mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    /// The default, and much the fastest.
    Qlever,
    /// The main Wikidata Query Service.
    Wdqs,
    /// The scholarly subgraph, which serves the reference properties.
    Scholarly,
}

impl Service {
    /// The public URL for this service.
    #[must_use]
    pub const fn url(self) -> &'static str {
        match self {
            Self::Qlever => QLEVER_WIKIDATA,
            Self::Wdqs => WDQS_WIKIDATA,
            Self::Scholarly => WDQS_SCHOLARLY,
        }
    }

    /// The environment variable that overrides this service's URL.
    pub(super) const fn variable(self) -> &'static str {
        match self {
            Self::Qlever => "LOTUS_QLEVER_ENDPOINT",
            Self::Wdqs => "LOTUS_WDQS_ENDPOINT",
            Self::Scholarly => "LOTUS_WDQS_SCHOLARLY_ENDPOINT",
        }
    }

    /// The URL to POST to: the override if one is set, else the public one.
    #[must_use]
    pub fn target(self) -> String {
        self.target_from(|name| std::env::var(name).ok())
    }

    /// Like [`Self::target`], but with the environment supplied by the caller.
    #[must_use]
    pub fn target_from<F>(self, mut env: F) -> String
    where
        F: FnMut(&str) -> Option<String>,
    {
        env(self.variable())
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| self.url().to_string())
    }

    /// How to name the service in provenance metadata.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Qlever => "QLever",
            Self::Wdqs => "Wikidata Query Service",
            Self::Scholarly => "Wikidata Query Service (scholarly subgraph)",
        }
    }
}

impl std::fmt::Display for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A [`Service`] and the URL actually used, so a caller can report which one
/// answered without re-reading the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    service: Service,
    url: String,
}

impl Endpoint {
    /// Resolve a service to the URL that will actually be used, honouring an
    /// environment override.
    #[must_use]
    pub fn new(service: Service) -> Self {
        Self {
            service,
            url: service.target(),
        }
    }

    /// The service this is.
    #[must_use]
    pub const fn service(&self) -> Service {
        self.service
    }

    /// The URL that queries are posted to.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Default for Endpoint {
    fn default() -> Self {
        Self::new(Service::Qlever)
    }
}

/// A response, and where it came from.
#[derive(Debug, Clone)]
pub struct Answer {
    /// Where it came from, which the provenance should record.
    pub endpoint: Endpoint,
    /// The raw body, in the format that was asked for.
    pub body: Vec<u8>,
}

impl Answer {
    /// The body as UTF-8.
    ///
    /// # Errors
    /// Returns [`FetchError::Parse`] if the body is not UTF-8, which a SPARQL
    /// response is always meant to be.
    pub fn text(self) -> Result<String, FetchError> {
        String::from_utf8(self.body)
            .map_err(|e| FetchError::Parse(format!("the response was not UTF-8: {e}")))
    }
}

/// A response whose body has not been read.
///
/// Returned by [`execute_streaming`](super::execute_streaming). `endpoint` is
/// kept so the provenance can still say which service answered: by the time the
/// body has been read, the response that carried the headers is gone.
pub struct StreamAnswer {
    /// Where it came from, which the provenance should record.
    pub endpoint: Endpoint,
    /// The body, read one chunk at a time.
    pub chunks: ChunkedBody,
}

impl std::fmt::Debug for StreamAnswer {
    /// The endpoint and nothing else: the body has not been read, and reading it
    /// to describe it would defeat the point of having streamed it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamAnswer")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}
