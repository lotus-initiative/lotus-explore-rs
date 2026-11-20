// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `header_meta`, in their own file.

use super::ResolvedTaxonMetaItem;
use crate::i18n::Locale;
use dioxus::prelude::*;

fn render(typed: &'static str, linked_names: bool) -> String {
    let mut dom = VirtualDom::new_with_props(
        ResolvedTaxonMetaItem,
        super::ResolvedTaxonMetaItemProps {
            locale: Locale::En,
            qid: "Q10620358".into(),
            typed: typed.to_owned(),
            linked_names,
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn the_header_names_what_was_searched_and_that_linked_names_are_included() {
    let html = render("Michelia champaca", true);
    assert!(html.contains(">Searched taxon</span>"), "{html}");
    assert!(html.contains(">Michelia champaca</span>"), "{html}");
    assert!(
        html.contains(
            ">Records filed under a linked name, such as a synonym or a basionym, are included.</span>"
        ),
        "{html}"
    );
}

#[test]
fn a_typed_qid_is_not_repeated_and_no_claim_is_made_with_the_switches_off() {
    let html = render("q10620358", false);
    assert_eq!(html.matches("10620358").count(), 1, "{html}");
    assert!(!html.contains("linked name"), "{html}");
}
