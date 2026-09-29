// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_model::ElementState;

#[derive(Clone, PartialEq)]
pub(super) struct FormulaSectionState {
    pub(super) formula_enabled: bool,
    pub(super) formula_exact: String,
    pub(super) c_min: u16,
    pub(super) c_max: u16,
    pub(super) h_min: u16,
    pub(super) h_max: u16,
    pub(super) n_min: u16,
    pub(super) n_max: u16,
    pub(super) o_min: u16,
    pub(super) o_max: u16,
    pub(super) p_min: u16,
    pub(super) p_max: u16,
    pub(super) s_min: u16,
    pub(super) s_max: u16,
    pub(super) f_state: ElementState,
    pub(super) cl_state: ElementState,
    pub(super) br_state: ElementState,
    pub(super) i_state: ElementState,
}

pub(super) fn parse_f64_input(raw: &str) -> Option<f64> {
    raw.parse::<f64>().ok()
}

pub(super) fn parse_u16_input(raw: &str) -> Option<u16> {
    raw.parse::<u16>().ok()
}

#[must_use]
pub(super) fn normalized_year_input_max(current_year: u16) -> u16 {
    current_year.max(lotus_model::YEAR_MIN)
}

#[cfg(test)]
mod tests {
    use super::{normalized_year_input_max, parse_f64_input, parse_u16_input};

    #[test]
    fn parse_f64_input_accepts_valid_numbers_and_rejects_invalid_text() {
        assert_eq!(parse_f64_input("42"), Some(42.0));
        assert_eq!(parse_f64_input("2.5"), Some(2.5));
        assert_eq!(parse_f64_input("abc"), None);
    }

    #[test]
    fn parse_u16_input_accepts_positive_integers_only() {
        assert_eq!(parse_u16_input("007"), Some(7));
        assert_eq!(parse_u16_input("65535"), Some(u16::MAX));
        assert_eq!(parse_u16_input("-1"), None);
        assert_eq!(parse_u16_input("12.5"), None);
    }

    #[test]
    fn normalized_year_input_max_never_drops_below_default_floor() {
        assert_eq!(normalized_year_input_max(2030), 2030);
        assert_eq!(
            normalized_year_input_max(lotus_model::YEAR_MIN),
            lotus_model::YEAR_MIN
        );
        assert_eq!(normalized_year_input_max(1700), lotus_model::YEAR_MIN);
    }
}
