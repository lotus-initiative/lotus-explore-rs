// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `structure`, in their own file.

use super::*;

const V2000: &str = "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n    0.0000    0.0000    0.0000 C   0  0  0  0  0  0\nM  END\n";
const V3000: &str = "\n\n\n  0  0  0  0  0  0            999 V3000\nM  V30 BEGIN CTAB\nM  END\n";

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
fn the_examples_are_a_name_a_key_a_structure_and_a_name_in_another_language() {
    // The buttons exist to show a reader which kind they are typing, so each
    // has to land on a different path -- and the test pins the same strings
    // the buttons fill in.
    let expected: [(&str, InputKind); 4] = [
        ("carnosol", InputKind::Name),
        ("XUSYGBPHQBWGAD-PJSUUKDQSA-N", InputKind::InChIKey),
        (
            "CC(C)C1=C(C(=C2C(=C1)[C@@H]3C[C@@H]4[C@@]2(CCCC4(C)C)C(=O)O3)O)O",
            InputKind::Structure,
        ),
        ("amarogentina", InputKind::Name),
    ];
    assert_eq!(
        STRUCTURE_INPUT_EXAMPLES,
        expected.map(|(example, _)| example)
    );
    for (example, kind) in expected {
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

#[test]
fn numbered_and_unicode_chemical_names_are_offered_to_the_name_lookup() {
    for name in [
        "5-fluorouracil",
        "12-O-Methylcarnosic acid",
        "7-O-Methylrosmanol",
        "1,8-cineole",
        "14′-O-sulfocriophylline",
        "14'-O-sulfocriophylline",
        "(±)-3-carene",
        "(−)-epicatechin",
        "vitamin B12",
        "β-pinene",
        "2-methylbutane",
    ] {
        assert!(could_be_a_compound_name(name), "{name} should be looked up");
    }
}

#[test]
fn salts_and_smiles_with_digits_stay_structures() {
    for input in [
        "CCO.O",
        "CCN.Cl",
        "c1ccccc1",
        "C1CCCCC1",
        "CC1CC1",
        "Q18216",
        "q18216",
        "a1",
        "c1ccccc1-c2ccccc2",
        "b1ccccc1-c2ccccc2",
        "n1ccccc1-c2ccccc2",
        "o1cccc1-c2cccc2",
        "p1ccccc1-c2ccccc2",
        "s1cccc1-c2cccc2",
        "Cl-c1ccccc1",
        "Br-c1ccccc1",
    ] {
        assert!(
            !could_be_a_compound_name(input),
            "{input} must not go to compound-name lookup"
        );
    }
}

#[test]
fn normalize_structure_trims_smiles_and_keeps_molfile_header_blanks() {
    assert_eq!(normalize_structure("  CCO \r\n"), "CCO");
    let mol = "\r\n  Mrv2014 010124\n\n  0  0  0     0  0            999 V2000\nM  END\r\n";
    let normalized = normalize_structure(mol);
    assert!(
        normalized.starts_with("\n  Mrv"),
        "leading blank header line must survive normalize_structure: {normalized:?}"
    );
    assert!(!normalized.contains('\r'));
}
