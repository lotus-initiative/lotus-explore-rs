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
