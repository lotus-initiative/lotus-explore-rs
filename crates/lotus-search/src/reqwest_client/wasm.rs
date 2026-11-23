// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The browser's streaming reader, macrotask yield, and visibility-aware
//! deadlines. Only compiled for wasm32, so no native test can observe it;
//! `.cargo/mutants.toml` excludes this file for that reason.

use super::{
    BodyChunks, CONNECTION_IDLE_TIMEOUT, ChunkFuture, DeadlineTracker, FetchError, RESUME_GRACE_MS,
    ReadProgress, ResponseBody, STREAM_STALL_TIMEOUT, YieldSchedule, duration_millis_u32,
};
use futures::{FutureExt as _, StreamExt as _};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;

fn now_epoch_ms() -> u64 {
    let raw = js_sys::Date::now();
    if raw.is_finite() && raw > 0.0 {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "Date::now() is checked finite and positive; integer milliseconds fit below 2^53"
        )]
        {
            raw as u64
        }
    } else {
        0
    }
}

fn document_hidden() -> bool {
    let global = js_sys::global();
    let Ok(doc) = js_sys::Reflect::get(&global, &JsValue::from_str("document")) else {
        return false;
    };
    if doc.is_undefined() || doc.is_null() {
        return false;
    }
    js_sys::Reflect::get(&doc, &JsValue::from_str("hidden"))
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

struct MacrotaskYield {
    port1: JsValue,
    port2: JsValue,
    rx: futures::channel::oneshot::Receiver<()>,
    _on_message: Closure<dyn FnMut(JsValue)>,
}

impl MacrotaskYield {
    fn try_new() -> Option<Self> {
        let global = js_sys::global();
        let ctor = js_sys::Reflect::get(&global, &JsValue::from_str("MessageChannel"))
            .ok()?
            .dyn_into::<js_sys::Function>()
            .ok()?;
        let channel = js_sys::Reflect::construct(&ctor, &js_sys::Array::new()).ok()?;
        let port1 = js_sys::Reflect::get(&channel, &JsValue::from_str("port1")).ok()?;
        let port2 = js_sys::Reflect::get(&channel, &JsValue::from_str("port2")).ok()?;
        if port1.is_undefined() || port1.is_null() || port2.is_undefined() || port2.is_null() {
            return None;
        }
        let post_message = js_sys::Reflect::get(&port1, &JsValue::from_str("postMessage"))
            .ok()?
            .dyn_into::<js_sys::Function>()
            .ok()?;

        let (tx, rx) = futures::channel::oneshot::channel();
        let mut tx = Some(tx);
        let on_message = Closure::wrap(Box::new(move |_event: JsValue| {
            if let Some(sender) = tx.take() {
                let _ = sender.send(());
            }
        }) as Box<dyn FnMut(JsValue)>);

        if !matches!(
            js_sys::Reflect::set(&port2, &JsValue::from_str("onmessage"), on_message.as_ref()),
            Ok(true)
        ) {
            close_port(&port1);
            close_port(&port2);
            return None;
        }

        let yield_fut = Self {
            port1,
            port2,
            rx,
            _on_message: on_message,
        };
        if post_message
            .call1(&yield_fut.port1, &JsValue::UNDEFINED)
            .is_err()
        {
            return None;
        }
        Some(yield_fut)
    }
}

impl Drop for MacrotaskYield {
    fn drop(&mut self) {
        let _ = js_sys::Reflect::set(&self.port2, &JsValue::from_str("onmessage"), &JsValue::NULL);
        close_port(&self.port1);
        close_port(&self.port2);
    }
}

impl std::future::Future for MacrotaskYield {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match std::pin::Pin::new(&mut self.rx).poll(cx) {
            std::task::Poll::Ready(_) => std::task::Poll::Ready(()),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

fn close_port(port: &JsValue) {
    if let Ok(close) = js_sys::Reflect::get(port, &JsValue::from_str("close"))
        && let Some(func) = close.dyn_ref::<js_sys::Function>()
    {
        let _ = func.call0(port);
    }
}

/// Yield one browser macrotask without calling `setTimeout(0)`, avoiding
/// Firefox's background-tab timer clamping (`>= 1 s` / `10 s` budget throttling).
pub async fn yield_to_browser() {
    if let Some(macrotask) = MacrotaskYield::try_new() {
        macrotask.await;
    } else {
        let _ = JsFuture::from(js_sys::Promise::resolve(&JsValue::UNDEFINED)).await;
    }
}

struct VisibilityHub {
    waiters: std::rc::Rc<std::cell::RefCell<Vec<futures::channel::mpsc::UnboundedSender<()>>>>,
    listener: Option<Closure<dyn FnMut(JsValue)>>,
}

impl VisibilityHub {
    fn new() -> Self {
        let waiters = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let listener = register_visibility_listener(std::rc::Rc::clone(&waiters));
        Self { waiters, listener }
    }

    fn subscribe(&self) -> Option<futures::channel::mpsc::UnboundedReceiver<()>> {
        self.listener.as_ref()?;
        let (tx, rx) = futures::channel::mpsc::unbounded();
        let Ok(mut waiters) = self.waiters.try_borrow_mut() else {
            return None;
        };
        waiters.retain(|sender| !sender.is_closed());
        waiters.push(tx);
        Some(rx)
    }
}

fn register_visibility_listener(
    waiters: std::rc::Rc<std::cell::RefCell<Vec<futures::channel::mpsc::UnboundedSender<()>>>>,
) -> Option<Closure<dyn FnMut(JsValue)>> {
    let global = js_sys::global();
    let doc = js_sys::Reflect::get(&global, &JsValue::from_str("document")).ok()?;
    if doc.is_undefined() || doc.is_null() {
        return None;
    }
    let add_listener = js_sys::Reflect::get(&doc, &JsValue::from_str("addEventListener"))
        .ok()?
        .dyn_into::<js_sys::Function>()
        .ok()?;
    let cb = Closure::wrap(Box::new(move |_event: JsValue| {
        if !document_hidden()
            && let Ok(mut list) = waiters.try_borrow_mut()
        {
            list.retain(|sender| sender.unbounded_send(()).is_ok());
        }
    }) as Box<dyn FnMut(JsValue)>);
    add_listener
        .call2(&doc, &JsValue::from_str("visibilitychange"), cb.as_ref())
        .ok()?;
    Some(cb)
}

thread_local! {
    static VISIBILITY_HUB: VisibilityHub = VisibilityHub::new();
}

fn subscribe_visibility_resume() -> Option<futures::channel::mpsc::UnboundedReceiver<()>> {
    VISIBILITY_HUB.with(VisibilityHub::subscribe)
}

async fn wait_deadline_with_grace(duration: std::time::Duration, resume_grace_ms: u32) {
    let mut tracker = DeadlineTracker::new(now_epoch_ms(), duration, resume_grace_ms);
    let mut remaining_ms = duration_millis_u32(duration);
    let mut vis_rx = subscribe_visibility_resume();

    loop {
        if remaining_ms == 0 {
            return;
        }
        let timer = gloo_timers::future::TimeoutFuture::new(remaining_ms);
        if let Some(rx) = vis_rx.as_mut() {
            futures::pin_mut!(timer);
            let next_vis = rx.next();
            futures::pin_mut!(next_vis);
            match futures::future::select(timer, next_vis).await {
                futures::future::Either::Left(((), _)) => return,
                futures::future::Either::Right((Some(()), _)) => {
                    remaining_ms = tracker.remaining_on_resume(now_epoch_ms());
                }
                futures::future::Either::Right((None, timer_fut)) => {
                    timer_fut.await;
                    return;
                }
            }
        } else {
            timer.await;
            return;
        }
    }
}

/// Wait until `duration` has elapsed, re-checking wall-clock progress on
/// `visibilitychange` when returning to the foreground and granting a brief
/// grace window so buffered network responses can arrive first.
pub async fn wait_deadline(duration: std::time::Duration) {
    wait_deadline_with_grace(duration, RESUME_GRACE_MS).await;
}

/// Sleep for `duration` of wall-clock time without extending the delay when a
/// backgrounded tab returns to the foreground.
pub async fn sleep_wall_clock(duration: std::time::Duration) {
    wait_deadline_with_grace(duration, 0).await;
}

async fn with_idle_timeout<T, F>(fut: F, label: &str) -> Result<T, FetchError>
where
    F: std::future::Future<Output = Result<T, reqwest::Error>>,
{
    let timeout = wait_deadline(CONNECTION_IDLE_TIMEOUT);
    futures::pin_mut!(fut);
    futures::pin_mut!(timeout);
    match futures::future::select(fut, timeout).await {
        futures::future::Either::Left((result, _timeout)) => {
            result.map_err(|e| FetchError::Network(e.to_string()))
        }
        futures::future::Either::Right(((), _fut)) => {
            Err(FetchError::Network(format!("request timed out {label}")))
        }
    }
}

pub(super) async fn send_request(
    request: reqwest::RequestBuilder,
) -> Result<reqwest::Response, FetchError> {
    with_idle_timeout(request.send(), "while waiting for response headers").await
}

pub(super) async fn read_bytes(response: reqwest::Response) -> Result<ResponseBody, FetchError> {
    with_idle_timeout(response.bytes(), "while reading response body").await
}

pub(super) async fn read_text(response: reqwest::Response) -> Result<String, FetchError> {
    with_idle_timeout(response.text(), "while reading response body").await
}

/// Wasm streaming: `reqwest`'s `ReadableStream` as a `Stream` of `Bytes`.
pub(super) struct WasmChunks {
    inner: std::pin::Pin<Box<dyn futures::Stream<Item = Result<ResponseBody, reqwest::Error>>>>,
    progress: ReadProgress,
    yield_schedule: YieldSchedule,
}

impl WasmChunks {
    pub(super) fn new(response: reqwest::Response) -> Self {
        Self {
            inner: Box::pin(response.bytes_stream()),
            progress: ReadProgress::default(),
            yield_schedule: YieldSchedule::new(now_epoch_ms()),
        }
    }

    fn finish_chunk(
        &mut self,
        ready: Option<Result<ResponseBody, reqwest::Error>>,
    ) -> Result<Option<ResponseBody>, FetchError> {
        let chunk = ready
            .transpose()
            .map_err(|e| self.progress.failed(e.to_string()))?;
        if let Some(bytes) = &chunk {
            self.yield_schedule.record_bytes(bytes.len());
        }
        Ok(self.progress.arrived(chunk))
    }
}

impl BodyChunks for WasmChunks {
    fn next_chunk(&mut self) -> ChunkFuture<'_> {
        Box::pin(async {
            if self
                .yield_schedule
                .should_yield(now_epoch_ms(), document_hidden())
            {
                yield_to_browser().await;
                self.yield_schedule.mark_yielded(now_epoch_ms());
            }
            let mut next = self.inner.next();
            // Fast path: if a chunk is already buffered in the browser's
            // `ReadableStream`, take it immediately without allocating and
            // cancelling a 45s `setTimeout` across the WASM-JS boundary.
            if let Some(ready) = (&mut next).now_or_never() {
                return self.finish_chunk(ready);
            }
            let idle = wait_deadline(STREAM_STALL_TIMEOUT);
            futures::pin_mut!(next);
            futures::pin_mut!(idle);
            // `select` is biased to its first argument, so a chunk that lands in
            // the same tick as the deadline wins. That ordering is the whole
            // point: the deadline must not eat data that has already arrived.
            match futures::future::select(next, idle).await {
                futures::future::Either::Left((chunk, _idle)) => self.finish_chunk(chunk),
                futures::future::Either::Right(((), _next)) => Err(self.progress.stalled()),
            }
        })
    }
}
