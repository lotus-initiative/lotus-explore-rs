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
    if super::identify::find_ascii_ci(text, b"M  END").is_none() {
        return StructureKind::Smiles;
    }
    if super::identify::find_ascii_ci(text, b"V3000").is_some()
        || super::identify::find_ascii_ci(text, b"BEGIN CTAB").is_some()
    {
        StructureKind::MolfileV3000
    } else if super::identify::find_ascii_ci(text, b"V2000").is_some() {
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
    let Some((first, rest)) = text.split_once('-') else {
        return false;
    };
    let Some((second, third)) = rest.split_once('-') else {
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

/// What the reference field's example button offers: a DOI from the *Journal
/// of Natural Products*, resolved by looking the reference up. It reports two
/// compounds, *Voatriafricanine A* and *B*.
///
/// The list lives here, not in the app, so a test can read the same strings
/// the buttons fill in.
pub const REFERENCE_INPUT_EXAMPLES: [&str; 1] = ["10.1021/acs.jnatprod.1c00812"];

/// What the structure field's example buttons offer: *carnosol* (`Q16634054`)
/// three ways, so a reader sees the kinds of input the field takes and that
/// they agree.
///
/// * `carnosol` — a name, resolved through a label or alias lookup.
/// * `XUSYGBPHQBWGAD-PJSUUKDQSA-N` — its `InChIKey`, resolved through `P235`.
/// * its isomeric SMILES — resolved by asking the structure service.
///
/// The list lives here, not in the app, so the test checking their
/// classification reads the same strings the buttons fill in.
pub const STRUCTURE_INPUT_EXAMPLES: [&str; 3] = [
    "carnosol",
    "XUSYGBPHQBWGAD-PJSUUKDQSA-N",
    "CC(C)C1=C(C(=C2C(=C1)[C@@H]3C[C@@H]4[C@@]2(CCCC4(C)C)C(=O)O3)O)O",
];

/// Whether a string is a Wikidata QID.
///
/// QIDs are `Q` followed by digits, and this accepts a lowercase `q` because the
/// taxon field has always done so and a reader who types `q18216` means
/// Q18216. The digit requirement is what separates it from a structure: no SMILES
/// starts with `Q`, and `CCC` is not a QID.
#[must_use]
pub fn looks_like_a_compound_qid(text: &str) -> bool {
    super::looks_like_reference_qid(text)
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
#[path = "structure/tests.rs"]
mod tests;

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
