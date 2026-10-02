// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Search panel and its subsection components.
//!
//! The panel is three groups — compound, taxon, reference — not one input per
//! criteria field. Every filter here constrains one of the three things a
//! result row is about, and the four boxes this replaced were grouped by input
//! type instead, which is why a molecular mass and a publication year looked
//! like peers of a taxon name. See `form_sections::entity_group` for the rule
//! that decides what is always visible and what is collapsed.

use crate::components::form_sections::{
    EntityFilters, FieldExamples, FilterEntity, FormulaSection, MassRangeInput, TaxonInput,
    TaxonNomenclatureFilters, YearRangeInput,
};
use crate::features::explore::{FormAction, use_criteria_selector};

#[path = "search_panel/structure_model.rs"]
mod structure_model;

use crate::components::form_inputs::SearchButton;
use crate::features::explore::{use_explore_interactions, use_lifecycle_selector};
use crate::i18n::{TextKey, t, threshold_label};
use crate::state::{use_form_criteria_context, use_results_context};
use crate::ui::a11y_contract::SEARCH_PANEL_BODY_ID;
use dioxus::prelude::*;

/// Four structures, chosen to show the three shapes a structure field is usually
/// given: a straight chain, an aromatic ring, and a stereocentre. A stereocentre
/// is the one worth an example, because it is the case a person cannot type from
/// memory and would otherwise go looking for a tool to draw one.
const STRUCTURE_SUGGESTIONS: &[&str] = &["CC", "CCC", "c1ccccc1", "C[C@H](O)CO"];
use lotus_model::SmilesSearchType;
use lotus_model::classify_structure;

/// JSON Schema for search form autofill / MCP tooling introspection.
const SEARCH_SCHEMA: &str = r#"{"type":"object","properties":{"taxon":{"type":"string","description":"Taxon name, Wikidata QID, or * for all taxa"},"smiles":{"type":"string","description":"SMILES or Molfile input"},"mass_min":{"type":"number","description":"Minimum molecular mass in Da"},"mass_max":{"type":"number","description":"Maximum molecular mass in Da"},"year_min":{"type":"integer","description":"Minimum publication year"},"year_max":{"type":"integer","description":"Maximum publication year"},"formula_exact":{"type":"string","description":"Exact molecular formula filter, e.g. C7H5O5N"},"formula_enabled":{"type":"boolean","description":"Whether the formula filter is applied"},"stype":{"type":"string","enum":["substructure","similarity"],"description":"Structure search mode"}},"additionalProperties":true}"#;

pub fn SearchPanel() -> Element {
    let state = use_results_context();
    let form_ctx = use_form_criteria_context();
    let interactions = use_explore_interactions();
    let locale = crate::hooks::use_locale();

    let loading = *use_lifecycle_selector(state.explore, |lifecycle| lifecycle.loading).read();
    let is_dirty = form_ctx.is_dirty();
    let form_search = interactions;
    let button_search = form_search.clone();

    rsx! {
        form {
            id: "lotus-search-form",
            class: "flex-0-auto flex flex-col gap-2 rounded-xl border border-border bg-panel-soft p-3.5 w-full min-w-0 min-h-[300px]",
            aria_label: t(locale, TextKey::Search).to_string(),
            // WebMCP declarative tool registration. These four `tool*`
            // attributes are the ones a WebMCP-capable browser reads; it
            // synthesises the input schema from the named controls below and
            // `toolparamdescription` supplies each property's description.
            // The `data-mcp-*` hooks further down are a separate, non-standard
            // convention kept for tooling that predates WebMCP — they register
            // nothing on their own. See docs/DESIGN_SYSTEM.md.
            "toolname": "search_lotus",
            "tooldescription": "Search LOTUS compounds by taxon, structure, mass range, publication year, and formula.",
            "toolautosubmit": "true",
            "data-webmcp-id": "lotus-search-form",
            "data-webmcp-type": "form",
            "data-webmcp-name": "LOTUS search form",
            "data-webmcp-description": "Search compounds by taxon, SMILES or Molfile, mass range, publication year, and formula constraints.",
            "data-webmcp-schema": "{SEARCH_SCHEMA}",
            "data-mcp-id": "lotus-search-form",
            "data-mcp-type": "form",
            "data-mcp-name": "LOTUS search form",
            "data-mcp-description": "Search compounds by taxon, SMILES or Molfile, mass range, publication year, and formula constraints.",
            "data-mcp-schema": "{SEARCH_SCHEMA}",
            onsubmit: move |evt: Event<FormData>| {
                evt.prevent_default();
                form_search.search();
            },
            div {
                id: SEARCH_PANEL_BODY_ID,
                class: "search-panel-body grid grid-cols-1 items-start gap-3 md:grid-cols-2 lg:grid-cols-3",
                CompoundFilters {}
                TaxonFilters {}
                ReferenceFilters {}
            }

            SearchButton {
                loading,
                is_dirty,
                on_click: move |()| button_search.search(),
            }
        }
    }
}

/// Compound: the structure, plus everything that narrows a structure — how it
/// is matched, how heavy it is, and what it is made of.
#[component]
fn CompoundFilters() -> Element {
    let primary = rsx! { StructureInput {} };
    let advanced = rsx! {
        div { class: "flex flex-col gap-3",
            StructureSearchOptions {}
            MassRangeInput {}
            FormulaSection {}
        }
    };

    rsx! {
        EntityFilters {
            entity: FilterEntity::Compound,
            primary,
            advanced: Some(advanced),
        }
    }
}

/// Taxon: the name as typed, plus how far back in its naming history to look.
#[component]
fn TaxonFilters() -> Element {
    let primary = rsx! { TaxonInput {} };
    let advanced = rsx! { TaxonNomenclatureFilters {} };

    rsx! {
        EntityFilters {
            entity: FilterEntity::Taxon,
            primary,
            advanced: Some(advanced),
        }
    }
}

/// Reference: when it was published.
///
/// The only criterion a reference carries today, so this group has no advanced
/// disclosure at all. It is drawn anyway: the three groups are a set, and a
/// result row is always about all three things whether or not you constrain all
/// three.
#[component]
fn ReferenceFilters() -> Element {
    let primary = rsx! { YearRangeInput {} };

    rsx! {
        EntityFilters {
            entity: FilterEntity::Reference,
            primary,
            advanced: None,
        }
    }
}

/// The structure field: the compound group's primary filter, plus what kind of
/// input it currently holds.
#[component]
fn StructureInput() -> Element {
    let locale = crate::hooks::use_locale();
    let ctx = use_form_criteria_context();
    // Only the two fields this actually draws. The threshold is not one of them,
    // and subscribing to it would re-render the field on every tick of the
    // slider that lives in the advanced panel.
    let structure_fields = use_criteria_selector(ctx.criteria, |criteria| {
        (criteria.structure.clone(), criteria.structure_search)
    });
    let (smiles, smiles_search_type) = structure_fields.read().clone();
    let smiles_for_kind = smiles.clone();
    let kind = use_memo(move || classify_structure(&smiles_for_kind));
    let kind_value = *kind.read();
    let view_model = structure_model::build_structure_section_model(kind_value, smiles_search_type);

    rsx! {
        div { class: "flex min-w-0 flex-col gap-1.5",
            label {
                class: "text-body font-semibold text-text",
                r#for: "smiles-input",
                "{t(locale, TextKey::StructureSmilesOrMol)}"
            }
            textarea {
                id: "smiles-input",
                name: "smiles",
                "toolparamdescription": "SMILES or Molfile input.",
                autocomplete: "off",
                spellcheck: "false",
                placeholder: "{t(locale, TextKey::StructurePlaceholder)}",
                "aria-describedby": "smiles-input-examples-heading",
                value: "{smiles}",
                oninput: move |e| ctx.update(FormAction::Smiles(e.value())),
                rows: "2",
                class: "resize-field min-h-16 font-mono w-full rounded-xl border border-border bg-surface px-3 py-2 text-body text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
            }
            FieldExamples {
                target: "smiles-input",
                values: STRUCTURE_SUGGESTIONS,
                heading: TextKey::Examples,
                onfill: move |value: String| ctx.update(FormAction::Smiles(value)),
            }
            if let Some(note_key) = view_model.note_key {
                p { class: "flex flex-wrap items-center gap-2 text-micro text-subtle",
                    span {
                        class: "rounded-full bg-accent/10 px-1.5 py-0.5 font-semibold text-accent",
                        "{kind_value.label()}"
                    }
                    span { "{t(locale, note_key)}" }
                }
            }
        }
    }
}

/// How the structure is matched: contains a substructure, or is similar to it.
///
/// Advanced rather than primary because it changes what the structure field
/// *means* rather than narrowing it, and a search that never touches it gets
/// the documented default. The similarity threshold follows this control, so it
/// is here too rather than sitting in the compound group's primary slot.
#[component]
fn StructureSearchOptions() -> Element {
    let locale = crate::hooks::use_locale();
    let ctx = use_form_criteria_context();
    let structure_fields = use_criteria_selector(ctx.criteria, |criteria| {
        (
            criteria.structure.clone(),
            criteria.structure_search,
            criteria.structure_threshold,
        )
    });
    let (smiles, smiles_search_type, smiles_threshold) = structure_fields.read().clone();
    let kind = use_memo(move || classify_structure(&smiles.clone()));
    let view_model =
        structure_model::build_structure_section_model(*kind.read(), smiles_search_type);

    rsx! {
        fieldset { class: "m-0 flex flex-col gap-1.5 border-0 p-0",
            // On the fieldset, not on each radio: WebMCP describes a radio
            // group as one parameter, and repeating it on both controls
            // synthesises a conflicting property and fails schema validation.
            "toolparamdescription": "Structure search mode: substructure or similarity.",
            legend { class: "text-micro font-semibold uppercase tracking-wide text-subtle", "{t(locale, TextKey::StructureSearchMode)}" }
            label { class: "inline-flex items-center gap-1.5 text-ui text-muted",
                input {
                    r#type: "radio",
                    id: "smiles-search-type-substructure",
                    name: "stype",
                    autocomplete: "off",
                    class: "accent-accent h-4 w-4 cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    checked: smiles_search_type == SmilesSearchType::Substructure,
                    onchange: move |_| {
                        ctx.update(FormAction::SmilesSearchType(SmilesSearchType::Substructure));
                    },
                }
                "{t(locale, TextKey::Substructure)}"
            }
            label { class: "inline-flex items-center gap-1.5 text-ui text-muted",
                input {
                    r#type: "radio",
                    id: "smiles-search-type-similarity",
                    name: "stype",
                    autocomplete: "off",
                    class: "accent-accent h-4 w-4 cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    checked: smiles_search_type == SmilesSearchType::Similarity,
                    onchange: move |_| {
                        ctx.update(FormAction::SmilesSearchType(SmilesSearchType::Similarity));
                    },
                }
                "{t(locale, TextKey::Similarity)}"
            }
            if view_model.show_similarity_threshold {
                div { class: "flex flex-col gap-1",
                    label {
                        class: "text-micro font-semibold uppercase tracking-wide text-subtle",
                        r#for: "threshold-input",
                        "{threshold_label(locale, smiles_threshold)}"
                    }
                    input {
                        id: "threshold-input",
                        name: "smiles_threshold",
                        autocomplete: "off",
                        r#type: "range",
                        min: "0.0",
                        max: "1.0",
                        step: "0.01",
                        value: "{smiles_threshold}",
                        aria_valuemin: "0",
                        aria_valuemax: "1",
                        aria_valuenow: "{smiles_threshold}",
                        class: "w-full accent-accent cursor-pointer appearance-none h-2 bg-border rounded-xl focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<f64>() {
                                ctx.update(FormAction::SmilesThreshold(v));
                            }
                        },
                    }
                }
            }
        }
    }
}
