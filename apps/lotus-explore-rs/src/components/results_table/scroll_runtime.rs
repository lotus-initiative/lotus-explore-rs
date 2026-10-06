// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#[cfg(target_arch = "wasm32")]
use dioxus::prelude::*;
#[cfg(target_arch = "wasm32")]
use std::cell::{Cell, RefCell};
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

// The closure each queued frame will call, kept alive for the life of the page.
//
// A `Closure` handed to `requestAnimationFrame` must outlive the browser's handle
// to it. Dropping it zeroes its index, and the next frame the browser delivers
// throws "closure invoked recursively or after being dropped" -- the same bug, and
// the same deliberate leak, as the row-height measurement recorded in
// `virtualization_controller/measurement_frame_safety.rs`.
//
// Holding the closure in a `Signal` ties its life to the component, and searching
// again replaces the table: the scope dies while the frame is still queued. A
// process-wide slot outlives the scope, but a slot that keeps only the newest
// closure reintroduces the same failure one level up. The table being replaced and
// the table replacing it share that slot, while the guard which stops two frames
// from overlapping is per-instance and cannot see the two meet. So the incoming
// table zeroes the outgoing table's closure and the pending frame throws.
//
// The slot therefore keeps every outstanding closure and reclaims only the ones
// whose frame has already fired. Each callback shares a flag which lives outside
// the slot, so a frame can mark itself without borrowing the storage holding it and
// reclaiming is always safe. Nothing accumulates either: the guard admits one frame
// per instance at a time, so a steady stream of frames reuses a single entry.
#[cfg(target_arch = "wasm32")]
thread_local! {
    static PENDING_FRAME_CLOSURES: RefCell<Vec<(Rc<Cell<bool>>, RafClosure)>> =
        const { RefCell::new(Vec::new()) };
}

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
type RafIdSignal = Signal<Option<i32>>;
#[cfg(target_arch = "wasm32")]
type ScrollHostSignal = Signal<Option<web_sys::HtmlElement>>;

/// Park a closure the browser now holds a handle to.
///
/// Reclaims an entry whose frame has already fired, and grows only while every
/// entry is still outstanding. The single-slot version assumed that case away, and
/// assuming it is what dropped a queued frame on every search.
#[cfg(target_arch = "wasm32")]
fn retain_frame_closure(fired: Rc<Cell<bool>>, raf_cb: RafClosure) {
    PENDING_FRAME_CLOSURES.with(|slot| {
        let mut slot = slot.borrow_mut();
        match slot.iter_mut().find(|(done, _)| done.get()) {
            Some(reclaimable) => *reclaimable = (fired, raf_cb),
            None => slot.push((fired, raf_cb)),
        }
    });
}

/// Record `next`, unless the signal is gone or already holds it.
///
/// A frame outlives the scope that queued it: searching again replaces the table
/// and the browser still delivers the frame. `peek` and `write` unwrap internally,
/// so a late frame has to record nothing rather than take the page down.
#[cfg(target_arch = "wasm32")]
fn try_write_if_changed<T: Copy + PartialEq + 'static>(mut sig: Signal<T>, next: T) {
    let differs = match sig.try_peek() {
        Ok(current) => *current != next,
        Err(_) => return,
    };
    if differs && let Ok(mut slot) = sig.try_write() {
        *slot = next;
    }
}

/// Bundled scroll-RAF state, reducing `schedule_virtual_scroll_frame`'s arity.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
pub(super) struct ScrollFrameState {
    pub scroll_host: ScrollHostSignal,
    pub raf_scheduled: Signal<bool>,
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

    let first_visible_row_sig = first_visible_row;
    let viewport_height_px_sig = viewport_height_px;
    let mut scroll_raf_scheduled_sig = scroll_raf_scheduled;
    let mut scroll_raf_id_sig = scroll_raf_id;
    let div_for_raf = div;
    let fired = Rc::new(Cell::new(false));
    let fired_in_cb = Rc::clone(&fired);
    let raf_cb = wasm_bindgen::closure::Closure::wrap(Box::new(move |_ts: f64| {
        // Mark first, and unconditionally. Reclaiming the slot depends on it, and a
        // signal write below that finds a dropped scope must not strand the entry.
        fired_in_cb.set(true);
        let top = usize::try_from(div_for_raf.scroll_top().max(0)).unwrap_or(0);
        let height = usize::try_from(div_for_raf.client_height().max(0)).unwrap_or(0);
        let next_first = next_first_visible_row(top, row_height_px, total_rows);
        try_write_if_changed(first_visible_row_sig, next_first);
        if height > 0 {
            try_write_if_changed(viewport_height_px_sig, height);
        }
        if let Ok(mut slot) = scroll_raf_id_sig.try_write() {
            *slot = None;
        }
        if let Ok(mut slot) = scroll_raf_scheduled_sig.try_write() {
            *slot = false;
        }
    }) as Box<dyn FnMut(f64)>);

    let scheduled_id: Option<i32> = web_sys::window().and_then(|win| {
        win.request_animation_frame(raf_cb.as_ref().unchecked_ref())
            .ok()
    });
    if let Some(id) = scheduled_id {
        // The browser holds the handle from here on, so the closure has to be kept.
        retain_frame_closure(fired, raf_cb);
        *scroll_raf_id.write() = Some(id);
    } else {
        // Nothing was queued, so nothing holds the handle and letting the closure go
        // now is safe.
        *scroll_raf_id.write() = None;
        *scroll_raf_scheduled.write() = false;
    }
}

#[cfg(test)]
#[path = "scroll_runtime/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "scroll_runtime/frame_closure_safety.rs"]
mod frame_closure_safety;
