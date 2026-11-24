// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Minimal native Zenodo REST API v1 client for `lotus archive --publish`.
//!
//! Replaces `zenodo-rs` (which pulled `chrono` and a second rustls crypto backend)
//! with the small subset of Zenodo's deposition API the archive subcommand uses:
//! creating or versioning a draft, updating metadata, replacing files in the
//! draft bucket, and publishing.

mod client;
mod publish;
mod record;

pub use client::ZenodoClient;
pub use record::{
    Community, Creator, DepositMetadata, DepositionId, RelatedIdentifier, UploadFile,
    prepare_uploads,
};

#[cfg(test)]
use client::{MAX_TRANSIENT_ATTEMPTS, PRODUCTION_API_URL, SANDBOX_API_URL};
#[cfg(test)]
use publish::parse_record_id_from_url;
#[cfg(test)]
use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
