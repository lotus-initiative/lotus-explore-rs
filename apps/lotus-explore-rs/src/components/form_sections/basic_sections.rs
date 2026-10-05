// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::form_actions::FormAction;
use crate::features::explore::interactions::use_explore_interactions;
use crate::features::explore::selectors::use_criteria_selector;
use crate::i18n::{TextKey, t};
use crate::state::use_form_criteria_context;
use dioxus::prelude::*;
use lotus_model::SearchCriteria;
use lotus_model::taxon_nomenclature::{self, Relation};

use super::field_examples::FieldExamples;
use super::shared::{normalized_year_input_max, parse_f64_input, parse_u16_input};

/// Example taxa, as they would be typed. The first is a real QID rather than a
/// rank, because a rank is not a taxon the endpoint resolves: `Plantae` is a clade
/// with no compound, so it returns nothing and reads as a broken search.
pub(super) const TAXON_SUGGESTIONS: &[&str] =
    &["bitterwort", "Gentianales", "Q178265", "Plantae", "*"];

/// The four nomenclatural relationships a taxon search can follow.
///
/// An enum rather than a `(key, label)` tuple, because the key is also what
/// picks the criteria field and the form action, and matching on a string to
/// do that is a rename away from a silent no-op.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum NomenclatureToggle {
    /// Accepted name ↔ its synonyms (`P1420` / `P12763`).
    AcceptedSynonyms,
    /// New combination ↔ its basionym (`P566` / `P12766`).
    Basionyms,
    /// Current name ↔ its original combination, or protonym
    /// (`P1403` / `P12765`).
    Protonyms,
    /// Replacement name ↔ the name it replaced (`P694` / `P12764`).
    Replacements,
}

impl NomenclatureToggle {
    /// Every toggle, in the order the relationships are introduced in
    /// `lotus_model::taxon_nomenclature`: accepted/synonym first because it is
    /// the one most searches want, then the three chronological ones.
    pub(super) const ALL: [Self; 4] = [
        Self::AcceptedSynonyms,
        Self::Basionyms,
        Self::Protonyms,
        Self::Replacements,
    ];

    /// The value in a criteria.
    pub(super) fn read(self, criteria: &SearchCriteria) -> bool {
        criteria.taxon_names.for_relation(self.relation())
    }

    /// The model-side relationship this toggle drives.
    pub(super) const fn relation(self) -> &'static Relation {
        match self {
            Self::AcceptedSynonyms => &taxon_nomenclature::ACCEPTED_SYNONYM,
            Self::Basionyms => &taxon_nomenclature::BASIONYM,
            Self::Protonyms => &taxon_nomenclature::PROTONYM,
            Self::Replacements => &taxon_nomenclature::REPLACEMENT,
        }
    }

    /// The DOM `id`/`name` stem, which is also the query-parameter name.
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::AcceptedSynonyms => "accepted_synonyms",
            Self::Basionyms => "basionyms",
            Self::Protonyms => "protonyms",
            Self::Replacements => "replacements",
        }
    }

    /// The label, naming the relationship rather than describing the toggle —
    /// a user who knows the vocabulary should recognise it.
    pub(super) const fn label(self) -> TextKey {
        match self {
            Self::AcceptedSynonyms => TextKey::TaxonNomenclatureAccepted,
            Self::Basionyms => TextKey::TaxonNomenclatureBasionym,
            Self::Protonyms => TextKey::TaxonNomenclatureProtonym,
            Self::Replacements => TextKey::TaxonNomenclatureReplacement,
        }
    }

    /// The `toolparamdescription`, which is the place a user who does *not*
    /// know the vocabulary finds out what the toggle actually does.
    pub(super) const fn description(self) -> &'static str {
        match self {
            Self::AcceptedSynonyms => {
                "Also match the taxon's accepted name and its synonyms (Wikidata P1420)."
            }
            Self::Basionyms => {
                "Also match the basionym, the name the taxon was first described under (Wikidata P566)."
            }
            Self::Protonyms => {
                "Also match the original combination, the binomial as first published (Wikidata P1403)."
            }
            Self::Replacements => {
                "Also match a replacement name (nomen novum) and the name it replaced (Wikidata P694)."
            }
        }
    }
}

#[component]
pub fn TaxonInput() -> Element {
    let locale = crate::hooks::use_locale();
    let ctx = use_form_criteria_context();
    let interactions = use_explore_interactions();
    let taxon = use_criteria_selector(ctx.criteria, |c| c.taxon.clone());

    rsx! {
        div { class: "flex min-w-0 flex-col gap-1.5",
            label {
                class: "text-body font-semibold text-text",
                r#for: "taxon-input",
                "{t(locale, TextKey::TaxonField)}"
            }
            input {
                id: "taxon-input",
                name: "taxon",
                "toolparamdescription": "Taxon scientific name (P225), common name (P1843), Wikidata QID, or * for all taxa.",
                r#type: "text",
                autocomplete: "off",
                spellcheck: "false",
                placeholder: "{t(locale, TextKey::TaxonPlaceholder)}",
                value: "{taxon.read()}",
                // The examples are a described group of buttons, not a hint in the
                // placeholder, so the input points at them by name.
                "aria-describedby": "taxon-input-examples-heading",
                class: "resize-field w-full min-h-9 rounded-xl border border-border bg-surface px-3 py-2 text-body text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                list: "taxon-suggestions",
                oninput: move |e| ctx.update(FormAction::Taxon(e.value())),
                onkeydown: move |e| {
                    if e.key() == Key::Enter {
                        interactions.search();
                    }
                },
            }
            datalist { id: "taxon-suggestions",
                for item in TAXON_SUGGESTIONS {
                    option { value: "{item}" }
                }
            }
            FieldExamples {
                target: "taxon-input",
                values: TAXON_SUGGESTIONS,
                heading: TextKey::Examples,
                onfill: move |value: String| ctx.update(FormAction::Taxon(value)),
            }
        }
    }
}

/// The taxon's other names — the advanced half of the taxon group.
///
/// Four separate toggles, not one "include synonyms": the four are independent
/// nomenclatural relationships — a basionym is a rename with a chronology behind it,
/// an accepted name's synonym is not — so lumping them leaves no way to say which
/// was unticked.
///
/// They sit under a `role="group"` with a visible heading rather than as four loose
/// checkboxes, because the group is one decision ("how far back in this taxon's
/// naming history to look") that a screen reader user must hear before the first
/// option.
#[component]
pub fn TaxonNomenclatureFilters() -> Element {
    let locale = crate::hooks::use_locale();
    let ctx = use_form_criteria_context();
    // A selector per toggle rather than one selector for the set: each checkbox
    // re-renders on its own value changing, and a single selector holding all
    // four would re-render all of them on any one. Each reads through
    // `NomenclatureToggle::read`, so which field backs which checkbox is stated
    // once, here, rather than repeated in the closure and in the action.
    let nomenclature = NomenclatureToggle::ALL
        .map(|toggle| use_criteria_selector(ctx.criteria, move |c| toggle.read(c)));

    rsx! {
        div {
            role: "group",
            aria_labelledby: "taxon-nomenclature-heading",
            class: "flex flex-col gap-1",
            p {
                id: "taxon-nomenclature-heading",
                class: "text-micro font-semibold uppercase tracking-wide text-subtle",
                "{t(locale, TextKey::TaxonNomenclature)}"
            }
            for (toggle, is_on) in NomenclatureToggle::ALL.into_iter().zip(nomenclature) {
                label {
                    key: "{toggle.key()}",
                    class: "flex cursor-pointer items-start gap-1.5 text-ui text-muted",
                    input {
                        r#type: "checkbox",
                        id: "taxon-{toggle.key()}-input",
                        name: "taxon_{toggle.key()}",
                        "toolparamdescription": "{toggle.description()}",
                        autocomplete: "off",
                        class: "accent-accent mt-0.5 cursor-pointer",
                        checked: *is_on.read(),
                        onchange: move |e| ctx.update(FormAction::TaxonNomenclature { relation: toggle.relation(), on: e.checked() }),
                    }
                    span { "{t(locale, toggle.label())}" }
                }
            }
        }
    }
}

#[component]
pub fn MassRangeInput() -> Element {
    let locale = crate::hooks::use_locale();
    let ctx = use_form_criteria_context();
    let mass_range = use_criteria_selector(ctx.criteria, |c| (c.mass_min, c.mass_max));
    let (min_value, max_value) = *mass_range.read();

    rsx! {
        div {
            role: "group",
            aria_labelledby: "mass-range-label",
            class: "flex flex-col gap-1.5",
            p { id: "mass-range-label", class: "text-micro font-semibold uppercase tracking-wide text-subtle", "{t(locale, TextKey::MolecularMass)}" }
            div { class: "grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-end gap-2",
                div { class: "flex min-w-0 flex-col gap-0.5",
                    label {
                        class: "text-micro font-semibold uppercase tracking-wide text-subtle",
                        r#for: "mass-min",
                        "{t(locale, TextKey::Min)}"
                    }
                    input {
                        id: "mass-min",
                        name: "mass_min",
                        "toolparamdescription": "Minimum molecular mass in Da.",
                        r#type: "number",
                         autocomplete: "off",
                        min: "0",
                        max: "10000",
                        step: "1",
                        value: "{min_value}",
                        class: "resize-field w-full min-h-8 rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        oninput: move |e| {
                            if let Some(v) = parse_f64_input(&e.value()) {
                                ctx.update(FormAction::MassMin(v));
                            }
                        },
                    }
                }
                span { aria_hidden: "true", class: "pb-2 text-subtle", "-" }
                div { class: "flex min-w-0 flex-col gap-0.5",
                    label {
                        class: "text-micro font-semibold uppercase tracking-wide text-subtle",
                        r#for: "mass-max",
                        "{t(locale, TextKey::Max)}"
                    }
                    input {
                        id: "mass-max",
                        name: "mass_max",
                        "toolparamdescription": "Maximum molecular mass in Da.",
                        r#type: "number",
                        autocomplete: "off",
                        min: "0",
                        max: "10000",
                        step: "1",
                        value: "{max_value}",
                        class: "resize-field w-full min-h-8 rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        oninput: move |e| {
                            if let Some(v) = parse_f64_input(&e.value()) {
                                ctx.update(FormAction::MassMax(v));
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
pub fn YearRangeInput() -> Element {
    let locale = crate::hooks::use_locale();
    let ctx = use_form_criteria_context();
    let year_range = use_criteria_selector(ctx.criteria, |c| (c.year_min, c.year_max));
    let (min_value, max_value) = *year_range.read();
    let current = normalized_year_input_max(crate::clock::current_year());

    rsx! {
        div {
            role: "group",
            aria_labelledby: "year-range-label",
            class: "flex flex-col gap-1.5",
            p { id: "year-range-label", class: "text-micro font-semibold uppercase tracking-wide text-subtle", "{t(locale, TextKey::PublicationYear)}" }
            div { class: "grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-end gap-2",
                div { class: "flex min-w-0 flex-col gap-0.5",
                    label {
                        class: "text-micro font-semibold uppercase tracking-wide text-subtle",
                        r#for: "year-min",
                        "{t(locale, TextKey::YearFrom)}"
                    }
                    input {
                        id: "year-min",
                        name: "year_min",
                        "toolparamdescription": "Minimum publication year.",
                        r#type: "number",
                        autocomplete: "off",
                        min: "{lotus_model::YEAR_MIN}",
                        max: "{current}",
                        step: "1",
                        value: "{min_value}",
                        class: "resize-field w-full min-h-8 rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        oninput: move |e| {
                            if let Some(v) = parse_u16_input(&e.value()) {
                                ctx.update(FormAction::YearMin(v));
                            }
                        },
                    }
                }
                span { aria_hidden: "true", class: "pb-2 text-subtle", "-" }
                div { class: "flex min-w-0 flex-col gap-0.5",
                    label {
                        class: "text-micro font-semibold uppercase tracking-wide text-subtle",
                        r#for: "year-max",
                        "{t(locale, TextKey::YearTo)}"
                    }
                    input {
                        id: "year-max",
                        name: "year_max",
                        "toolparamdescription": "Maximum publication year.",
                        r#type: "number",
                        autocomplete: "off",
                        min: "{lotus_model::YEAR_MIN}",
                        max: "{current}",
                        step: "1",
                        value: "{max_value}",
                        class: "resize-field w-full min-h-8 rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        oninput: move |e| {
                            if let Some(v) = parse_u16_input(&e.value()) {
                                ctx.update(FormAction::YearMax(v));
                            }
                        },
                    }
                }
            }
        }
    }
}
