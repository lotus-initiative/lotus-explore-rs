// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub use lotus_curation::parse_tsv as parse_tsv_rows;
pub use lotus_curation::row_uniqueness_key;

use lotus_curation::CurationInputRow;

pub fn example_rows() -> Vec<CurationInputRow> {
    vec![
        CurationInputRow {
            name: "Voatriafricanine A".into(),
            smiles: "OC12N3C4=C(O)C([C@H](C[C@H]/5[C@@H]6C(OC)=O)C(N([H])C7=C8C=CC=C7)=C8CC6N(C)CC5=C\\C)=CC=C4[C@@]19CCN%10C9[C@@]%11(C[C@H]2C[C@H]%12[C@H]%13[C@@]%14(CC(C(OC)=O)=C%15NC%16=CC=CC=C%16[C@@]%15%17CCN([C@@H]%123)C%14%17)CCO%13)CCO[C@H]%11CC%10".into(),
            taxon: Some("Voacanga africana".into()),
            doi: Some("10.1021/acs.jnatprod.1c00812".into()),
        },
        CurationInputRow {
            name: "Voatriafricanine B (taxon and DOI wrong but new)".into(),
            smiles: "OC12N3C4=C(O)C([C@H](C[C@H]/5[C@@H]6C(OC)=O)C(N([H])C7=C8C=CC=C7)=C8CC6N(C)CC5=C\\C)=CC=C4[C@@]19CCN%10C9[C@@]%11(C[C@H]2C[C@H]%12[C@H]%13[C@@]%14(CC(C(OC)=O)=C%15NC%16=C(OC)C=CC=C%16[C@@]%15%17CCN([C@@H]%123)C%14%17)CCO%13)CCO[C@H]%11CC%10".into(),
            taxon: Some("Gentiana lutea".into()),
            doi: Some("10.1068/P080363".into()),
        },
        CurationInputRow {
            name: "[HYPOTHETICAL - non-real test case]".into(),
            smiles: "CCN(CC)C(=O)N1C=NC2=C1N=CN2C(F)(F)F".into(),
            taxon: Some("Ficticia imaginaria".into()),
            doi: Some("10.59350/sk00y-3gh44".into()),
        },
    ]
}
