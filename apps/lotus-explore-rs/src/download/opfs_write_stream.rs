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
