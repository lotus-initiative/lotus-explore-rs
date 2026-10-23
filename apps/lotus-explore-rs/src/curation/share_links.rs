// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::{CurationInputRow, Locale};
use crate::features::explore::url_state::{is_true_flag, read_url_query_params};
use std::collections::BTreeMap;

pub const CURATION_ROWS_PARAM: &str = "curation_rows";
const CURATION_RUN_PARAM: &str = "curation_run";

pub fn initial_curation_rows_from_url() -> Vec<CurationInputRow> {
    let params = read_url_query_params();
    curation_rows_from_query_params(&params)
}

pub fn initial_curation_autorun_from_url() -> bool {
    let params = read_url_query_params();
    params
        .get(CURATION_RUN_PARAM)
        .is_some_and(|value| is_true_flag(value))
}

pub fn build_curation_share_url(
    rows: &[CurationInputRow],
    locale: Locale,
    autorun: bool,
) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    let params = curation_query_params(rows, locale, autorun);
    let query = params
        .into_iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding::encode(&key),
                urlencoding::encode(&value)
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    Some(format!("/curation?{query}"))
}

fn curation_query_params(
    rows: &[CurationInputRow],
    locale: Locale,
    autorun: bool,
) -> BTreeMap<String, String> {
    let mut params = BTreeMap::new();
    params.insert("lang".into(), locale.lang_code().into());
    params.insert(
        CURATION_ROWS_PARAM.into(),
        serde_json::to_string(rows).unwrap_or_else(|_| String::from("[]")),
    );
    if autorun {
        params.insert(CURATION_RUN_PARAM.into(), "true".into());
    }
    params
}

pub fn curation_rows_from_query_params(params: &BTreeMap<String, String>) -> Vec<CurationInputRow> {
    params
        .get(CURATION_ROWS_PARAM)
        .and_then(|raw| serde_json::from_str::<Vec<CurationInputRow>>(raw).ok())
        .unwrap_or_default()
}
