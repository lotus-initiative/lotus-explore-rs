// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Handler and artifact helpers for `lotus archive`.

use std::process::ExitCode;

use crate::args::ArchiveArgs;
use crate::clock::{now_iso_utc, parse_iso_utc};
use crate::zenodo;

/// Validate `SOURCE_CODE_URL` for the source-code related identifier Zenodo
/// expects: an `https` URL with a `/commit/<40-hex-SHA>` suffix.
///
/// Returns the canonical string or a clap-style error ready for `anyhow`.
pub fn validate_source_code_url(url: &str) -> anyhow::Result<String> {
    let Some(rest) = url.strip_prefix("https://") else {
        anyhow::bail!("SOURCE_CODE_URL must be an https URL");
    };
    let authority = rest.split('/').next().unwrap_or("");
    let host = authority.split(':').next().unwrap_or("");
    if authority.is_empty() || host.is_empty() || authority.chars().any(char::is_whitespace) {
        anyhow::bail!("SOURCE_CODE_URL must include a non-empty host");
    }
    let Some((_, revision)) = url.rsplit_once("/commit/") else {
        anyhow::bail!("SOURCE_CODE_URL must have a /commit/<sha> suffix");
    };
    if revision.len() != 40 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        anyhow::bail!("SOURCE_CODE_URL revision must be a 40-character commit SHA");
    }
    Ok(url.to_string())
}

/// Build the Zenodo `DepositMetadata` for a LOTUS archive.
pub fn zenodo_metadata(
    title: &str,
    creator: &str,
    publication_date: lotus_model::utc::UtcDateTime,
    source_code_url: &str,
) -> zenodo::DepositMetadata {
    zenodo::DepositMetadata {
        title: title.to_string(),
        upload_type: "dataset",
        publication_date: publication_date.format_date(),
        description_html:
            "<p>InChIKey-deduplicated Wikidata SMILES for chemical structures with P703 \
             (found in taxon), fetched from QLever. Isomeric P2017 values take \
             precedence over canonical P233 values. The archive includes source \
             statements and query provenance.</p>"
                .to_string(),
        creators: vec![zenodo::Creator {
            name: creator.to_string(),
        }],
        access_right: "open",
        license: "cc-by-4.0",
        keywords: vec![
            "LOTUS".to_string(),
            "Wikidata".to_string(),
            "SMILES".to_string(),
        ],
        related_identifiers: vec![zenodo::RelatedIdentifier {
            identifier: source_code_url.to_string(),
            relation: "isDerivedFrom",
            scheme: "url",
            resource_type: "software",
        }],
        communities: vec![zenodo::Community {
            identifier: "the-lotus-initiative",
        }],
    }
}

/// Run the Wikidata archive: fetch, deduplicate, package as tar.gz, and
/// optionally publish to Zenodo. By default only writes files to disk; real
/// publishing requires `--publish` and `ZENODO_TOKEN`, and `--dry-run`
/// validates the plan without contacting Zenodo.
pub async fn archive(args: ArchiveArgs) -> anyhow::Result<ExitCode> {
    let now = args.now.unwrap_or_else(now_iso_utc);
    let fetched_at = parse_iso_utc(&now)?;

    let http = lotus_search::reqwest_client::ReqwestClient::new()?;
    let endpoint = lotus_search::Endpoint::new(lotus_search::Service::Qlever);

    let filename = archive_filename(fetched_at);
    let (artifacts, summary) = Box::pin(lotus_export::archive::run_archive(
        &http,
        &lotus_export::archive::ArchiveOptions {
            endpoint,
            fetched_at: &now,
            archive_filename: &filename,
        },
    ))
    .await
    .map_err(|e| anyhow::anyhow!("archive failed: {e}"))?;

    let (archive_path, manifest_path, checksums_path) =
        write_archive_files(args.output.as_ref(), &filename, &artifacts)?;

    eprintln!(
        "lotus: wrote archive ({} source, {} deduplicated, {} missing-key, {} parse errors) to {}",
        summary.source_records,
        summary.records,
        summary.missing_inchi_key_records,
        summary.parse_errors,
        archive_path.display(),
    );

    if args.publish {
        let root_id = root_deposition_id()?;
        let title = format!("LOTUS Wikidata SMILES — {}", fetched_at.format_date());
        if args.dry_run {
            // Validate the metadata + upload plan without contacting Zenodo.
            let _metadata =
                zenodo_metadata(&title, &args.creator, fetched_at, &source_code_url_env()?);
            eprintln!(
                "{}",
                dry_run_zenodo(&archive_path, &manifest_path, &checksums_path, root_id)?
            );
        } else {
            Box::pin(publish_to_zenodo(
                &title,
                &args.creator,
                fetched_at,
                root_id,
                &archive_path,
                &manifest_path,
                &checksums_path,
            ))
            .await?;
        }
    }

    Ok(ExitCode::SUCCESS)
}

/// The archive's file name, stamped with the fetch time so successive Zenodo
/// versions do not share one name.
pub fn archive_filename(fetched_at: lotus_model::utc::UtcDateTime) -> String {
    format!(
        "lotus-wikidata-smiles-{}.tar.gz",
        fetched_at.format_compact_utc()
    )
}

/// Write the archive tar.gz, manifest JSON, and SHA256SUMS file to `output_dir`.
///
/// `filename` must be the name the archive's checksum line was computed under.
pub fn write_archive_files(
    output: Option<&std::path::PathBuf>,
    filename: &str,
    artifacts: &lotus_export::archive::ArchiveArtifacts,
) -> anyhow::Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> {
    let output_dir: &std::path::Path =
        output.map_or_else(|| std::path::Path::new("."), |p| p.as_path());
    std::fs::create_dir_all(output_dir)?;

    let archive_path = output_dir.join(filename);
    let manifest_path = output_dir.join("lotus-wikidata-smiles.manifest.json");
    let checksums_path = output_dir.join("lotus-wikidata-smiles.SHA256SUMS");

    std::fs::write(&archive_path, &artifacts.archive)?;
    std::fs::write(&manifest_path, &artifacts.manifest)?;
    std::fs::write(&checksums_path, &artifacts.checksums)?;

    Ok((archive_path, manifest_path, checksums_path))
}

/// Resolve the root deposition ID from `ZENODO_ROOT_DEPOSITION_ID`, if set.
fn root_deposition_id() -> anyhow::Result<Option<zenodo::DepositionId>> {
    root_deposition_id_from(|key| std::env::var(key))
}

pub fn root_deposition_id_from(
    mut get_var: impl FnMut(&str) -> Result<String, std::env::VarError>,
) -> anyhow::Result<Option<zenodo::DepositionId>> {
    match get_var("ZENODO_ROOT_DEPOSITION_ID") {
        Ok(value) => parse_deposition_id(&value),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => anyhow::bail!("cannot read ZENODO_ROOT_DEPOSITION_ID: {e}"),
    }
}

/// Parse a raw string as a Zenodo root deposition ID.
pub fn parse_deposition_id(value: &str) -> anyhow::Result<Option<zenodo::DepositionId>> {
    let id: u64 = value
        .parse()
        .map_err(|_| anyhow::anyhow!("ZENODO_ROOT_DEPOSITION_ID is not a valid deposition ID"))?;
    if id == 0 {
        anyhow::bail!("ZENODO_ROOT_DEPOSITION_ID must be non-zero");
    }
    Ok(Some(zenodo::DepositionId(id)))
}

/// Validate the Zenodo metadata + upload plan without writing to disk or
/// contacting Zenodo, and return the line to report.
///
/// Returns the report rather than printing it, so the plan it states is
/// checkable without capturing stderr.
pub fn dry_run_zenodo(
    archive_path: &std::path::Path,
    manifest_path: &std::path::Path,
    checksums_path: &std::path::Path,
    root_id: Option<zenodo::DepositionId>,
) -> anyhow::Result<String> {
    // An error, not a message: a dry run that cannot build its plan has found
    // the problem it exists to find, and a script checking the exit status must
    // see it.
    let files = archive_upload_paths(archive_path, manifest_path, checksums_path)
        .map_err(|e| anyhow::anyhow!("Zenodo dry run failed: upload spec: {e}"))?;
    let plan = root_id.map_or_else(
        || "publish the initial Zenodo record".to_string(),
        |root_id| format!("publish a new version from deposition {}", root_id.0),
    );
    Ok(format!(
        "lotus: Zenodo dry run OK — metadata valid, {} files ready, plan: {plan}",
        files.len()
    ))
}

/// Actually publish the archive to Zenodo. Tries the sandbox token first, then
/// falls back to the production token.
pub async fn publish_to_zenodo(
    title: &str,
    creator: &str,
    publication_date: lotus_model::utc::UtcDateTime,
    root_id: Option<zenodo::DepositionId>,
    archive_path: &std::path::Path,
    manifest_path: &std::path::Path,
    checksums_path: &std::path::Path,
) -> anyhow::Result<()> {
    let source_code_url = source_code_url_env()?;
    let metadata = zenodo_metadata(title, creator, publication_date, &source_code_url);
    let upload_specs = archive_upload_paths(archive_path, manifest_path, checksums_path)?;

    let (client, target_label) = zenodo::ZenodoClient::from_env_or_sandbox()?;
    eprintln!("lotus: publishing to {target_label}");
    let published = Box::pin(client.publish_dataset(root_id, &metadata, &upload_specs)).await?;

    eprintln!(
        "lotus: published to Zenodo, record ID: {}",
        published.record_id.0
    );
    Ok(())
}

/// Read and validate `SOURCE_CODE_URL` for the source-code related identifier
/// Zenodo expects: an `https` URL with a `/commit/<40-hex-SHA>` suffix.
fn source_code_url_env() -> anyhow::Result<String> {
    source_code_url_from(|key| std::env::var(key))
}

pub fn source_code_url_from(
    mut get_var: impl FnMut(&str) -> Result<String, std::env::VarError>,
) -> anyhow::Result<String> {
    let url =
        get_var("SOURCE_CODE_URL").map_err(|_| anyhow::anyhow!("SOURCE_CODE_URL is not set"))?;
    validate_source_code_url(&url)
}

/// Build `UploadFile`s from three file paths for the Zenodo upload.
pub fn archive_upload_paths(
    archive: &std::path::Path,
    manifest: &std::path::Path,
    checksums: &std::path::Path,
) -> anyhow::Result<Vec<zenodo::UploadFile>> {
    let specs = zenodo::prepare_uploads([
        (
            archive.file_name().map_or_else(
                || "lotus-wikidata-smiles.tar.gz".to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            archive,
        ),
        (
            manifest.file_name().map_or_else(
                || "lotus-wikidata-smiles.manifest.json".to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            manifest,
        ),
        (
            checksums.file_name().map_or_else(
                || "lotus-wikidata-smiles.SHA256SUMS".to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            checksums,
        ),
    ])
    .map_err(|e| anyhow::anyhow!("prepare Zenodo upload specs: {e}"))?;
    Ok(specs)
}
