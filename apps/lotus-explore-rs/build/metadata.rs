// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Configuration schema and canonical host extraction for `site-metadata.json`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub site: Site,
    pub manifest: Manifest,
}

#[derive(Debug, Deserialize)]
pub struct Site {
    pub name: String,
    pub short_name: String,
    pub description: String,
    /// The site's canonical URL including its subpath, and the single source of
    /// the host. `index.html` holds two hand-written copies of it that a test
    /// ties back here. See docs/DEPLOYMENT.md, "Changing the CNAME".
    pub base_url: String,
    pub repo_url: String,
    pub issues_url: String,
    pub discussions_url: String,
    /// Where the wider LOTUS initiative lives; defaults to this site's origin.
    #[serde(default)]
    pub lotus_home_url: Option<String>,
    pub paper_doi_url: String,
    pub paper_landing_url: String,
    pub app_license_url: String,
    pub data_license_url: String,
    pub security_contact_url: String,
    pub security_policy_url: String,
    pub source_path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    #[serde(skip_deserializing)]
    pub name: String,
    #[serde(skip_deserializing)]
    pub short_name: String,
    #[serde(skip_deserializing)]
    pub description: String,
    start_url: String,
    scope: String,
    /// Stable app identity, so a reinstall is not treated as a different app.
    /// Resolved against the origin, which keeps it inside `scope` for a
    /// subpath deploy.
    id: Option<String>,
    display: String,
    /// Progressive enhancement: the browser takes the first mode it supports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_override: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    orientation: Option<String>,
    background_color: String,
    theme_color: String,
    lang: String,
    dir: String,
    /// Omitted rather than emitted as `[]`: an empty screenshots array is worse
    /// than an absent one, and there are no screenshots to declare.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    screenshots: Option<Vec<String>>,
    icons: Vec<Icon>,
    categories: Vec<String>,
    prefer_related_applications: bool,
    shortcuts: Vec<Shortcut>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Icon {
    src: String,
    sizes: String,
    #[serde(rename = "type")]
    mime_type: String,
    purpose: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shortcut {
    name: String,
    short_name: String,
    description: String,
    url: String,
    icons: Vec<Icon>,
}

/// The origin (`scheme://host[:port]`) `base_url` sits on, subpath stripped.
///
/// Not a parsed `Url`: the value is only ever formatted into a string, and a
/// parser to read one field of it is more machinery than the extraction.
pub fn host(meta: &Metadata) -> String {
    let base = meta.site.base_url.trim_end_matches('/');
    let scheme = base.split_once("://").map_or("https", |(scheme, _)| scheme);
    format!("{scheme}://{}", hostname(meta))
}

/// The LOTUS initiative home: an override if set, else this site's own origin.
pub fn lotus_home(meta: &Metadata) -> String {
    meta.site
        .lotus_home_url
        .clone()
        .unwrap_or_else(|| host(meta))
}

/// The authority (host plus any port) from `base_url`, without the scheme.
pub fn hostname(meta: &Metadata) -> &str {
    let base = meta.site.base_url.trim_end_matches('/');
    let rest = base.split_once("://").map_or(base, |(_, rest)| rest);
    let authority = rest.split('/').next().unwrap_or(rest);
    authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host)
}
