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
    /// How to name this format in a form or a message.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Empty => "—",
            Self::Smiles => "SMILES",
            Self::MolfileV2000 => "Molfile V2000",
            Self::MolfileV3000 => "Molfile V3000",
        }
    }

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
/// than to an error is deliberate: the endpoint will reject an unparseable
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

#[cfg(test)]
mod tests {
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
}
