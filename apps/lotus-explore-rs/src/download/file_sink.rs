// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Writing a large export straight to disk, without holding it in memory.
//!
//! # Why this exists
//!
//! The export is streamed out of the result set in chunks, and then every chunk was
//! collected into a `Vec` before being handed to `Blob`. The streaming was real and
//! useless: the peak was still the whole file in JavaScript memory, sitting next to a
//! result set that was already most of the tab's budget. At three million rows that is
//! roughly 600 MB of CSV against roughly 254 MB of set, and the tab dies — the wasm
//! module cannot allocate, the exception surfaces as a blank page, and the browser
//! reloads it. Eighty per cent of that fits; all of it does not. The limit was never
//! the exporter's chunking, it was the assembly at the end.
//!
//! [`Blob`] cannot fix this. `new Blob(parts)` does not copy the parts, but the parts
//! are the problem: they are the entire file, already resident.
//!
//! # What does fix it
//!
//! Writing each chunk to a file as it is produced, through the File System Access API.
//! The browser owns the file; the tab hands over 256 KB at a time and drops it. Peak
//! memory is one chunk, whatever the size of the search.
//!
//! Two things about that API drive the shape of this module:
//!
//! - It is not everywhere. Firefox and Safari do not have it, so there is a fallback
//!   to the collecting path and this is an optimisation, not a requirement.
//! - `showSaveFilePicker` must be *called* during a user gesture. So it cannot be
//!   called from the export loop, which runs in a task `spawn`ed from the click and has
//!   therefore already lost the activation. It is called in the click handler and the
//!   resulting promise is armed here for the export to pick up. That is why this is a
//!   module-level slot rather than a parameter: threading a promise through four layers
//!   of component props to satisfy a browser timing rule is worse than a single
//!   explicitly-named handoff.
//!
//! Everything goes through `Reflect` rather than `web-sys` bindings, because
//! `web-sys` 0.3 does not expose `showSaveFilePicker` and adding a feature for it would
//! mean depending on a binding shape that is still moving.

use std::cell::RefCell;

use js_sys::Reflect;
use wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::JsFuture;

/// A `showSaveFilePicker()` promise, armed during the click and consumed by the export.
type Picker = js_sys::Promise;

thread_local! {
    /// Armed by [`arm_file_sink`] in the click handler.
    ///
    /// Thread-local because a browser tab has one event loop and the export that
    /// consumes this is the one the same click started. A download the user cancels
    /// leaves nothing armed, and the next click overwrites rather than appends.
    static PENDING_FILE_SINK: RefCell<Option<Picker>> = const { RefCell::new(None) };
}

thread_local! {
    /// The last export written through the private-storage sink, awaiting removal.
    ///
    /// Held rather than removed on a timer. An anchor click only *starts* a download:
    /// the browser resolves the URL afterwards, so anything deleted on a delay is
    /// deleted on a guess. Removing the previous export when the next one opens is the
    /// same cleanup with nothing left to race -- the browser is demonstrably finished
    /// with the old file by the time there is a new one to write.
    static PENDING_EXPORT_NAME: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Remove the export the previous download left behind, now that a new one is starting.
///
/// Best-effort by design: a failure here is a file the browser will reclaim under
/// storage pressure, and refusing the new export over it would trade a working
/// download for a stale temporary.
#[cfg(target_arch = "wasm32")]
async fn discard_previous_export() {
    let previous = PENDING_EXPORT_NAME.with(|slot| slot.borrow_mut().take());
    let Some(previous) = previous else {
        return;
    };
    let Ok(root) = opfs_root().await else {
        return;
    };
    let Ok(exports) = directory_handle(&root, EXPORTS_DIR, false).await else {
        return;
    };
    let _ = remove_entry(&exports, &previous).await;
}

/// Ask the browser where to write `suggested_name`, right now, while the click that
/// triggered the download is still a user gesture.
///
/// Returns nothing when the API is absent, which is the caller's signal to use the
/// Blob fallback. A user who *cancels* the picker is not an error: the export has not
/// started and nothing needs to be cleaned up.
pub fn arm_file_sink(suggested_name: &str) -> bool {
    let Some(promise) = show_save_file_picker(suggested_name) else {
        log::info!("event=download sink=file state=unavailable reason=no_file_system_access");
        return false;
    };
    PENDING_FILE_SINK.with(|slot| {
        if slot.borrow().is_some() {
            // An armed picker from an earlier click that never ran. Replacing it is
            // right: the earlier download is not going to happen, and holding its
            // promise would leak the handle.
            log::info!("event=download sink=file state=replaced reason=stale_pending_picker");
        }
        *slot.borrow_mut() = Some(promise);
    });
    true
}

/// Take the armed picker, if there is one. Consuming it keeps a second export from
/// reusing a handle the user already granted for a different file.
pub fn take_file_sink() -> Option<Picker> {
    PENDING_FILE_SINK.with(|slot| slot.borrow_mut().take())
}

/// Forget any armed picker without using it.
pub fn clear_file_sink() {
    PENDING_FILE_SINK.with(|slot| {
        slot.borrow_mut().take();
    });
}

/// `showSaveFilePicker({ suggestedName })`, or `None` where the API is absent.
fn show_save_file_picker(suggested_name: &str) -> Option<Picker> {
    let window = web_sys::window()?;
    // A missing method and a throwing one are the same thing to a caller: there is no
    // disk sink available. `showSaveFilePicker` also rejects in a cross-origin frame
    // and in a sandboxed one, but that arrives as a rejected promise, which the caller
    // handles as a fallback.
    let picker: js_sys::Function = Reflect::get(&window, &"showSaveFilePicker".into())
        .ok()?
        .unchecked_into();

    let options = js_sys::Object::new();
    // `suggestedName` is what the browser puts in the dialog's name field, so the
    // filename the reader was shown is the filename they are offered.
    Reflect::set(
        &options,
        &"suggestedName".into(),
        &wasm_bindgen::JsValue::from_str(suggested_name),
    )
    .ok()?;

    picker.call1(&window, &options).ok()?.dyn_into().ok()
}

/// An open file, once the picker has resolved and `createWritable` has returned.
///
/// Held open for the length of the export and closed explicitly: a writable left
/// unclosed is a partially-written file the user has to delete by hand.
pub struct FileSink {
    writable: JsValue,
}

impl FileSink {
    /// Resolve the picker and open the file for writing.
    ///
    /// The `await` belongs to the caller: this is sync glue around two promises, and
    /// making it `async` here would hide which of them can reject.
    pub async fn open(picker: Picker) -> Result<Self, String> {
        let handle = JsFuture::from(picker)
            .await
            .map_err(|error| describe_js_error("the save dialog", &error))?;

        let create: Result<JsValue, _> = Reflect::get(&handle, &"createWritable".into());
        let create = create
            .map_err(|_| "the browser exposed no way to write the chosen file".to_string())?;
        let create: js_sys::Function = create.unchecked_into();
        let writable = create
            .call0(&handle)
            .map_err(|error| describe_js_error("opening the chosen file", &error))?;

        Ok(Self { writable })
    }

    /// Append one chunk. Resolves when the bytes are handed over, not when they land on
    /// the platter — which is what keeps the producer from running ahead of the disk.
    pub async fn write(&self, chunk: &[u8]) -> Result<(), String> {
        let write: js_sys::Function = Reflect::get(&self.writable, &"write".into())
            .map_err(|_| "the open file is not writable".to_string())?
            .unchecked_into();
        let array = js_sys::Uint8Array::from(chunk);
        let written: js_sys::Promise = write
            .call1(&self.writable, &array)
            .map_err(|error| describe_js_error("writing to the chosen file", &error))?
            .unchecked_into();
        JsFuture::from(written)
            .await
            .map_err(|error| describe_js_error("writing to the chosen file", &error))?;
        Ok(())
    }

    /// Finish the file.
    ///
    /// Failing here is a real failure even though every byte was written: without the
    /// close the file is left truncated, so this must not be reported as success.
    pub async fn close(self) -> Result<(), String> {
        let close: js_sys::Function = Reflect::get(&self.writable, &"close".into())
            .map_err(|_| "the open file cannot be closed".to_string())?
            .unchecked_into();
        let closed: js_sys::Promise = close
            .call0(&self.writable)
            .map_err(|error| describe_js_error("closing the chosen file", &error))?
            .unchecked_into();
        JsFuture::from(closed)
            .await
            .map_err(|error| describe_js_error("closing the chosen file", &error))?;
        Ok(())
    }
}

/// A rejection worth putting in a log line.
fn describe_js_error(what: &str, error: &JsValue) -> String {
    // `AbortError` is the user pressing Escape in the save dialog. Worth naming,
    // because it is the one rejection that is entirely expected and not a defect.
    let name = Reflect::get(error, &"name".into())
        .ok()
        .and_then(|name| name.as_string())
        .unwrap_or_default();
    if name == "AbortError" {
        return "the save dialog was cancelled".to_string();
    }
    // An `ErrorEvent` stringifies to `"[object Error]"`, which tells nobody anything.
    // `message` is the part worth having; fall back to the name when there is none.
    let detail = Reflect::get(error, &"message".into())
        .ok()
        .and_then(|message| message.as_string())
        .unwrap_or_else(|| name.clone());
    if name.is_empty() {
        format!("{what} failed: {detail}")
    } else {
        format!("{what} failed: {name}: {detail}")
    }
}

//
// `showSaveFilePicker` is Chromium-only. Safari and Firefox have neither it nor any
// equivalent that can be called from a gesture, so on those the export still has to
// assemble itself in memory -- and that is the case that kills the tab.
//
// The Origin Private File System is the way out, and it needs nothing installed: Safari
// 15.2 and Firefox 111 both have `navigator.storage.getDirectory()`, and a file written
// through it lives on disk. `getFile()` then hands back a `File` -- which is a `Blob`
// that the browser streams from storage rather than one it holds in memory -- so the
// download the reader receives is read off disk.
//
// The trade is real and worth stating: OPFS is sandboxed storage, so the export has to
// be handed to an anchor afterwards and the temporary file deleted. The bytes still
// never sit in the tab's heap, which is the only thing that was failing.
//
// Used only when the File System Access API is absent. Where both exist the picker wins,
// because it lets the reader choose where the file goes and does not need a temp file.

/// Whether this browser can write to the Origin Private File System.
///
/// On `navigator.storage`, not on `window`. There is no `window.storage`, so
/// reading it found a missing property, and `Reflect::get` reports a missing
/// property as `Ok(undefined)` rather than as an error -- which made the old
/// chain `Ok(Reflect::get(undefined, "getDirectory")).is_ok()`, and that is
/// always true. So this claimed OPFS was available everywhere, and
/// [`OpfsSink::open`] then failed on the same wrong path for every export, on
/// every browser: the OPFS sink could never be selected and every download fell
/// through to the in-memory blob path.
///
/// Hence `is_function()` rather than `is_ok()`. Asking whether a property is
/// present by asking whether fetching it errored is the bug, and it is worth
/// spelling out at the one place it appeared.
#[must_use]
pub fn opfs_available() -> bool {
    web_sys::window().is_some_and(|window| {
        js_sys::Reflect::get(&window.navigator().storage(), &"getDirectory".into())
            .is_ok_and(|get_directory| get_directory.is_function())
    })
}

/// A file in the Origin Private File System, written chunk by chunk.
pub struct OpfsSink {
    writable: JsValue,
    /// The file name inside [`EXPORTS_DIR`], needed to delete it afterwards.
    name: String,
}

impl OpfsSink {
    /// Open a temporary file in the exports directory and keep it writable.
    pub async fn open(filename: &str) -> Result<Self, String> {
        // The previous export goes now, not on a timer: by this point the browser is
        // finished with it, and the origin's quota is what a large export runs into.
        discard_previous_export().await;

        let root = opfs_root().await?;
        let exports = directory_handle(&root, EXPORTS_DIR, true).await?;

        // Named exactly as the download is, so the `File` handed back by `getFile()`
        // already carries the name the reader was shown. A `.part` suffix here would
        // have to be stripped later for no benefit.
        //
        // An earlier version of this asked for the export through
        // `getDirectoryHandle`, which does not fail for a name that is not a
        // directory yet -- it makes one. Every browser that reached that code has a
        // *directory* sitting where its export file should be, and `getFileHandle`
        // refuses such a name with a `TypeMismatchError` rather than replacing it.
        // So the leftover is cleared first: the name is this export's own, and an
        // entry of the wrong kind under it is debris from our own bug rather than
        // anything of the reader's.
        let _ = remove_entry(&exports, filename).await;
        let handle = file_handle(&exports, filename, true).await?;

        // Probed, not assumed. `createWritable` is missing on Safari before 17, and a
        // missing method used to reach `.call0` as `undefined`, producing
        // `TypeError: can't access property "call"` -- which reads like a bug in this
        // code rather than a browser that cannot do it. There is already a capability
        // probe for `getDirectory` above; this is the second half of it, because a
        // browser can have OPFS and still not be able to write to a file in it.
        let create = Reflect::get(&handle, &"createWritable".into())
            .map_err(|_| "the export file cannot be opened for writing".to_string())?;
        if !create.is_function() {
            return Err("this browser has private storage but cannot write to it".to_string());
        }
        let create: js_sys::Function = create.unchecked_into();
        let pending = create
            .call0(&handle)
            .map_err(|error| describe_js_error("opening the temporary export", &error))?;

        // Awaited. `createWritable` is asynchronous and returns a promise for the
        // stream; holding the promise and then calling `.write` on it found nothing,
        // so this sink failed on every browser rather than only the old ones.
        let writable = JsFuture::from(pending.unchecked_into::<js_sys::Promise>())
            .await
            .map_err(|error| describe_js_error("opening the temporary export", &error))?;

        Ok(Self {
            writable,
            name: filename.to_string(),
        })
    }

    pub async fn write(&self, chunk: &[u8]) -> Result<(), String> {
        let write: js_sys::Function = Reflect::get(&self.writable, &"write".into())
            .map_err(|_| "the temporary export is not writable".to_string())?
            .unchecked_into();
        let array = js_sys::Uint8Array::from(chunk);
        let written: js_sys::Promise = write
            .call1(&self.writable, &array)
            .map_err(|error| describe_js_error("writing the temporary export", &error))?
            .unchecked_into();
        JsFuture::from(written)
            .await
            .map_err(|error| describe_js_error("writing the temporary export", &error))?;
        Ok(())
    }

    /// Flush and seal the entry.
    ///
    /// Takes `&self` rather than `self` because [`OpfsSink::hand_to_browser`] needs the
    /// name afterwards to read the file back and delete it.
    pub async fn close(&self) -> Result<(), String> {
        let close: js_sys::Function = Reflect::get(&self.writable, &"close".into())
            .map_err(|_| "the temporary export cannot be closed".to_string())?
            .unchecked_into();
        let closed: js_sys::Promise = close
            .call0(&self.writable)
            .map_err(|error| describe_js_error("closing the temporary export", &error))?
            .unchecked_into();
        JsFuture::from(closed)
            .await
            .map_err(|error| describe_js_error("closing the temporary export", &error))?;
        Ok(())
    }

    /// Hand the finished file to the browser as a download, then remove it.
    ///
    /// `getFile()` is the step that matters. It returns a `File` -- which is a `Blob`
    /// backed by the file on disk -- so the browser streams the download out of storage
    /// rather than out of the tab's heap. Handing it the accumulated chunks instead
    /// would put the whole export back in memory, which is the case this exists to
    /// survive.
    pub async fn hand_to_browser(&self, filename: &str, mime: &str) -> Result<(), String> {
        let root = opfs_root().await?;
        let exports = directory_handle(&root, EXPORTS_DIR, false).await?;
        let entry = file_handle(&exports, &self.name, false).await?;

        let get_file: js_sys::Function = Reflect::get(&entry, &"getFile".into())
            .map_err(|_| "the finished export cannot be read back".to_string())?
            .unchecked_into();
        let pending: JsValue = get_file
            .call0(&entry)
            .map_err(|error| describe_js_error("reading the finished export", &error))?;

        // Awaited. `getFile()` returns a promise for the `File`, and handing that
        // promise to `createObjectURL` throws "TypeError: Type error" -- which is the
        // whole download failing at its last step, once the export has already been
        // written to disk. `open` awaits `createWritable` for the same reason, and
        // forgetting it there made the sink unusable everywhere; this is the same
        // omission at the other end of it.
        let file: JsValue = JsFuture::from(pending.unchecked_into::<js_sys::Promise>())
            .await
            .map_err(|error| describe_js_error("reading the finished export", &error))?;

        // A `File` is a `Blob`, so the existing object-URL path takes it unchanged. The
        // MIME type is already on the `File` OPFS recorded, so `mime` is only a fallback
        // for the case where the entry has none.
        let _ = mime;
        let url = web_sys::Url::create_object_url_with_blob(
            &file.clone().unchecked_into::<web_sys::Blob>(),
        )
        .map_err(|error| describe_js_error("preparing the download", &error))?;

        let clicked = crate::upload::download_url(&url, filename);

        // Not revoked here. The object URL is a handle into this file, so revoking or
        // deleting the entry underneath it is what makes the browser report the file
        // as missing.
        if !clicked {
            // Nothing was started, so nothing will read it: this is the one case where
            // the temporary can go immediately.
            let _ = remove_entry(&exports, &self.name).await;
            return Err("the browser refused to start the download".to_string());
        }

        // The temporary goes, but only once the browser has actually taken the file.
        //
        // Deleting it here instead -- which is where it used to be -- raced the
        // download. An anchor click only *starts* one: Firefox resolves the URL
        // afterwards, by which time a `removeEntry` already awaited has made the blob
        // a dead handle, and the reader gets "Firefox can't find the file at blob:...".
        // Chromium reads the blob eagerly enough to hide the race, so it looked like
        // Firefox again, and it was this module's fourth consecutive defect to present
        // that way.
        //
        // A macrotask is enough: it is one turn of the event loop after the click, by
        // which time the download has taken its reference and holds it for its own
        // duration. Revoking the URL is left to the document, which drops it on
        // unload -- revoking it here would cancel a download still in flight.
        // The file stays until the next export, which is what makes this race-free
        // rather than merely slower to lose. Deleting it on a timer is still a guess
        // about when the browser has finished reading; deferring to the next export is
        // not, because by then the download has either completed or been abandoned,
        // and both are states the next export can observe without racing anything.
        PENDING_EXPORT_NAME.with(|slot| *slot.borrow_mut() = Some(self.name.clone()));
        Ok(())
    }
}

/// Subdirectory of the origin private file system that exports are written to.
const EXPORTS_DIR: &str = "lotus-exports";

/// `navigator.storage.getDirectory()`.
// The closure is what `clippy::redundant_closure_for_method_calls` asks to
// replace with `js_sys::JsValue::is_function`, and that does not compile: `JsValue`
// is a private re-export of `wasm_bindgen::JsValue`, so naming it through `js_sys`
// is `error[E0603]: struct JsValue is private`. The suggested fix is not a fix.
#[expect(
    clippy::redundant_closure_for_method_calls,
    reason = "the method path clippy suggests does not compile; JsValue is private in js_sys"
)]
async fn opfs_root() -> Result<JsValue, String> {
    let storage = web_sys::window()
        .map(|window| window.navigator().storage())
        .ok_or_else(|| "this browser exposes no origin private file system".to_string())?;
    let get_directory: js_sys::Function = Reflect::get(&storage, &"getDirectory".into())
        .ok()
        .filter(|value| value.is_function())
        .ok_or_else(|| "this browser cannot write to the origin private file system".to_string())?
        .unchecked_into();
    let root: JsValue = get_directory
        .call0(&storage)
        .map_err(|error| describe_js_error("opening private storage", &error))?;
    JsFuture::from(root.unchecked_into::<js_sys::Promise>())
        .await
        .map_err(|error| describe_js_error("opening private storage", &error))
}

/// `getDirectoryHandle(name, { create })` on `parent`.
async fn directory_handle(parent: &JsValue, name: &str, create: bool) -> Result<JsValue, String> {
    let get: js_sys::Function = Reflect::get(parent, &"getDirectoryHandle".into())
        .map_err(|_| "private storage does not support directories".to_string())?
        .unchecked_into();
    let options = js_sys::Object::new();
    Reflect::set(
        &options,
        &"create".into(),
        &wasm_bindgen::JsValue::from_bool(create),
    )
    .map_err(|_| "could not describe the private-storage request".to_string())?;
    let name_value = wasm_bindgen::JsValue::from_str(name);
    let handle: JsValue = get
        .call2(parent, &name_value, &options)
        .map_err(|error| describe_js_error("opening the exports directory", &error))?;
    JsFuture::from(handle.unchecked_into::<js_sys::Promise>())
        .await
        .map_err(|error| describe_js_error("opening the exports directory", &error))
}

/// `getFileHandle(name, { create })` on `parent`.
///
/// Deliberately not [`directory_handle`], and that is the whole point of the second
/// function. Asking the exports directory for the export by name through
/// `getDirectoryHandle` *succeeds*: it hands back a directory called `results.ttl`.
/// A directory handle has no `createWritable`, so the probe in [`OpfsSink::open`]
/// reported "this browser has private storage but cannot write to it" and no
/// browser could ever use the private-storage sink.
///
/// Chromium never found out, because a save picker answers first and this branch is
/// only reached by the browsers without one. Firefox reaching it turned a defect
/// that was everywhere into a defect that looked like a browser limitation, and the
/// reader paid for it in refused exports.
async fn file_handle(parent: &JsValue, name: &str, create: bool) -> Result<JsValue, String> {
    let get: js_sys::Function = Reflect::get(parent, &"getFileHandle".into())
        .map_err(|_| "private storage does not support files".to_string())?
        .unchecked_into();
    let options = js_sys::Object::new();
    Reflect::set(
        &options,
        &"create".into(),
        &wasm_bindgen::JsValue::from_bool(create),
    )
    .map_err(|_| "could not describe the private-storage request".to_string())?;
    let name_value = wasm_bindgen::JsValue::from_str(name);
    let handle: JsValue = get
        .call2(parent, &name_value, &options)
        .map_err(|error| describe_js_error("opening the export file", &error))?;
    JsFuture::from(handle.unchecked_into::<js_sys::Promise>())
        .await
        .map_err(|error| describe_js_error("opening the export file", &error))
}

/// `removeEntry(name, { recursive })`, ignoring a missing entry.
async fn remove_entry(parent: &JsValue, name: &str) -> Result<(), String> {
    let remove: js_sys::Function = match Reflect::get(parent, &"removeEntry".into()) {
        Ok(remove) => remove.unchecked_into(),
        Err(_) => return Ok(()),
    };
    let options = js_sys::Object::new();
    let name_value = wasm_bindgen::JsValue::from_str(name);
    let removed: JsValue = remove
        .call2(parent, &name_value, &options)
        .map_err(|error| describe_js_error("removing the temporary export", &error))?;
    // `removeEntry` resolves immediately; the promise is not awaited so a failure here
    // cannot turn a successful download into an error.
    let _ = JsFuture::from(removed.unchecked_into::<js_sys::Promise>()).await;
    Ok(())
}
