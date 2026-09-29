// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Curation-page UI sections: share bar, status notice, add-row, table.

use crate::components::ui::Button;
use crate::curation::{CurationInputRow, QuickStatementsBundle};
use crate::features::curation::services::quickstatements::build_qs_dev_link;
use crate::hooks::use_add_row_form::AddRowForm;
use crate::i18n::{
    Locale, TextKey, button_add_row, button_append_tsv_rows, button_generate_quickstatements,
    button_generating, button_load_example_rows, button_remove, button_second_pass, col_action,
    col_name, curation_qs_dev_label, curation_qs_dev_main_hint, curation_qs_dev_prereq_hint,
    heading_add_one_row, heading_queued_rows, heading_quickstatements,
    heading_quickstatements_dependencies, heading_tsv_import, hint_expected_tsv_headers,
    msg_delay_advice, msg_two_step_hint, placeholder_doi_optional, placeholder_molecule_name,
    placeholder_taxon_optional, t,
};
use crate::ui::prelude::{NoticeBar, NoticeTone};
use crate::upload::{extract_blob_from_file_data, read_blob_string};
use dioxus::prelude::*;
use std::sync::Arc;

use crate::components::copy_button::CopyButton;
use crate::features::explore::absolute_share_url;

#[component]
pub fn ShareBar(locale: Locale, share: Arc<str>) -> Element {
    rsx! {
        div { class: "flex flex-col gap-2 p-3 rounded-xl border border-shell-border bg-shell-raised", role: "status",
            span { class: "text-ui font-semibold text-text2", "{t(locale, TextKey::Share)}" }
            div { class: "flex flex-col gap-2",
                input {
                    id: "curation-share-url-field",
                    name: "share_url",
                    autocomplete: "off",
                    aria_label: "{t(locale, TextKey::CopyShareableLink)}",
                    class: "w-full font-mono rounded-xl border border-border bg-surface px-3 py-2 text-body text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    r#type: "text",
                    readonly: true,
                    value: "{share}",
                }
                CopyButton {
                    text: Arc::<str>::from(absolute_share_url(&share)),
                    title: t(locale, TextKey::CopyShareableLink),
                    locale,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use crate::curation::CurationInputRow;

    #[test]
    fn queue_rows_removal_logic_preserves_other_rows() {
        let mut rows = vec![
            CurationInputRow {
                name: "A".to_string(),
                smiles: "CCO".to_string(),
                taxon: None,
                doi: None,
            },
            CurationInputRow {
                name: "B".to_string(),
                smiles: "CCN".to_string(),
                taxon: None,
                doi: None,
            },
            CurationInputRow {
                name: "C".to_string(),
                smiles: "CCC".to_string(),
                taxon: None,
                doi: None,
            },
        ];
        rows.remove(1);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "A");
        assert_eq!(rows[1].name, "C");
    }
}

#[component]
pub fn StatusNotice(locale: Locale, message: Arc<str>) -> Element {
    rsx! {
        // The shared NoticeBar: the hand-rolled copy this replaced had drifted
        // out of its min-height reserve and its badge weight.
        NoticeBar {
            label: t(locale, TextKey::Notice).to_string(),
            tone: NoticeTone::Warning,
            role: "status",
            aria_live: "polite",
            span { class: "flex-1 min-w-0 text-ui text-warning break-words leading-snug", "{message}" }
        }
    }
}

#[component]
pub fn AddRowCard(
    locale: Locale,
    form: AddRowForm,
    processing: bool,
    on_add_row: EventHandler<()>,
    on_load_examples: EventHandler<()>,
) -> Element {
    let schema = r#"{"type":"object","properties":{"name":{"type":"string","description":"Compound name"},"smiles":{"type":"string","description":"SMILES representation"},"taxon":{"type":"string","description":"Taxon name or identifier"},"doi":{"type":"string","description":"Optional DOI"}},"additionalProperties":true}"#;

    rsx! {
        form {
            id: "lotus-curation-add-row-form",
            // WebMCP declarative registration; see the note in search_panel.rs.
            // Deliberately no `toolautosubmit`: this form changes the curation
            // queue, so the user should press the button.
            "toolname": "add_curation_row",
            "tooldescription": "Add a single curated chemical compound record with a name, SMILES, taxon, and DOI.",
            "data-webmcp-id": "lotus-curation-add-row-form",
            "data-webmcp-type": "form",
            "data-webmcp-name": "LOTUS curation add-row form",
            "data-webmcp-description": "Add a single curated chemical compound record with a name, SMILES, taxon, and DOI.",
            "data-webmcp-schema": "{schema}",
            "data-mcp-id": "lotus-curation-add-row-form",
            "data-mcp-type": "form",
            "data-mcp-name": "LOTUS curation add-row form",
            "data-mcp-description": "Add a single curated chemical compound record with a name, SMILES, taxon, and DOI.",
            "data-mcp-schema": "{schema}",
            onsubmit: move |evt: Event<FormData>| {
                evt.prevent_default();
                on_add_row.call(());
            },
            class: "flex flex-col gap-4 rounded-xl border border-border bg-panel-soft p-4",
            h3 { "{heading_add_one_row(locale)}" }
            div { class: "grid grid-cols-1 gap-3",
                label { class: "form-label", r#for: "curation-name-input",
                    "{placeholder_molecule_name(locale)}"
                }
                input {
                    id: "curation-name-input",
                    name: "name",
                    "toolparamdescription": "Compound name.",
                    autocomplete: "off",
                    class: "form-input w-full rounded-xl border border-border bg-surface px-3 py-2 text-body text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    r#type: "text",
                    placeholder: "{placeholder_molecule_name(locale)}",
                    value: "{form.name}",
                    oninput: move |e| form.name.set(e.value()),
                }
                label {
                    class: "form-label",
                    r#for: "curation-smiles-input",
                    "SMILES"
                }
                input {
                    id: "curation-smiles-input",
                    name: "smiles",
                    "toolparamdescription": "SMILES representation of the structure.",
                    autocomplete: "off",
                    class: "form-input w-full rounded-xl border border-border bg-surface px-3 py-2 text-body text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    r#type: "text",
                    placeholder: "SMILES",
                    value: "{form.smiles}",
                    oninput: move |e| form.smiles.set(e.value()),
                }
                label {
                    class: "form-label",
                    r#for: "curation-taxon-input",
                    "{placeholder_taxon_optional(locale)}"
                }
                input {
                    id: "curation-taxon-input",
                    name: "taxon",
                    "toolparamdescription": "Taxon name or identifier the compound was reported for.",
                    autocomplete: "off",
                    class: "form-input w-full rounded-xl border border-border bg-surface px-3 py-2 text-body text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    r#type: "text",
                    placeholder: "{placeholder_taxon_optional(locale)}",
                    value: "{form.taxon}",
                    oninput: move |e| form.taxon.set(e.value()),
                }
                label { class: "form-label", r#for: "curation-doi-input",
                    "{placeholder_doi_optional(locale)}"
                }
                input {
                    id: "curation-doi-input",
                    name: "doi",
                    "toolparamdescription": "Optional DOI of the source publication.",
                    autocomplete: "off",
                    class: "form-input w-full rounded-xl border border-border bg-surface px-3 py-2 text-body text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    r#type: "text",
                    placeholder: "{placeholder_doi_optional(locale)}",
                    value: "{form.doi}",
                    oninput: move |e| form.doi.set(e.value()),
                }
            }
            div { class: "flex flex-wrap items-center gap-2.5",
                Button {
                    label: button_add_row(locale).to_string(),
                    class: "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl bg-accent text-bg font-semibold shadow-xs hover:bg-accent-2 active:bg-accent-2 min-h-[34px] gap-1.5 px-3 py-1.5 text-ui active:scale-[0.98]",
                    onclick: Some(EventHandler::new(move |_: Event<MouseData>| on_add_row.call(()))),
                }
                Button {
                    label: button_load_example_rows(locale).to_string(),
                    class: "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-[34px] gap-1.5 px-3 py-1.5 text-ui active:scale-[0.98] disabled:opacity-100 disabled:bg-panel-soft disabled:text-muted disabled:cursor-not-allowed",
                    disabled: processing,
                    onclick: Some(EventHandler::new(move |_: Event<MouseData>| on_load_examples.call(()))),
                }
            }
        }
    }
}

#[component]
pub fn TsvImportCard(
    locale: Locale,
    tsv_input: Signal<String>,
    processing: bool,
    has_tsv_input: bool,
    on_parse_tsv: EventHandler<()>,
    on_import_uploaded_tsv: EventHandler<String>,
    on_import_error: EventHandler<String>,
) -> Element {
    let tsv_schema = r#"{"type":"object","properties":{"tsv":{"type":"string","description":"TSV rows with name, SMILES, taxon, and DOI columns"}},"additionalProperties":true}"#;

    rsx! {
        form {
            id: "lotus-curation-tsv-form",
            // No `toolautosubmit` here either, for the same reason.
            "toolname": "import_curation_tsv",
            "tooldescription": "Paste or upload a TSV file of curated compound rows to import into the queue.",
            "data-webmcp-id": "lotus-curation-tsv-form",
            "data-webmcp-type": "form",
            "data-webmcp-name": "LOTUS TSV import form",
            "data-webmcp-description": "Paste or upload a TSV file of curated compound rows to import into the queue.",
            "data-webmcp-schema": "{tsv_schema}",
            "data-mcp-id": "lotus-curation-tsv-form",
            "data-mcp-type": "form",
            "data-mcp-name": "LOTUS TSV import form",
            "data-mcp-description": "Paste or upload a TSV file of curated compound rows to import into the queue.",
            "data-mcp-schema": "{tsv_schema}",
            onsubmit: move |evt: Event<FormData>| {
                evt.prevent_default();
                if has_tsv_input {
                    on_parse_tsv.call(());
                }
            },
            class: "flex flex-col gap-4 rounded-xl border border-border bg-panel-soft p-4",
            h3 { "{heading_tsv_import(locale)}" }
            p { class: "text-ui text-subtle leading-snug", "{hint_expected_tsv_headers(locale)}" }
            label { class: "form-label", r#for: "curation-tsv-input", "TSV" }
            textarea {
                id: "curation-tsv-input",
                name: "tsv",
                "toolparamdescription": "TSV rows with name, SMILES, taxon, and DOI columns.",
                autocomplete: "off",
                class: "form-textarea mono w-full min-h-[130px] rounded-xl border border-border bg-surface p-2.5 font-mono text-body text-text shadow-xs focus:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                aria_describedby: "curation-tsv-hint",
                value: "{tsv_input}",
                oninput: move |e| tsv_input.set(e.value()),
            }
            p { id: "curation-tsv-hint", class: "sr-only",
                "{hint_expected_tsv_headers(locale)}"
            }
            div { class: "flex flex-wrap items-center gap-2.5",
                Button {
                    label: button_append_tsv_rows(locale).to_string(),
                    class: "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-[34px] gap-1.5 px-3 py-1.5 text-ui active:scale-[0.98] disabled:opacity-100 disabled:bg-panel-soft disabled:text-muted disabled:cursor-not-allowed",
                    disabled: processing || !has_tsv_input,
                    onclick: Some(EventHandler::new(move |_: Event<MouseData>| on_parse_tsv.call(()))),
                }
                // Visible <label for>: the old aria-label was hardcoded English
                // in a four-locale app and there was no visible label at all.
                label {
                    r#for: "curation-tsv-file-input",
                    class: "text-ui font-semibold text-muted",
                    "{t(locale, TextKey::TsvFileUpload)}"
                }
                input {
                    id: "curation-tsv-file-input",
                    name: "tsv_file",
                    "toolparamdescription": "Upload a TSV file instead of pasting. An agent cannot set a file input, so it should use the tsv property instead.",
                    autocomplete: "off",
                    class: "curation-file-input w-full max-w-full cursor-pointer rounded-xl border border-border bg-surface px-3 py-2 text-ui text-muted shadow-xs transition-colors hover:border-accent/50 hover:bg-bg focus:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 file:mr-3 file:cursor-pointer file:rounded-lg file:border-0 file:bg-accent file:px-3 file:py-1.5 file:text-ui file:font-semibold file:text-bg hover:file:bg-accent-2",
                    r#type: "file",
                    accept: ".tsv,text/tab-separated-values,text/plain",
                    disabled: processing,
                    onchange: move |evt| {
                        let files = evt.files();
                        let Some(file) = files.first().cloned() else {
                            return;
                        };
                        spawn(async move {
                            match extract_blob_from_file_data(&[file]) {
                                Ok(Some(extracted)) => {
                                    match read_blob_string(&extracted.blob).await {
                                        Ok(content) => on_import_uploaded_tsv.call(content),
                                        Err(err) => on_import_error.call(err.to_string()),
                                    }
                                }
                                Ok(None) => {}
                                Err(err) => on_import_error.call(err),
                            }
                        });
                    },
                }
            }
        }
    }
}

#[component]
pub fn QueueRowsCard(
    locale: Locale,
    rows: Signal<Vec<CurationInputRow>>,
    processing: bool,
    on_process: EventHandler<()>,
) -> Element {
    // `rows.read().clone()` on every render deep-copied the whole
    // `Vec<CurationInputRow>` — four `String`s plus three `Option<String>` per
    // row — each time the page re-rendered for an unrelated reason. The page
    // subscribes to `tsv_input` through `has_tsv_input()`, so every keystroke in
    // the TSV textarea re-rendered this card and paid for that copy.
    //
    // The memo reads the same signal and subscribes to nothing else, so the deep
    // copy happens once per actual change to `rows`. Wrapping it in an `Arc`
    // lets each render take a refcount bump instead of a copy, and drops the
    // read guard before the template is built — the same shape as
    // `prepared_state` in `results_table.rs` and `result_rows_memo` in
    // `data_curation_page.rs`.
    let rows_snapshot = use_memo(move || Arc::<[CurationInputRow]>::from(rows.read().clone()));
    let rows_snapshot = rows_snapshot.read().clone();

    rsx! {
        div { class: "flex flex-col gap-4 rounded-xl",
            div { class: "flex flex-wrap items-center justify-between gap-2.5",
                h3 { "{heading_queued_rows(locale)}" }
                Button {
                    label: if processing {
                        button_generating(locale).to_string()
                    } else {
                        button_generate_quickstatements(locale).to_string()
                    },
                    class: "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl bg-accent text-bg font-semibold shadow-xs hover:bg-accent-2 active:bg-accent-2 min-h-[34px] gap-1.5 px-3 py-1.5 text-ui active:scale-[0.98] disabled:opacity-100 disabled:bg-accent/30 disabled:text-bg/60 disabled:cursor-not-allowed",
                    disabled: processing,
                    onclick: Some(EventHandler::new(move |_: Event<MouseData>| on_process.call(()))),
                }
            }
            div {
                class: "w-full overflow-x-auto rounded-xl border border-shell-border focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                role: "region",
                tabindex: "0",
                aria_label: "{heading_queued_rows(locale)}",
                table {
                     class: "curation-queued-table w-full min-w-max table-auto border-collapse text-ui",

                    thead {
                        tr { class: "text-left",
                            th { scope: "col", class: "border-b border-panel-border bg-panel-soft px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[110px] min-w-[110px]", "{col_action(locale)}" }
                            th { scope: "col", class: "border-b border-panel-border bg-panel-soft px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted min-w-[3ch]", "#" }
                            th { scope: "col", class: "border-b border-panel-border bg-panel-soft px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[140px] min-w-[140px]", "{col_name(locale)}" }
                            th { scope: "col", class: "border-b border-panel-border bg-panel-soft px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted min-w-[220px]", "SMILES" }
                            th { scope: "col", class: "border-b border-panel-border bg-panel-soft px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[140px] min-w-[140px]", "{t(locale, TextKey::TaxonCol)}" }
                            th { scope: "col", class: "border-b border-panel-border bg-panel-soft px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[140px] min-w-[140px]", "DOI" }
                        }
                    }
                    tbody {
                        if rows_snapshot.is_empty() {
                            tr {
                                td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text w-[110px] min-w-[110px] font-mono text-micro", "-" }
                                td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text min-w-[3ch] font-mono text-micro", "-" }
                                td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text font-mono text-micro", "-" }
                                td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text min-w-[220px] font-mono text-micro", "-" }
                                td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text font-mono text-micro", "-" }
                                td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text font-mono text-micro", "-" }
                            }
                        } else {
                            for (idx, row) in rows_snapshot.iter().enumerate() {
                                tr { key: "{row.name}|{row.smiles}",
                                    class: "odd:bg-surface/30 hover:bg-surface/60",
                                    td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text w-[110px] min-w-[110px]",
                                        Button {
                                            label: button_remove(locale).to_string(),
                                            class: "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-danger/35 bg-danger/10 text-danger font-semibold hover:bg-danger/15 active:bg-danger/20 min-h-[34px] gap-1.5 px-3 py-1.5 text-ui active:scale-[0.98]",
                                            onclick: Some(EventHandler::new(move |_: Event<MouseData>| {
                                                let row_count = rows.read().len();
                                                if idx < row_count {
                                                    rows.write().remove(idx);
                                                }
                                            })),
                                        }
                                    }
                                    td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text min-w-[3ch] font-mono text-micro", "{idx + 1}" }
                                    td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text", "{row.name}" }
                                    td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text min-w-[220px]", "{row.smiles}" }
                                    td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text", "{row.taxon.as_deref().unwrap_or(\"\")}" }
                                    td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text font-mono text-micro", "{row.doi.as_deref().unwrap_or(\"\")}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn QuickStatementsCard(
    locale: Locale,
    quickstatements: Signal<QuickStatementsBundle>,
    awaiting_second_pass: bool,
    processing: bool,
    on_second_pass: EventHandler<()>,
) -> Element {
    let qs_ref = quickstatements.read();
    if qs_ref.dependencies.is_empty() && qs_ref.main.is_empty() {
        return rsx! {};
    }

    let qs_dependency_link = build_qs_dev_link(&qs_ref.dependencies);
    let qs_main_link = build_qs_dev_link(&qs_ref.main);

    rsx! {
        div { class: "flex flex-col gap-4 rounded-xl",
            if !qs_ref.dependencies.is_empty() {
                p { class: "text-ui text-subtle leading-snug", "{msg_two_step_hint(locale)}" }
                p { class: "text-ui text-subtle leading-snug", "{msg_delay_advice(locale)}" }
                p { class: "text-ui text-subtle leading-snug",
                    a {
                        href: "{qs_dependency_link}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "font-medium text-accent underline",
                        "{curation_qs_dev_label(locale)}"
                    }
                    " - {curation_qs_dev_prereq_hint(locale)}"
                }
                div { class: "flex flex-wrap items-center justify-between gap-2.5",
                    h3 { "{heading_quickstatements_dependencies(locale)}" }
                    CopyButton {
                        text: qs_ref.dependencies.clone(),
                        locale,
                    }
                }
                textarea {
                    id: "curation-quickstatements-dependencies",
                    name: "quickstatements_dependencies",
                    autocomplete: "off",
                    class: "form-textarea mono w-full min-h-[220px] rounded-xl border border-border bg-surface p-2.5 font-mono text-body text-text shadow-xs focus:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    aria_label: "{heading_quickstatements_dependencies(locale)}",
                    readonly: true,
                    value: "{qs_ref.dependencies}",
                }
                Button {
                    label: button_second_pass(locale).to_string(),
                    class: "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-[34px] gap-1.5 px-3 py-1.5 text-ui active:scale-[0.98] disabled:opacity-100 disabled:bg-panel-soft disabled:text-muted disabled:cursor-not-allowed",
                    disabled: processing,
                    onclick: Some(EventHandler::new(move |_: Event<MouseData>| on_second_pass.call(()))),
                }
            }

            if !awaiting_second_pass && !qs_ref.main.is_empty() {
                p { class: "text-ui text-subtle leading-snug",
                    a {
                        href: "{qs_main_link}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "font-medium text-accent underline",
                        "{curation_qs_dev_label(locale)}"
                    }
                    " - {curation_qs_dev_main_hint(locale)}"
                }
                div { class: "flex flex-wrap items-center justify-between gap-2.5",
                    h3 { "{heading_quickstatements(locale)}" }
                    CopyButton {
                        text: qs_ref.main.clone(),
                        locale,
                    }
                }
                textarea {
                    id: "curation-quickstatements-main",
                    name: "quickstatements_main",
                    autocomplete: "off",
                    class: "form-textarea mono w-full min-h-[220px] rounded-xl border border-border bg-surface p-2.5 font-mono text-body text-text shadow-xs focus:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    aria_label: "{heading_quickstatements(locale)}",
                    readonly: true,
                    value: "{qs_ref.main}",
                }
            }
        }
    }
}
