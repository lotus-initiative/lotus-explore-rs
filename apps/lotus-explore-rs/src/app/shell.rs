// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::bootstrap::{AppBootstrap, bootstrap_app};
use super::routes::{Route, normalize_empty_query};
use crate::app_state::AppState;
use crate::components::layout::footer::Footer;
use crate::components::layout::header_meta::HeaderMetaSection;
use crate::components::layout::notices::{ErrorNotice, ShareNotice, TaxonNotice};
use crate::components::layout::page_header::PageHeader;
use crate::components::layout::structured_data::StructuredDataHead;
use crate::components::results_viewport::ResultsViewport;
use crate::components::welcome::SearchExamples;
use crate::features::explore::{
    ExploreInteractions, ExploreState, SearchTaskController, build_shareable_url,
    initial_url_state, is_true_flag, use_download_dispatch_effect, use_startup_effect,
};
use crate::hooks::LocaleProvider;
use crate::i18n::{Locale, TextKey, t};
use crate::models::SearchCriteria;
use crate::services::AppServices;
use crate::state::{
    AppStateContext, FormCriteriaContext, ResultsContext, use_app_state_context,
    use_form_criteria_context, use_results_context,
};
use crate::ui::a11y_contract::{MAIN_PANEL_ID, PAGE_TITLE_ID, SKIP_TO_RESULTS_HREF};
use dioxus::prelude::*;
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[allow(clippy::missing_const_for_fn)]
fn resolve_startup_dark_mode(startup: &crate::features::explore::InitialUrlState) -> bool {
    if startup.dark_mode {
        return true;
    }

    #[cfg(target_arch = "wasm32")]
    let mut startup_dark_mode = false;
    #[cfg(not(target_arch = "wasm32"))]
    let startup_dark_mode = false;

    #[cfg(target_arch = "wasm32")]
    {
        // Check localStorage first for persisted user preference
        if let Some(win) = web_sys::window()
            && let Ok(storage) = js_sys::Reflect::get(&win, &"localStorage".into())
            && !storage.is_undefined()
            && let Ok(func) = js_sys::Reflect::get(&storage, &"getItem".into())
            && let Some(get_item) = func.dyn_ref::<js_sys::Function>()
            && let Ok(value) = get_item.call1(&storage, &"dark_mode".into())
            && let Some(value_str) = value.as_string()
        {
            startup_dark_mode = value_str == "true";
        }

        let params = crate::features::explore::url_state::read_url_query_params();
        if let Some(raw) = params.get("dark_mode") {
            return crate::features::explore::url_state::is_true_flag(raw);
        }

        if let Some(win) = web_sys::window()
            && let Ok(media) = win.match_media("(prefers-color-scheme: dark)")
            && let Some(media_query) = media
        {
            return media_query.matches();
        }
    }

    startup_dark_mode
}

#[component]
pub fn AppRoot() -> Element {
    normalize_empty_query();
    let startup_url_state = initial_url_state();
    let startup_dark_mode = resolve_startup_dark_mode(&startup_url_state);
    let AppBootstrap {
        app_state: initial_app_state,
        criteria: initial_criteria,
        criteria_baseline: initial_criteria_baseline,
        locale: initial_locale,
        explore: initial_explore,
    } = bootstrap_app(startup_url_state);

    let app_state: Signal<AppState> = use_signal(move || AppState {
        dark_mode: startup_dark_mode,
        ..initial_app_state
    });
    let criteria: Signal<SearchCriteria> = use_signal(move || initial_criteria);
    let criteria_baseline: Signal<SearchCriteria> = use_signal(move || initial_criteria_baseline);
    let locale: Signal<Locale> = use_signal(move || initial_locale);
    let explore: Signal<ExploreState> = use_signal(move || initial_explore);

    let services = use_context_provider(AppServices::new);
    let repo = services.repository();
    let _app_state_ctx = use_context_provider(move || AppStateContext::new(app_state));
    let form_ctx =
        use_context_provider(move || FormCriteriaContext::new(criteria, criteria_baseline));
    let _results_ctx = use_context_provider(move || ResultsContext::new(explore));
    let search_task_controller = use_context_provider(SearchTaskController::new);
    let _explore_interactions = use_context_provider({
        let tc = search_task_controller;
        move || ExploreInteractions::new(criteria, form_ctx, explore, tc, repo)
    });

    rsx! {
        LocaleProvider { locale,
            Router::<Route> {}
        }
    }
}

#[component]
fn AppRuntimeEffects() -> Element {
    let mut locale = crate::hooks::use_locale_signal();
    let mut app_state = use_app_state_context().state;
    let explore = use_results_context().explore;
    let criteria = use_form_criteria_context().criteria;
    let search_task_controller = use_context::<SearchTaskController>();
    let repo = use_context::<AppServices>().repository();

    use_effect(move || {
        normalize_empty_query();
        let route = router().current::<Route>();
        let query = route.query_value();
        if let Some(lang) = query.get("lang") {
            let next_locale = Locale::detect(lang);
            if *locale.read() != next_locale {
                *locale.write() = next_locale;
            }
        }
        if let Some(dark_mode) = query.get("dark_mode") {
            let next_dark_mode = is_true_flag(dark_mode);
            if app_state.read().dark_mode != next_dark_mode {
                app_state.with_mut(|state| state.dark_mode = next_dark_mode);
            }
        }
    });

    // Sync <html lang> and <html data-theme> in a single effect to batch DOM
    // reads (document_element) and writes (set_attribute) — avoids interleaved
    // read-write-reflow patterns that cause forced synchronous layout.
    use_effect(move || {
        #[cfg(target_arch = "wasm32")]
        {
            let lang = locale.read().lang_code();
            let dark_mode = app_state.read().dark_mode;
            if let Some(doc) = web_sys::window().and_then(|w| w.document())
                && let Some(html) = doc.document_element()
            {
                let _ = html.set_attribute("lang", lang);
                let _ = html.set_attribute("data-theme", if dark_mode { "dark" } else { "light" });
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (locale.read(), app_state.read().dark_mode);
        }
    });

    #[cfg(target_arch = "wasm32")]
    use_effect(|| {
        if let Some(doc) = web_sys::window().and_then(|window| window.document())
            && let Ok(Some(shell)) = doc.query_selector("[data-boot-shell]")
        {
            shell.remove();
        }
    });

    use_startup_effect(app_state, explore, criteria, search_task_controller, repo);
    use_download_dispatch_effect(app_state, explore);

    rsx! {}
}

#[component]
pub fn AppShell() -> Element {
    let locale = crate::hooks::use_locale();
    rsx! {
        AppRuntimeEffects {}
        a {
            href: SKIP_TO_RESULTS_HREF,
            class: "skip-link",
            "{t(locale, TextKey::SkipToResults)}"
        }
        div {
            class: "app-shell bg-shell-page",
            div {
                class: "app-layout",
                main {
                    id: MAIN_PANEL_ID,
                     class: "main-content min-w-0 min-h-0 w-full max-w-none",

                    tabindex: "-1",
                    aria_labelledby: PAGE_TITLE_ID,
                    PageHeader {}
                    Outlet::<Route> {}
                }
            }
            footer {
                 class: "flex flex-col shrink-0 w-full bg-shell-chrome min-h-[80px]",

                div {
                    class: "w-full max-w-[1600px] mx-auto px-5 pt-[3px] pb-[6px] box-border lg:px-8",
                    Footer {}
                }
            }
        }
    }
}

#[component]
pub(crate) fn ExplorePage() -> Element {
    let locale = crate::hooks::use_locale();
    let criteria = use_form_criteria_context().criteria;
    let searched_once = use_results_context().explore.read().lifecycle.searched_once;
    let shareable_url =
        use_memo(move || build_shareable_url(&criteria.read()).map(Arc::<str>::from));

    rsx! {
        section {
            class: "page-section w-full max-w-none px-4 sm:px-6 lg:px-8",
            h2 { class: "sr-only", id: "search-page-heading", "{t(locale, TextKey::Search)}" }
            div { class: "w-full rounded-xl border border-shell-border bg-shell-raised overflow-hidden",
                div { class: "page-body flex min-h-0 flex-col gap-4 px-4 pb-4 pt-5 sm:px-6 sm:pb-6 sm:pt-6",
                    TaxonNotice {}
                    ErrorNotice {}
                    SearchPanelInline {}
                    SearchExamples {}
                    if searched_once {
                        ShareNotice { shareable_url }
                        HeaderMetaSection {}
                    }
                    // Outside the `searched_once` branch: the component renders
                    // nothing without a result set, and gating it here would
                    // also hide it during the first search.
                    StructuredDataHead {}
                    ResultsViewport {}
                }
            }
        }
    }
}

#[component]
fn SearchPanelInline() -> Element {
    crate::components::search_panel::SearchPanel()
}
