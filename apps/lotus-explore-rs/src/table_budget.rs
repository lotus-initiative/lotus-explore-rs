// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! How many rows this host will actually render.
//!
//! The answer is a property of the machine, not of the data, which is why it
//! lives in the web client rather than in a shared crate: the server has no
//! browser to ask, and a query library has no business knowing what a table
//! costs. A search for a genus can return tens of thousands of rows; drawing
//! that many DOM nodes locks up a phone, and the endpoint is asked for no more
//! than is going to be shown.

/// The ceiling, by host.
///
/// A desktop window is a browser window: the same DOM, the same layout cost, and
/// the difference between the two is a few hundred pixels. It gets a larger
/// ceiling than a phone because the machine usually has more to give, and the
/// same one as a desktop browser because the cost is the same.
///
/// The API server is a different program and does not render a table at all; it
/// has its own ceiling, and the two are not conflated here.
#[cfg(target_arch = "wasm32")]
pub const TABLE_ROW_LIMIT: usize = 1_000;
#[cfg(not(target_arch = "wasm32"))]
pub const TABLE_ROW_LIMIT: usize = 5_000;

/// The most rows the API will return for one request.
///
/// Separate from [`TABLE_ROW_LIMIT`] because the two answer different
/// questions. The table ceiling is what a client can draw; this is what one
/// request may carry. They were the same number, which meant a client-side
/// tuning decision -- "a phone cannot draw 1,000 rows" -- silently capped how
/// much a server would return to anyone, including a caller asking for a bulk
/// export over the API.
#[cfg(feature = "server")]
pub const API_MAX_ROWS: usize = 200_000;

/// The row count to actually ask the endpoint for.
///
/// On the browser this reads what the device says about itself:
/// `navigator.deviceMemory` when the browser reports it, and the user agent
/// for mobile. A phone that says it has 2 GB gets 220 rows rather than the 500
/// a desktop gets, which is the difference between a usable page and a white
/// one. Where neither signal is available the conservative default stands.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn runtime_table_row_limit() -> usize {
    let mut limit = 500usize;
    if let Some(win) = web_sys::window() {
        let win_js = wasm_bindgen::JsValue::from(win);
        if let Ok(nav) =
            js_sys::Reflect::get(&win_js, &wasm_bindgen::JsValue::from_str("navigator"))
        {
            if let Ok(mem) =
                js_sys::Reflect::get(&nav, &wasm_bindgen::JsValue::from_str("deviceMemory"))
                && let Some(gb) = mem.as_f64()
            {
                if gb <= 2.0 {
                    limit = 220;
                } else if gb <= 4.0 {
                    limit = 360;
                } else if gb >= 8.0 {
                    limit = 800;
                }
            }

            if let Ok(ua) =
                js_sys::Reflect::get(&nav, &wasm_bindgen::JsValue::from_str("userAgent"))
                && let Some(ua) = ua.as_string()
            {
                let ua = ua.to_ascii_lowercase();
                let mobile = ua.contains("iphone")
                    || ua.contains("ipad")
                    || ua.contains("android")
                    || ua.contains("mobile");
                if mobile {
                    limit = limit.min(280);
                }
            }
        }
    }
    limit.clamp(180, TABLE_ROW_LIMIT)
}

/// On the server there is no table to fill, so the ceiling is the ceiling.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub const fn runtime_table_row_limit() -> usize {
    TABLE_ROW_LIMIT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_budget_is_within_the_ceiling() {
        let limit = runtime_table_row_limit();
        assert!(
            limit >= 180,
            "a table this small is not worth fetching: {limit}"
        );
        assert!(limit <= TABLE_ROW_LIMIT, "{limit} exceeds the ceiling");
    }

    #[test]
    fn the_budget_is_never_zero() {
        // Every call site divides by this or iterates it; zero would be a hang
        // rather than an error.
        assert!(runtime_table_row_limit() > 0);
    }
    #[test]
    fn the_api_ceiling_is_not_the_table_ceiling() {
        // They were the same constant, so lowering the one a phone can draw
        // also lowered how much the server would return to a bulk caller. The
        // two answer different questions and have to move apart.
        #[cfg(feature = "server")]
        {
            assert!(
                API_MAX_ROWS > TABLE_ROW_LIMIT,
                "the API must be able to return more than one screen of rows: \
                 {API_MAX_ROWS} vs {TABLE_ROW_LIMIT}"
            );
            assert!(
                TABLE_ROW_LIMIT > 0 && API_MAX_ROWS > 0,
                "a limit of zero means the caller can never have a result"
            );
        }
    }
}
