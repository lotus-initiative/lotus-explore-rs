// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure presentation helpers for the structure section of the search panel.

use crate::i18n::TextKey;
use lotus_model::SmilesSearchType;
use lotus_model::StructureKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StructureSectionModel {
    pub(super) kind_class: &'static str,
    pub(super) note_key: Option<TextKey>,
    pub(super) show_similarity_threshold: bool,
}

#[must_use]
pub(super) fn build_structure_section_model(
    kind: StructureKind,
    smiles_search_type: SmilesSearchType,
) -> StructureSectionModel {
    StructureSectionModel {
        kind_class: kind_class(kind),
        note_key: kind_note_key(kind),
        show_similarity_threshold: smiles_search_type == SmilesSearchType::Similarity,
    }
}

#[must_use]
const fn kind_class(kind: StructureKind) -> &'static str {
    match kind {
        StructureKind::Empty => "empty",
        StructureKind::Smiles => "smiles",
        StructureKind::MolfileV2000 => "mol2000",
        StructureKind::MolfileV3000 => "mol3000",
    }
}

#[must_use]
const fn kind_note_key(kind: StructureKind) -> Option<TextKey> {
    match kind {
        StructureKind::Empty => None,
        StructureKind::Smiles => Some(TextKey::KindNoteSmiles),
        StructureKind::MolfileV2000 => Some(TextKey::KindNoteMol2000),
        StructureKind::MolfileV3000 => Some(TextKey::KindNoteMol3000),
    }
}

#[cfg(test)]
mod tests {
    use super::build_structure_section_model;
    use crate::i18n::TextKey;
    use lotus_model::SmilesSearchType;
    use lotus_model::StructureKind;

    #[test]
    fn empty_structure_shows_empty_hint_state_without_threshold() {
        let model =
            build_structure_section_model(StructureKind::Empty, SmilesSearchType::Substructure);

        assert_eq!(model.kind_class, "empty");
        assert_eq!(model.note_key, None);
        assert!(!model.show_similarity_threshold);
    }

    #[test]
    fn structure_kind_mapping_exposes_note_keys_and_classes() {
        let smiles =
            build_structure_section_model(StructureKind::Smiles, SmilesSearchType::Substructure);
        let mol2000 = build_structure_section_model(
            StructureKind::MolfileV2000,
            SmilesSearchType::Substructure,
        );
        let mol3000 = build_structure_section_model(
            StructureKind::MolfileV3000,
            SmilesSearchType::Substructure,
        );

        assert_eq!(smiles.kind_class, "smiles");
        assert_eq!(smiles.note_key, Some(TextKey::KindNoteSmiles));
        assert_eq!(mol2000.kind_class, "mol2000");
        assert_eq!(mol2000.note_key, Some(TextKey::KindNoteMol2000));
        assert_eq!(mol3000.kind_class, "mol3000");
        assert_eq!(mol3000.note_key, Some(TextKey::KindNoteMol3000));
    }

    #[test]
    fn similarity_mode_controls_threshold_visibility_independently_of_kind() {
        let substructure =
            build_structure_section_model(StructureKind::Smiles, SmilesSearchType::Substructure);
        let similarity =
            build_structure_section_model(StructureKind::Smiles, SmilesSearchType::Similarity);

        assert!(!substructure.show_similarity_threshold);
        assert!(similarity.show_similarity_threshold);
    }
}
