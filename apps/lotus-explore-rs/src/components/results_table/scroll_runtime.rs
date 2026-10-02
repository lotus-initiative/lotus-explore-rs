// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#[cfg(target_arch = "wasm32")]
use dioxus::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(any(target_arch = "wasm32", test))]
#[must_use]
pub(super) fn next_first_visible_row(
    scroll_top_px: usize,
    row_height_px: usize,
    total_rows: usize,
) -> usize {
    if row_height_px == 0 {
        return 0;
    }
    (scroll_top_px / row_height_px).min(total_rows)
}

#[cfg(any(target_arch = "wasm32", test))]
#[must_use]
pub(super) fn resolve_sampled_row_height_px(
    sampled_total_px: usize,
    sampled_count: usize,
    fallback_row_height_px: usize,
) -> usize {
    let fallback = fallback_row_height_px.max(1);
    if sampled_count == 0 {
        return fallback;
    }
    let avg = sampled_total_px / sampled_count;
    avg.max(1)
}

#[cfg(target_arch = "wasm32")]
pub(super) fn measure_row_height_px(
    scroll_id: &'static str,
    fallback_row_height_px: usize,
) -> usize {
    let Some(win) = web_sys::window() else {
        return fallback_row_height_px.max(1);
    };
    let Some(document) = win.document() else {
        return fallback_row_height_px.max(1);
    };
    let Some(node) = document.get_element_by_id(scroll_id) else {
        return fallback_row_height_px.max(1);
    };
    let Ok(scroll_host) = node.dyn_into::<web_sys::HtmlElement>() else {
        return fallback_row_height_px.max(1);
    };

    let rows = scroll_host.get_elements_by_class_name("data-row");
    let mut sampled_total_px = 0usize;
    let mut sampled_count = 0usize;

    for i in 0..rows.length() {
        let Some(row) = rows.item(i) else {
            continue;
        };
        let Ok(row) = row.dyn_into::<web_sys::HtmlElement>() else {
            continue;
        };
        let h = usize::try_from(row.offset_height().max(0)).unwrap_or(0);
        if h > 0 {
            sampled_total_px = sampled_total_px.saturating_add(h);
            sampled_count = sampled_count.saturating_add(1);
        }
    }

    resolve_sampled_row_height_px(sampled_total_px, sampled_count, fallback_row_height_px)
}

#[cfg(target_arch = "wasm32")]
pub(super) type RafClosure = wasm_bindgen::closure::Closure<dyn FnMut(f64)>;
#[cfg(target_arch = "wasm32")]
type RafSignal = Signal<Option<RafClosure>>;
#[cfg(target_arch = "wasm32")]
type RafIdSignal = Signal<Option<i32>>;
#[cfg(target_arch = "wasm32")]
type ScrollHostSignal = Signal<Option<web_sys::HtmlElement>>;

/// Bundled scroll-RAF state, reducing `schedule_virtual_scroll_frame`'s arity.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
pub(super) struct ScrollFrameState {
    pub scroll_host: ScrollHostSignal,
    pub raf_scheduled: Signal<bool>,
    pub raf_cb: RafSignal,
    pub raf_id: RafIdSignal,
}

#[cfg(target_arch = "wasm32")]
// A frame is one measurement: the state, the element, the row geometry and the
// handle all belong to the same request, and grouping them into a struct would
// only add a type to rebuild at the one call site that has the parts.
pub(super) fn schedule_virtual_scroll_frame(
    frame: ScrollFrameState,
    scroll_id: &'static str,
    row_height_px: usize,
    total_rows: usize,
    first_visible_row: Signal<usize>,
    viewport_height_px: Signal<usize>,
) {
    let ScrollFrameState {
        mut scroll_host,
        raf_scheduled: mut scroll_raf_scheduled,
        raf_cb: mut scroll_raf_cb,
        raf_id: mut scroll_raf_id,
    } = frame;
    let div = if let Some(existing) = scroll_host.peek().as_ref() {
        existing.clone()
    } else {
        let Some(win) = web_sys::window() else {
            return;
        };
        let Some(document) = win.document() else {
            return;
        };
        let Some(node) = document.get_element_by_id(scroll_id) else {
            return;
        };
        let Ok(found) = node.dyn_into::<web_sys::HtmlElement>() else {
            return;
        };
        *scroll_host.write() = Some(found.clone());
        found
    };

    if *scroll_raf_scheduled.peek() {
        return;
    }
    *scroll_raf_scheduled.write() = true;

    let mut first_visible_row_sig = first_visible_row;
    let mut viewport_height_px_sig = viewport_height_px;
    let mut scroll_raf_scheduled_sig = scroll_raf_scheduled;
    let mut scroll_raf_cb_sig = scroll_raf_cb;
    let mut scroll_raf_id_sig = scroll_raf_id;
    let div_for_raf = div;
    let raf_cb = wasm_bindgen::closure::Closure::wrap(Box::new(move |_ts: f64| {
        let top = usize::try_from(div_for_raf.scroll_top().max(0)).unwrap_or(0);
        let height = usize::try_from(div_for_raf.client_height().max(0)).unwrap_or(0);
        let next_first = next_first_visible_row(top, row_height_px, total_rows);
        if next_first != *first_visible_row_sig.peek() {
            *first_visible_row_sig.write() = next_first;
        }
        if height > 0 && height != *viewport_height_px_sig.peek() {
            *viewport_height_px_sig.write() = height;
        }
        *scroll_raf_id_sig.write() = None;
        *scroll_raf_scheduled_sig.write() = false;
        *scroll_raf_cb_sig.write() = None;
    }) as Box<dyn FnMut(f64)>);

    *scroll_raf_cb.write() = Some(raf_cb);
    let scheduled_id: Option<i32> = web_sys::window().and_then(|win| {
        scroll_raf_cb.peek().as_ref().and_then(|cb| {
            win.request_animation_frame(cb.as_ref().unchecked_ref())
                .ok()
        })
    });
    if let Some(id) = scheduled_id {
        *scroll_raf_id.write() = Some(id);
    } else {
        *scroll_raf_id.write() = None;
        *scroll_raf_scheduled.write() = false;
        *scroll_raf_cb.write() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_top_maps_to_first_visible_row() {
        assert_eq!(next_first_visible_row(0, 100, 50), 0);
        assert_eq!(next_first_visible_row(250, 100, 50), 2);
        assert_eq!(next_first_visible_row(990, 100, 50), 9);
    }

    #[test]
    fn next_first_visible_row_clamps_to_total_rows() {
        assert_eq!(next_first_visible_row(50_000, 100, 7), 7);
    }

    #[test]
    fn zero_row_height_is_safe() {
        assert_eq!(next_first_visible_row(50, 0, 7), 0);
    }

    #[test]
    fn sampled_row_height_falls_back_when_no_rows_are_measured() {
        assert_eq!(resolve_sampled_row_height_px(0, 0, 114), 114);
        assert_eq!(resolve_sampled_row_height_px(0, 0, 0), 1);
    }

    #[test]
    fn sampled_row_height_uses_upward_biased_average() {
        assert_eq!(resolve_sampled_row_height_px(520, 4, 114), 130);
    }

    #[test]
    fn sampled_row_height_never_drops_below_fallback() {
        assert_eq!(resolve_sampled_row_height_px(0, 4, 114), 1);
    }
}
