// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Export helpers split by concern:

mod filename;
mod filters;
mod metadata;
mod urls;

// The download URLs are the server's; the filename guard is the client's. They
// are re-exported from one module because both are about the same operation, and
// a target that uses neither will say so.
#[cfg_attr(not(feature = "server"), allow(unused_imports))]
pub use lotus_query::sanitize_download_filename;
#[cfg_attr(not(feature = "server"), allow(unused_imports))]
pub use urls::{api_export_file_url, qlever_export_url};

pub use filename::generate_filename;
pub use metadata::{MetadataInputs, SparqlEndpoint, build_metadata_json};
