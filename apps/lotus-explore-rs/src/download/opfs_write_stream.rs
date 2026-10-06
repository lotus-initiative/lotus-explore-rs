// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

/// Read as text rather than tested in place, because this file is compiled for the
/// browser target only and its tests would never run on a host build. The gate is
/// worth having, so it lives where CI can see it.
const SINK: &str = include_str!("file_sink.rs");

/// `createWritable` is asynchronous, so the sink has to await what it returns.
///
/// Holding the promise and then calling `.write` on it finds nothing, so every
/// write fails with "the temporary export is not writable" -- on every browser,
/// including the ones that do support the sink. That made the in-memory blob path
/// look like the only path there is, which is why a large RDF export was refused on
/// a machine perfectly able to stream it to disk.
#[test]
fn the_writable_stream_is_awaited() {
    // Split so this assertion cannot match the needle it searches for.
    let create = "createWritable";
    let awaited = concat!(
        "JsFuture::from(pending",
        ".unchecked_into::<js_sys::Promise>())"
    );
    assert!(
        SINK.contains(create),
        "the sink opens its stream through the writable-file method"
    );
    assert!(
        SINK.contains(awaited),
        "the writable stream must be awaited; a promise has no `write` method, so \
         every write to it fails and the sink never works"
    );
}

/// A browser that cannot write to its own private storage is a capability, not an
/// error, and has to say so in words the reader can act on.
///
/// Safari before 17 has OPFS but not this method. Reaching the call with
/// `undefined` produced `TypeError: can't access property "call"`, which reads like
/// a defect in this code and sends whoever is looking at it hunting in the wrong
/// place.
#[test]
fn a_missing_capability_is_reported_as_a_capability() {
    let probe = concat!("create.is_", "function()");
    assert!(
        SINK.contains(probe),
        "the method must be probed before it is called, so an old browser gets a \
         reason rather than a TypeError"
    );
}

/// The export is a file, so it has to be asked for by the file-handle method.
///
/// This sink could never open, anywhere. `OpfsSink::open` requested the export
/// through `getDirectoryHandle`, which does not fail for a name that is not a
/// directory yet: it makes one. So the handle it got back had no
/// `createWritable`, and the probe above reported "this browser has private
/// storage but cannot write to it" -- a browser's fault, on a browser with
/// nothing wrong with it.
///
/// Only the browsers without a save picker ever reached the line, so Chromium
/// stayed green through two rounds of gates and Firefox was the first to be
/// refused an export it could have streamed to disk.
#[test]
fn the_export_is_requested_as_a_file_and_not_as_a_directory() {
    // Split so this assertion cannot match the needle it searches for.
    let as_file = concat!("file_handle(&exports, ", "filename");
    assert!(
        SINK.contains(as_file),
        "the export must be opened through the file-handle method"
    );
    let as_directory = concat!("directory_handle(&exports, ", "filename");
    assert!(
        !SINK.contains(as_directory),
        "the directory method succeeds for a name that is not a directory and \
         returns one, which cannot be written to; only browsers with no save \
         picker ever reached this, which is why it looked browser-specific"
    );
    let read_back = concat!("file_handle(&exports, &self.", "name");
    assert!(
        SINK.contains(read_back),
        "the finished export must be read back through the same file-handle \
         method it was written with"
    );
}

/// Reading the finished export back is asynchronous, and so is writing it.
///
/// `getFile()` returns a promise for the `File`. Handed that promise to
/// `createObjectURL` instead, the export is written to disk correctly and the
/// download then dies at its final step with "TypeError: Type error" -- so the
/// export is refused on every browser that reaches the sink, after doing all the
/// work. The sink is not selected until the fix above, which is why this waited
/// behind that one to be noticed.
#[test]
fn the_read_back_is_awaited_too() {
    // Keyed on the binding, not on `JsFuture::from(pending` -- which is the shape
    // `open` already uses for `createWritable`, so matching that would have this
    // gate pass on the broken source for the wrong reason. The read-back is the
    // only site binding `file`.
    let awaited = concat!("let file: JsValue = JsFuture::from(pending");
    assert!(
        SINK.contains(awaited),
        "`getFile()` returns a promise for the File; without awaiting it the whole \
         export fails at the last step with a TypeError, having already been written \
         to disk"
    );
}

/// The finished export must outlive the click that started its download.
///
/// Deleting the entry right after the click raced the download. An anchor click only
/// *starts* one -- the browser resolves the blob URL afterwards -- so a `removeEntry`
/// awaited at that point had usually already landed, and Firefox reported the file as
/// missing. Chromium read the blob eagerly enough to hide it, which is what made a
/// fourth defect in this one sink look like a Firefox limitation.
///
/// So the temporary is removed when the *next* export opens, which cannot race: the
/// browser is finished with the old file by the time there is a new one to write.
#[test]
fn the_finished_export_is_removed_by_the_next_one_and_not_by_the_click() {
    // Keyed on the call site rather than on the absence of the function: both exist,
    // and only the placement is the defect.
    let click_and_delete = concat!(
        "download_url(&url, filename)",
        ";\n\n        // The temporary goes either way"
    );
    assert!(
        !SINK.contains(click_and_delete),
        "removing the export in the same breath as the click races the download, \
         and the reader gets 'Firefox can't find the file at blob:...'"
    );
    let deferred = concat!("PENDING_EXPORT_NAME", ".with(|slot|");
    assert!(
        SINK.contains(deferred),
        "the finished export is held until the next one opens, rather than deleted \
         on a guess about when the browser has finished reading it"
    );
    let swept = concat!("discard_previous_export()", ".await");
    assert!(
        SINK.contains(swept),
        "the previous export is removed when a new export starts writing"
    );
}
