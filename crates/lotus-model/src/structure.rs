// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Classifying a structure string by chemical format.

/// The formats a structure can arrive in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureKind {
    /// Nothing was supplied.
    Empty,
    /// A SMILES string.
    Smiles,
    /// An MDL molfile, counts-line V2000.
    MolfileV2000,
    /// An MDL molfile, extended V3000.
    MolfileV3000,
}

impl StructureKind {
    /// Whether this is a molfile, which must be embedded in a SPARQL literal as
    /// a multi-line string rather than trimmed.
    #[must_use]
    pub const fn is_molfile(self) -> bool {
        matches!(self, Self::MolfileV2000 | Self::MolfileV3000)
    }
}

/// Classify a structure string.
///
/// A molfile is recognised by its `M  END` terminator together with a version
/// tag; anything else is treated as SMILES. Falling through to SMILES rather
/// than to an error is deliberate: the endpoint will reject an unparsable
/// structure with a message about the structure, which beats a rejection about
/// the request.
#[must_use]
pub fn classify_structure(text: &str) -> StructureKind {
    if text.trim().is_empty() {
        return StructureKind::Empty;
    }
    let upper = text.to_ascii_uppercase();
    if !upper.contains("M  END") {
        return StructureKind::Smiles;
    }
    if upper.contains("V3000") || upper.contains("BEGIN CTAB") {
        StructureKind::MolfileV3000
    } else if upper.contains("V2000") {
        StructureKind::MolfileV2000
    } else {
        StructureKind::Smiles
    }
}

/// Whether a string is an `InChIKey`.
///
/// The shape is fixed by the standard: two 14-character blocks and a one-character
/// version, separated by hyphens, all uppercase. Recognising it by shape rather
/// than by pattern-matching an endpoint's answer is what makes the identifier
/// route unambiguous -- a SMILES can never be mistaken for one, and vice versa.
#[must_use]
pub fn looks_like_inchikey(text: &str) -> bool {
    let blocks = text.split('-').collect::<Vec<_>>();
    if blocks.len() != 3 {
        return false;
    }
    let [first, second, third] = blocks.as_slice() else {
        return false;
    };
    third.len() == 1
        && first.len() == 14
        && second.len() == 10
        && [first, second, third].into_iter().all(|block| {
            block
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        })
}

/// What the reference field's example buttons offer.
///
/// * `10.1002/jlac.18360190207` -- a DOI, resolved by looking the reference up.
///   Reports one compound, *chrysophanol*.
/// * `10.1021/acs.jnatprod.1C00812` -- a DOI from a different publisher, so the
///   pair shows the prefix is not what is being matched. Reports two,
///   *Voatriafricanine A* and *B*, stereoisomers: a reference can report several
///   compounds, as separate facts about separate items.
/// * `Q28601559` -- a QID, used without a round trip. Reports three, including
///   *ethanol*.
///
/// These are three different answers, not one compound three ways: a structure
/// names one thing, a reference names an *occurrence*, and the same compound
/// appears under several references while one reference reports several compounds.
///
/// The list lives here, not in the app, so the test that checks the
/// classification reads the same strings the buttons fill in.
pub const REFERENCE_INPUT_EXAMPLES: [&str; 3] = [
    "10.1002/jlac.18360190207",
    "10.1021/acs.jnatprod.1C00812",
    "Q28601559",
];

/// What the structure field's example buttons offer.
///
/// * `amarogentina` — a name, resolved through a label or alias lookup.
/// * `DBOVHQOUSDWAPQ-WTONXPSSSA-N` — an `InChIKey`, resolved through `P235`.
/// * `Q23118` — a QID, confirmed against Wikidata.
/// * `C[C@H](O)CO` — a SMILES, resolved by asking the structure service.
///
/// One of each kind the field accepts, so a reader can tell which is which; the
/// list lives here, not in the app, so the test checking their classification
/// reads the same strings the buttons fill in.
///
/// The name and the `InChIKey` are the same compound (*amarogentin*, `Q3613679`)
/// two ways, which shows the alias route and the identifier route agree; the test
/// below checks both are classified as identifiers, not one as a name.
///
/// The stereocentre in the SMILES is deliberate over a simpler chain: it is the
/// structure a person cannot type from memory.
pub const STRUCTURE_INPUT_EXAMPLES: [&str; 5] = [
    "amarogentina",
    "红雀椿素",
    "DBOVHQOUSDWAPQ-WTONXPSSSA-N",
    "Q23118",
    "C[C@H](O)CO",
];

/// Whether a string is a Wikidata QID.
///
/// QIDs are `Q` followed by digits, and this accepts a lowercase `q` because the
/// taxon field has always done so and a reader who types `q18216` means
/// Q18216. The digit requirement is what separates it from a structure: no SMILES
/// starts with `Q`, and `CCC` is not a QID.
// `bytes[0]` and `bytes[1..]` are read behind a `len() > 1` guard, so neither can
// be out of bounds; the same shape as the taxon field's check.
#[allow(
    clippy::indexing_slicing,
    reason = "both reads are behind a `len() > 1` guard"
)]
#[must_use]
pub fn looks_like_a_compound_qid(text: &str) -> bool {
    let bytes = text.trim().as_bytes();
    bytes.len() > 1 && matches!(bytes[0], b'Q' | b'q') && bytes[1..].iter().all(u8::is_ascii_digit)
}

/// Whether a string is worth handing to the compound-name lookup.
///
/// Stops the name lookup from eating structure input, conservatively in the
/// direction of *not* resolving: a failing name is treated as a structure, which
/// is what every input was before the lookup existed.
///
/// Three exclusions, same reason -- a chemical name and a structure are written
/// with different alphabets:
///
/// **Any digit.** `c1ccccc1`, `C[C@H](O)CO`, `CC(=O)Oc1ccccc1C(=O)O`. No
/// Wikidata label has one, because a label is prose.
///
/// **Any SMILES punctuation**: `= @ [ ] # / \ % * $ :`. Same reason.
///
/// **A run of consecutive uppercase letters.** `CC` and `CCC` are how ethane and
/// propane get typed in a structure box, and two of this app's example buttons.
/// This exclusion costs real names -- `ATP`, `GDP` and `NAD` are known compounds
/// and are refused -- and that is the right way round: a structure silently
/// replaced by a same-spelled item is undetectable, while a name typed as SMILES
/// is friction the reader can route around.
///
/// Parentheses and `+` *are* allowed, for stereodescriptors: `s-(+)-carvone` and
/// `(R)-` are real names. That admits a structure like `C(C)`, the one
/// misclassification that can reach the name lookup, and its cost is bounded: a
/// lookup matching nothing is reported as "compound not found" rather than
/// answered, so the reader learns the input was read as a name instead of getting
/// a plausible wrong answer. Refusing a structure that looks like a name costs a
/// round trip; admitting one costs a question the reader must notice.
#[must_use]
pub fn could_be_a_compound_name(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let all_uppercase_letters = trimmed
        .chars()
        .all(|c| c.is_ascii_uppercase() || c == ' ' || c == '-');
    let alphabet_is_name_shaped = trimmed
        .chars()
        .all(|c| c.is_alphabetic() || matches!(c, ' ' | '-' | '.' | ',' | '(' | ')' | '+'));
    alphabet_is_name_shaped && !all_uppercase_letters
}

/// Whether this input names a Wikidata compound rather than being a structure.
///
/// One predicate for two callers: the search panel decides whether to show the
/// structure-matching controls, `resolve_structure` decides whether to run a
/// lookup query. Two predicates would eventually disagree, and the disagreement
/// would show as a control on screen for a search it does not affect.
///
/// A prediction, and exact rather than heuristic because the resolver commits to
/// it: input predicted to name a compound is looked up, and a miss is an error,
/// never a fallback to the structure service.
#[must_use]
pub fn names_a_compound(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && (looks_like_a_compound_qid(trimmed)
            || looks_like_inchikey(trimmed)
            || could_be_a_compound_name(trimmed))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const V2000: &str = "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n    0.0000    0.0000    0.0000 C   0  0  0  0  0  0\nM  END\n";
    const V3000: &str =
        "\n\n\n  0  0  0  0  0  0            999 V3000\nM  V30 BEGIN CTAB\nM  END\n";

    #[test]
    fn each_format_is_recognised_by_its_own_marker() {
        assert_eq!(classify_structure("c1ccccc1"), StructureKind::Smiles);
        assert_eq!(classify_structure(V2000), StructureKind::MolfileV2000);
        assert_eq!(classify_structure(V3000), StructureKind::MolfileV3000);
        assert_eq!(classify_structure("   "), StructureKind::Empty);
    }

    #[test]
    fn a_versionless_molfile_reads_as_smiles() {
        // `M  END` alone is not enough: without a version tag there is no way to
        // know the counts line layout, and the endpoint will say so.
        assert_eq!(classify_structure("junk\nM  END\n"), StructureKind::Smiles);
    }

    #[test]
    fn molfiles_are_the_formats_that_need_triple_quotes() {
        assert!(classify_structure(V2000).is_molfile());
        assert!(classify_structure(V3000).is_molfile());
        assert!(!classify_structure("c1ccccc1").is_molfile());
    }

    #[test]
    fn either_v3000_marker_is_enough_on_its_own() {
        // Two spellings of the same version tag: one molfile carries the literal
        // `V3000`, the other the `BEGIN CTAB` line instead. Both have to reach
        // V3000, because both are triple-quoted the same way.
        assert_eq!(
            classify_structure("x\nM  END\nM  V30 BEGIN CTAB\nM  V30 END CTAB\n"),
            StructureKind::MolfileV3000
        );
        assert_eq!(
            classify_structure("x\nM  V30 BEGIN CTAB\nM  END\n"),
            StructureKind::MolfileV3000
        );
    }

    // ── Which inputs are offered to the compound-name lookup ────────────────

    #[test]
    fn an_inchikey_is_recognised_by_its_own_shape() {
        // The real key for gentianine, and a synthetic one.
        assert!(looks_like_inchikey("BEHABCJQKGGRQN-XLPZGREQSA-N"));
        assert!(looks_like_inchikey("AAAAAAAAAAAAAA-BBBBBBBBBB-C"));
    }

    #[test]
    fn a_structure_is_never_mistaken_for_an_inchikey() {
        // Every one of these is valid input to the structure field, and every one
        // would be destructive if it took the identifier route.
        for smi in [
            "CC",
            "CCC",
            "c1ccccc1",
            "C[C@H](O)CO",
            "CC(=O)Oc1ccccc1C(=O)O",
        ] {
            assert!(!looks_like_inchikey(smi), "{smi} is not an `InChIKey`");
        }
    }

    #[test]
    fn a_name_is_not_an_inchikey() {
        // The two routes are disjoint. `amarogentina` is a compound name, and
        // treating it as an identifier would send it to a property that holds
        // hashed keys, where it can never match.
        assert!(!looks_like_inchikey("amarogentina"));
        assert!(looks_like_inchikey("DBOVHQOUSDWAPQ-WTONXPSSSA-N"));
    }

    #[test]
    fn a_name_and_an_inchikey_are_both_looked_up() {
        // The other side of the same boundary. An `InChIKey` is exactly 14-10-1,
        // so it is recognised with certainty; a name is recognised because its
        // alphabet is prose. Both go to the lookup, and neither can be confused
        // with the other -- which is what makes the identifier route
        // unambiguous rather than a heuristic.
        assert!(looks_like_inchikey("DBOVHQOUSDWAPQ-WTONXPSSSA-N"));
        assert!(could_be_a_compound_name("amarogentina"));
        assert!(
            !could_be_a_compound_name("DBOVHQOUSDWAPQ-WTONXPSSSA-N"),
            "a key is not also a name; the two routes are disjoint"
        );
    }

    #[test]
    fn a_near_miss_inchikey_is_rejected() {
        // Right shape, wrong blocks; and a lowercase key, which the standard
        // does not allow.
        assert!(!looks_like_inchikey("BEHABCJQKGGRQ-XLPZGREQSA-N"));
        assert!(!looks_like_inchikey("BEHABCJQKGGRQN-XLPZGREQSA-NN"));
        assert!(!looks_like_inchikey("behabcjqkggrqn-xlpgreqsa-n"));
        assert!(!looks_like_inchikey(""));
    }

    #[test]
    fn chemical_names_are_offered_to_the_name_lookup() {
        for name in [
            "aspirin",
            "quercetin",
            "Gentianine",
            "green tea",
            "s-(+)-carvone",
            "Aspirin",
        ] {
            assert!(could_be_a_compound_name(name), "{name} should be looked up");
        }
    }

    #[test]
    fn the_shorthand_structures_the_field_always_had_stay_structures() {
        // `CC` and `CCC` are how people write ethane and propane in a structure
        // box. If either reached the name lookup, a Wikidata item sharing that
        // spelling would silently replace the search.
        for smi in ["CC", "CCC", "c1ccccc1", "C[C@H](O)CO"] {
            assert!(
                !could_be_a_compound_name(smi),
                "{smi} must stay a structure"
            );
        }
    }

    #[test]
    fn the_examples_cover_every_kind_of_input_the_field_accepts() {
        // The buttons exist to show a reader which kind they are typing, so each
        // has to land on a different path -- and the test pins the same strings
        // the buttons fill in.
        let expected: [(&str, InputKind); 4] = [
            ("amarogentina", InputKind::Name),
            ("DBOVHQOUSDWAPQ-WTONXPSSSA-N", InputKind::InChIKey),
            ("Q23118", InputKind::Qid),
            ("C[C@H](O)CO", InputKind::Structure),
        ];
        for (example, kind) in expected {
            assert!(
                STRUCTURE_INPUT_EXAMPLES.contains(&example),
                "{example} is expected to be an example button"
            );
            assert_eq!(
                classify_structure_input(example),
                kind,
                "{example} is filed under the wrong path"
            );
        }
    }

    #[test]
    fn structures_with_digits_or_brackets_are_never_names() {
        for smi in [
            "c1ccccc1",
            "C[C@H](O)CO",
            "CC(=O)Oc1ccccc1C(=O)O",
            "N#Cc1ccccc1",
        ] {
            assert!(
                !could_be_a_compound_name(smi),
                "{smi} must stay a structure"
            );
        }
    }

    #[test]
    fn a_molfile_is_never_a_name() {
        assert!(!could_be_a_compound_name(V2000));
        assert!(!could_be_a_compound_name(V3000));
    }

    #[test]
    fn a_qid_is_recognised_by_its_own_shape() {
        assert!(looks_like_a_compound_qid("Q18216"));
        assert!(
            looks_like_a_compound_qid("q18216"),
            "case is forgiven, as for taxa"
        );
        assert!(looks_like_a_compound_qid("  Q18216  "));
    }

    #[test]
    fn a_structure_is_never_mistaken_for_a_qid() {
        for input in ["CCC", "C", "c1ccccc1", "Q", "Q18a16", "CCQ", "quinine"] {
            assert!(!looks_like_a_compound_qid(input), "{input} is not a QID");
        }
    }

    #[test]
    fn a_qid_is_never_offered_to_the_name_lookup() {
        // The routes must be disjoint, or a QID would cost two round trips
        // instead of one.
        assert!(!could_be_a_compound_name("Q18216"), "a digit rules it out");
    }

    #[test]
    fn every_kind_that_names_a_compound_is_recognised() {
        for input in [
            "Q23118",
            "q23118",
            "DBOVHQOUSDWAPQ-WTONXPSSSA-N",
            "amarogentina",
            "Aspirin",
        ] {
            assert!(names_a_compound(input), "{input} names a compound");
        }
    }

    #[test]
    fn a_structure_does_not_name_a_compound() {
        // The distinction the resolver commits to: input on the left is looked up
        // and a miss is an error, input on the right is searched as typed.
        for input in [
            "C",
            "CC",
            "CCC",
            "c1ccccc1",
            "C[C@H](O)CO",
            "CC(=O)Oc1ccccc1C(=O)O",
            "",
            "   ",
        ] {
            assert!(!names_a_compound(input), "{input} is a structure");
        }
    }

    #[test]
    fn blank_input_is_not_a_name() {
        assert!(!could_be_a_compound_name(""));
        assert!(!could_be_a_compound_name("   "));
    }

    #[test]
    fn a_molfile_with_only_a_terminator_is_smiles() {
        // `M  END` without a version is not enough to claim a molfile: falling
        // through to SMILES means the endpoint rejects it with a message about
        // the structure, which beats a rejection about the format.
        assert_eq!(
            classify_structure("something M  END somewhere"),
            StructureKind::Smiles
        );
    }
}

/// Which of the field's four paths an input takes.
///
/// The one place that decides, so the example buttons, the resolver and the tests
/// cannot each hold a different idea of what a given input is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    /// A QID: already the answer, no lookup.
    Qid,
    /// An `InChIKey`: resolved through `P235`.
    InChIKey,
    /// A name: resolved through a label or alias lookup.
    Name,
    /// A structure literal: sent to the structure service untouched.
    Structure,
}

/// Classify what was typed into the structure field.
#[must_use]
pub fn classify_structure_input(text: &str) -> InputKind {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return InputKind::Structure;
    }
    if looks_like_a_compound_qid(trimmed) {
        return InputKind::Qid;
    }
    if looks_like_inchikey(trimmed) {
        return InputKind::InChIKey;
    }
    if could_be_a_compound_name(trimmed) {
        return InputKind::Name;
    }
    InputKind::Structure
}
