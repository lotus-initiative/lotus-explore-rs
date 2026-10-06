// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

const SOURCE: &str = include_str!("../virtualization_controller.rs");

/// A frame must not outlive its closure.
///
/// The measurement is queued with `request_animation_frame`, and the browser keeps a
/// handle to call. Holding the closure in a component-owned signal looks like the tidy
/// answer -- it ties the closure's lifetime to the component's. It is the wrong
/// answer: unmounting the table drops the closure while the queued frame survives,
/// and every frame afterwards throws "closure invoked recursively or after being
/// dropped". The closure is forgotten on purpose.
///
/// This gate exists because the opposite was tried, and it replaced one panic with
/// another.
#[test]
fn the_measurement_closure_is_never_dropped_while_a_frame_is_queued() {
    // Split so this assertion cannot match the needle it searches for.
    let leaked = concat!("outer.", "forget()");
    assert!(
        SOURCE.contains(leaked),
        "the outer measurement closure must be forgotten: the browser holds a handle \
         to it after the request, so dropping it turns every later frame into a throw"
    );
    // The obvious alternative, and the one this gate stops someone reinstating. Keyed
    // on the signal name rather than the closure, so a fix which also cancels the
    // frame on unmount can land without tripping here.
    let owned = concat!("measure_raf", "_cb");
    assert!(
        !SOURCE.contains(owned),
        "owning the closure without cancelling the queued frame produces 'closure \
         invoked recursively or after being dropped' on the next search"
    );
}

/// A forgotten closure outlives the signal it writes, so the write cannot panic.
///
/// This is the other half, and it is what the first fix got wrong in the other
/// direction. Searching again replaces the results table, the old scope is dropped,
/// and the frame still arrives. `set` unwraps internally, so recording a row height
/// for a table that no longer exists takes the app down. A late frame is dropped.
#[test]
fn the_measurement_write_survives_a_dropped_signal() {
    let try_write = concat!("row_height_px.try_", "write()");
    assert!(
        SOURCE.contains(try_write),
        "the measurement write must be fallible: a frame in flight when the table \
         unmounts arrives after the signal is gone"
    );
    let unguarded = format!("row_height_px.{}set(", "set");
    assert!(
        !SOURCE.contains(&unguarded),
        "an unguarded write on the measurement signal is the panic this replaced"
    );
}
