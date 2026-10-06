// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

const SOURCE: &str = include_str!("../scroll_runtime.rs");

/// A queued frame's closure must survive until that frame has fired.
///
/// The browser keeps its own handle after `requestAnimationFrame` returns, and
/// dropping the closure zeroes the index that handle points at. The next frame
/// then throws "closure invoked recursively or after being dropped".
///
/// The single-slot version of this passed every test and still threw, because the
/// slot was only written when the caller believed nothing was outstanding -- a
/// per-instance belief that cannot see another instance's frame.
#[test]
fn the_frame_closure_is_reclaimed_only_after_its_frame_fired() {
    assert!(
        SOURCE.contains("done.get()"),
        "a slot entry must be reclaimed only once its frame has fired; reclaiming \
         on the caller's bookkeeping drops a frame the browser still holds"
    );
}

/// One slot is not enough, and only the replacing-table case proves it.
///
/// Two instances share the slot exactly when a search swaps one results table for
/// another, which is the only situation that made this throw.
#[test]
fn the_frame_closure_slot_is_not_single_valued() {
    // Spelled on the slot's type, not its name, so renaming the static cannot
    // quietly disarm this. Single-valued as this was when the bug was live.
    let single = concat!("RefCell<Option<", "RafClosure>>");
    assert!(
        !SOURCE.contains(single),
        "a single-valued slot lets an incoming results table evict the outgoing \
         table's closure while its frame is still queued"
    );
    assert!(
        SOURCE.contains("Vec<(Rc<Cell<bool>>"),
        "outstanding closures are kept in a list so a replacement cannot evict a \
         frame that has not fired"
    );
}

/// A frame marks itself fired before it touches a signal.
///
/// The flag lives outside the slot precisely so this works, and the ordering is
/// what makes a late frame safe: the writes below can fail against a dead scope,
/// and anything which returns early past the mark would strand the entry.
#[test]
fn a_frame_marks_itself_fired_before_it_writes_anything() {
    let mark = SOURCE.find("fired_in_cb.set(true)");
    let write = SOURCE.find("try_write_if_changed(first_visible_row_sig");
    assert!(mark.is_some(), "the callback must mark its own frame fired");
    assert!(
        write.is_some(),
        "the callback must record the row it measured"
    );
    assert!(
        mark < write,
        "the fired mark must come first: the signal writes below it can find a \
         dropped scope, and a frame which skipped the mark could never be reclaimed"
    );
}

/// A late frame records nothing; it must not unwind a signal whose scope is gone.
///
/// The closure deliberately outlives the scope that queued it, which is the whole
/// point of parking it here. That is only safe because every access it makes is
/// fallible -- otherwise stopping the throw just replaces it with a panic on the
/// very next frame, which is how the first attempt at this failed.
#[test]
fn a_late_frame_writes_nothing_and_does_not_panic() {
    let unguarded_write = format!("first_visible_row_sig.{}", "write()");
    assert!(
        !SOURCE.contains(&unguarded_write),
        "a frame arriving after its scope was dropped must not call write(), which \
         unwraps internally and takes the page down"
    );
    let unguarded_peek = format!("first_visible_row_sig.{}", "peek()");
    assert!(
        !SOURCE.contains(&unguarded_peek),
        "a frame arriving after its scope was dropped must not call peek(), which \
         also unwraps"
    );
    assert!(
        SOURCE.contains("try_peek"),
        "the frame reads its signals through the fallible accessor"
    );
}
