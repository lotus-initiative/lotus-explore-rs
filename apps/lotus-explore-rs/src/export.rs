// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Export helpers that belong to the host rather than to the domain.
//!
//! The filename, the filter block and the metadata document live in
//! `lotus-explore-core`: they are the same for every host. What is left here
//! is where an export is *served* from, which is a property of the server
//! that runs it.
//!
//! Export helpers split by concern:

// The download URLs appear in the server's metadata document. Nothing in the
// browser client asks for one, so a wasm build does not compile the module
// rather than compiling two functions it will never call.
#[cfg(not(target_arch = "wasm32"))]
mod urls;

// The download URLs are the server's; the filename guard is the client's. They
// are re-exported from one module because both are about the same operation, and
// a target that uses neither will say so.
#[cfg_attr(not(feature = "server"), allow(unused_imports))]
pub use lotus_query::sanitize_download_filename;
// The server's metadata document is the only place these appear.
#[cfg(feature = "server")]
pub use urls::{api_export_file_url, qlever_export_url};
