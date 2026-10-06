// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

const QS_DEV_HOME: &str = "https://qs-dev.toolforge.org/";
const QS_DEV_BATCH_NEW: &str = "https://qs-dev.toolforge.org/batch/new/?v1=";

pub fn build_qs_dev_link(commands: &str) -> String {
    let trimmed = commands.trim();
    if trimmed.is_empty() {
        return QS_DEV_HOME.into();
    }
    format!("{QS_DEV_BATCH_NEW}{}", urlencoding::encode(trimmed))
}

#[cfg(test)]
#[path = "quickstatements/tests.rs"]
mod tests;
