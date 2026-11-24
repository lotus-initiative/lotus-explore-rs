// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Zenodo deposition versioning, metadata, bucket upload, and publish workflow.

use anyhow::{Context, bail};
use reqwest::Method;
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use serde::Serialize;

use super::ZenodoClient;
use super::client::{ensure_success, parse_action_deposition, parse_json_response};
use super::types::{DepositMetadata, Deposition, DepositionId, PublishedRecord, UploadFile};

impl ZenodoClient {
    /// Creates a new deposition (or versions `root_id`), updates metadata,
    /// replaces draft files with `uploads`, and publishes the record.
    ///
    /// # Errors
    /// Returns an error if any step in the Zenodo deposition lifecycle fails.
    pub async fn publish_dataset(
        &self,
        root_id: Option<DepositionId>,
        metadata: &DepositMetadata,
        uploads: &[UploadFile],
    ) -> anyhow::Result<PublishedRecord> {
        let draft = match root_id {
            Some(id) => self
                .ensure_editable_draft(id)
                .await
                .context("ensure editable draft")?,
            None => self
                .create_deposition()
                .await
                .context("create Zenodo deposition")?,
        };
        let draft_id = DepositionId(draft.id);
        let updated = self
            .update_metadata(draft_id, metadata)
            .await
            .context("update Zenodo metadata")?;
        let refreshed = if updated.links.bucket.is_some() {
            updated
        } else {
            self.get_deposition(draft_id)
                .await
                .context("refresh Zenodo draft")?
        };
        let bucket = refreshed
            .links
            .bucket
            .as_deref()
            .context("Zenodo draft is missing links.bucket")?;

        for existing in &refreshed.files {
            self.delete_file(draft_id, &existing.id)
                .await
                .with_context(|| format!("delete existing draft file {}", existing.id))?;
        }

        for file in uploads {
            self.upload_file(bucket, file)
                .await
                .with_context(|| format!("upload {}", file.filename))?;
        }

        let published = self
            .publish_and_wait(draft_id)
            .await
            .context("publish Zenodo deposition")?;
        let record_id = DepositionId(published.record_id.unwrap_or(published.id));
        Ok(PublishedRecord { record_id })
    }

    async fn create_deposition(&self) -> anyhow::Result<Deposition> {
        let url = format!("{}/deposit/depositions", self.base_url);
        let response = self
            .request(Method::POST, &url)
            .json(&serde_json::json!({}))
            .send()
            .await
            .context("POST deposit/depositions")?;
        parse_json_response(response, "create deposition").await
    }

    pub(super) async fn get_deposition(&self, id: DepositionId) -> anyhow::Result<Deposition> {
        let url = format!("{}/deposit/depositions/{id}", self.base_url);
        self.get_deposition_by_url(&url).await
    }

    async fn get_deposition_by_url(&self, url: &str) -> anyhow::Result<Deposition> {
        self.send_with_retries(
            || self.request(Method::GET, url),
            "GET deposition",
            parse_json_response::<Deposition>,
        )
        .await
    }

    async fn ensure_editable_draft(&self, root_id: DepositionId) -> anyhow::Result<Deposition> {
        let deposition = self.get_deposition(root_id).await?;
        if !deposition.submitted {
            return Ok(deposition);
        }

        let latest = self.resolve_latest_published(deposition).await?;
        let new_ver_url = format!(
            "{}/deposit/depositions/{}/actions/newversion",
            self.base_url, latest.id
        );
        let response = self
            .request(Method::POST, &new_ver_url)
            .send()
            .await
            .context("POST actions/newversion")?;
        let versioned =
            parse_action_deposition(response, || self.get_deposition(DepositionId(latest.id)))
                .await?;
        let draft_url = versioned
            .links
            .latest_draft
            .context("Zenodo newversion response is missing links.latest_draft")?;

        for _ in 0..self.max_polls {
            if let Ok(draft) = self.get_deposition_by_url(&draft_url).await
                && !draft.submitted
                && draft.links.bucket.is_some()
            {
                return Ok(draft);
            }
            tokio::time::sleep(self.poll_interval).await;
        }
        bail!("timed out waiting for Zenodo editable draft at {draft_url}");
    }

    async fn resolve_latest_published(&self, deposition: Deposition) -> anyhow::Result<Deposition> {
        if let Some(latest_url) = deposition.links.latest.as_deref()
            && deposition.links.self_.as_deref() != Some(latest_url)
        {
            if latest_url.contains("/deposit/depositions/") {
                return self.get_deposition_by_url(latest_url).await;
            }
            if let Some(record_id) = parse_record_id_from_url(latest_url) {
                return self.get_deposition(DepositionId(record_id)).await;
            }
        }
        Ok(deposition)
    }

    async fn update_metadata(
        &self,
        id: DepositionId,
        metadata: &DepositMetadata,
    ) -> anyhow::Result<Deposition> {
        #[derive(Serialize)]
        struct Payload<'a> {
            metadata: &'a DepositMetadata,
        }
        let url = format!("{}/deposit/depositions/{id}", self.base_url);
        let response = self
            .request(Method::PUT, &url)
            .json(&Payload { metadata })
            .send()
            .await
            .context("PUT deposition metadata")?;
        parse_json_response(response, "update metadata").await
    }

    async fn delete_file(&self, id: DepositionId, file_id: &str) -> anyhow::Result<()> {
        let url = format!(
            "{}/deposit/depositions/{id}/files/{}",
            self.base_url,
            lotus_model::percent_encode(file_id)
        );
        let response = self
            .request(Method::DELETE, &url)
            .send()
            .await
            .context("DELETE draft file")?;
        ensure_success(response, "delete draft file").await?;
        Ok(())
    }

    async fn upload_file(&self, bucket_url: &str, file: &UploadFile) -> anyhow::Result<()> {
        let bytes = std::fs::read(&file.path)
            .with_context(|| format!("read upload file {}", file.path.display()))?;
        let len = bytes.len();
        let upload_url = format!(
            "{}/{}",
            bucket_url.trim_end_matches('/'),
            lotus_model::percent_encode(&file.filename)
        );
        self.send_with_retries(
            || {
                self.request(Method::PUT, &upload_url)
                    .header(CONTENT_TYPE, "application/octet-stream")
                    .header(CONTENT_LENGTH, len)
                    .body(bytes.clone())
            },
            "PUT bucket object",
            |resp, op| async move {
                ensure_success(resp, op).await?;
                Ok(())
            },
        )
        .await
    }

    async fn publish_and_wait(&self, id: DepositionId) -> anyhow::Result<Deposition> {
        let url = format!("{}/deposit/depositions/{id}/actions/publish", self.base_url);
        let response = self
            .request(Method::POST, &url)
            .send()
            .await
            .context("POST actions/publish")?;
        let initial = parse_action_deposition(response, || self.get_deposition(id)).await?;
        if initial.submitted {
            return Ok(initial);
        }
        for _ in 0..self.max_polls {
            tokio::time::sleep(self.poll_interval).await;
            let current = self.get_deposition(id).await?;
            if current.submitted {
                return Ok(current);
            }
        }
        bail!("timed out waiting for Zenodo deposition {id} to finish publishing");
    }
}

pub(super) fn parse_record_id_from_url(url: &str) -> Option<u64> {
    let (_, tail) = url.rsplit_once("/records/")?;
    let digits = tail.split(['/', '?', '#']).next()?;
    digits.parse::<u64>().ok()
}
