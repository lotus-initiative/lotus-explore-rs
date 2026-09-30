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
/// One number, deliberately, and no `cfg` split: a native window is a browser
/// window as far as the table is concerned -- same DOM, same layout cost, a few
/// hundred pixels of difference -- and giving the desktop ten times the rows
/// only bought a table nobody scrolls that far. It is also what a desktop
/// *browser* is served, so the two hosts agree.
///
/// The ceiling used to be 1,000 on the web and 5,000 natively, on the theory that
/// a bigger machine could absorb it. Rendering cost tracks the row count, not the
/// machine: the window was already unusable before the extra rows appeared, and
/// the extra rows still cost every scroll, every sort and every column resize.
///
/// The API server is a different program and does not render a table at all; it
/// has its own ceiling in [`API_MAX_ROWS`], which stays an order of magnitude
/// above this so that lowering what a window can draw does not lower what a bulk
/// caller can export.
pub const TABLE_ROW_LIMIT: usize = 500;

/// Step-down budgets for devices that report they cannot afford [`TABLE_ROW_LIMIT`].
const LOW_MEMORY_ROWS: usize = 220;
const MID_MEMORY_ROWS: usize = 360;
const MOBILE_MAX_ROWS: usize = 280;

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
/// for mobile. A phone that says it has 2 GB gets 220 rows rather than the
/// [`TABLE_ROW_LIMIT`] a desktop gets, which is the difference between a usable
/// page and a white one. Where neither signal is available the ceiling stands.
///
/// The step down is one-way. A machine reporting 8 GB or more used to be given
/// more rows than the ceiling allowed, which the clamp below silently discarded,
/// so the branch is gone rather than left to look meaningful.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn runtime_table_row_limit() -> usize {
    let mut limit = TABLE_ROW_LIMIT;
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
                    limit = LOW_MEMORY_ROWS;
                } else if gb <= 4.0 {
                    limit = MID_MEMORY_ROWS;
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
                    limit = limit.min(MOBILE_MAX_ROWS);
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
    fn every_host_gets_the_same_ceiling() {
        // The other tests here only compare the budget to the ceiling, so they
        // would still pass if the ceiling went back to 1,000 on the web and 5,000
        // on the desktop -- which is the exact regression this constant was
        // changed to fix. Asserting the number is the point; without it, nothing
        // fails when a host quietly gets ten times the rows again.
        assert_eq!(TABLE_ROW_LIMIT, 500, "the row ceiling is one number now");
        assert_eq!(
            runtime_table_row_limit(),
            TABLE_ROW_LIMIT,
            "nothing raises a host above the ceiling any more"
        );
    }

    /// The ceiling is the most any host may be served, so the per-device step
    /// down below it has to be reachable from it.
    ///
    /// The budget is the one part of this that cannot be tested on its own: it
    /// reads `navigator`, so off the web there is no window to ask and the
    /// function returns the ceiling unchanged. What can be checked is the
    /// relationship the device branches rely on -- each one is a floor value
    /// that only ever lowers the budget -- and that is what
    /// [`every_device_step_down_is_below_the_ceiling`] pins.
    #[test]
    fn every_device_step_down_is_below_the_ceiling() {
        // The values `runtime_table_row_limit` assigns. Each has to sit below the
        // ceiling, because the initial budget is the ceiling and a branch that
        // raises it would be discarded by the clamp and read as though it did
        // something.
        const DEVICE_STEP_DOWNS: [usize; 3] = [LOW_MEMORY_ROWS, MID_MEMORY_ROWS, MOBILE_MAX_ROWS];
        for rows in DEVICE_STEP_DOWNS {
            assert!(
                rows <= TABLE_ROW_LIMIT,
                "{rows} rows is not a step down from a ceiling of {TABLE_ROW_LIMIT}"
            );
        }
        assert_eq!(
            TABLE_ROW_LIMIT, 500,
            "the ceiling is the number both hosts share"
        );
    }

    #[test]
    fn the_api_can_still_export_a_bulk_result() {
        // Lowering what a window can draw must not lower what the endpoint is
        // asked for on a bulk caller's behalf. The margin is wide on purpose: the
        // table ceiling moves with UI performance and the export ceiling does
        // not, and a table ceiling that reached it would silently truncate
        // exports.
        #[cfg(feature = "server")]
        assert!(
            API_MAX_ROWS >= 100 * TABLE_ROW_LIMIT,
            "the export ceiling has lost its margin over the table ceiling"
        );
    }
    #[test]
    fn the_api_ceiling_is_not_the_table_ceiling() {
        // They were the same constant, so lowering the one a phone can draw
        // also lowered how much the server would return to a bulk caller. The
        // two answer different questions and have to move apart.
        #[cfg(feature = "server")]
        {
            // A `const` block, because both operands are constants and the
            // assertion is about the relationship between them rather than
            // about a runtime value. The messages are static strings: a const
            // block cannot format.
            const _: () = assert!(
                API_MAX_ROWS > TABLE_ROW_LIMIT,
                "the API must be able to return more than one screen of rows"
            );
            const _: () = assert!(
                TABLE_ROW_LIMIT > 0 && API_MAX_ROWS > 0,
                "a limit of zero means the caller can never have a result"
            );
        }
    }
}
