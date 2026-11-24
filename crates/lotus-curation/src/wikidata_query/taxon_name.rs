// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Botanical and mycological binomial / infraspecific name parsing and author stripping.

/// Extracts a canonical binomial or infraspecific scientific name.
///
/// Strips trailing botanical/mycological author citations and normalises
/// infraspecific rank abbreviations (`ssp.` -> `subsp.`, `forma` -> `f.`).
/// Returns `None` for bare genera (`"Gentiana"`), uncapitalised genera
/// (`"gentiana lutea"`), open nomenclature (`"Gentiana sp."`, `"Gentiana cf. lutea"`),
/// or malformed lowercase trailing words (`"Homo sapiens extra"`).
#[must_use]
pub fn extract_canonical_taxon_name(name: &str) -> Option<String> {
    let tokens: Vec<&str> = name.split_whitespace().filter(|w| !w.is_empty()).collect();
    let first = *tokens.first()?;
    let mut hybrid = false;
    let genus_clean = first.strip_prefix('×').map_or(first, |rest| {
        hybrid = true;
        rest
    });
    if !is_valid_genus(genus_clean) {
        return None;
    }

    let mut idx = 1usize;
    if tokens.get(idx).copied() == Some("×") {
        hybrid = true;
        idx += 1;
    }
    let epithet_raw = *tokens.get(idx)?;
    let epithet = epithet_raw.strip_prefix('×').map_or(epithet_raw, |rest| {
        hybrid = true;
        rest
    });
    if !is_valid_epithet(epithet) {
        return None;
    }

    let genus_out = if first.starts_with('×') {
        first.to_string()
    } else {
        genus_clean.to_string()
    };
    let base = if hybrid && !genus_out.starts_with('×') {
        format!("{genus_out} × {epithet}")
    } else {
        format!("{genus_out} {epithet}")
    };

    let rest = tokens.get(idx + 1..).unwrap_or_default();
    if rest.is_empty() {
        return Some(base);
    }

    if let Some((rank, infra)) = parse_infraspecific_tail(rest) {
        return Some(format!("{base} {rank} {infra}"));
    }
    parse_after_author_citation(&base, rest)
}

/// Whether a taxon name resolves to a species binomial or infraspecific taxon
/// (with optional botanical/mycological author citation stripped).
#[must_use]
pub fn is_binomial(name: &str) -> bool {
    extract_canonical_taxon_name(name).is_some()
}

fn is_valid_genus(tok: &str) -> bool {
    let mut chars = tok.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_uppercase()
        && tok.len() >= 2
        && chars.all(|c| c.is_ascii_lowercase() || c == '-')
        && !is_open_nomenclature(tok)
}

fn is_valid_epithet(tok: &str) -> bool {
    let mut chars = tok.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|c| c.is_ascii_lowercase() || c == '-')
        && !is_open_nomenclature(tok)
}

fn is_open_nomenclature(tok: &str) -> bool {
    matches!(
        tok.trim_end_matches(',').to_ascii_lowercase().as_str(),
        "sp" | "sp."
            | "spp"
            | "spp."
            | "cf"
            | "cf."
            | "aff"
            | "aff."
            | "indet"
            | "indet."
            | "sect"
            | "sect."
            | "subg"
            | "subg."
            | "gr"
            | "gr."
            | "agg"
            | "agg."
            | "nr"
            | "nr."
            | "s.l"
            | "s.l."
            | "s.s"
            | "s.s."
            | "sensu"
    )
}

fn normalize_infraspecific_rank(tok: &str) -> Option<&'static str> {
    match tok.to_ascii_lowercase().as_str() {
        "subsp." | "ssp." | "subsp" | "ssp" => Some("subsp."),
        "var." | "var" => Some("var."),
        "f.sp." => Some("f. sp."),
        "f." | "forma" => Some("f."),
        "subvar." => Some("subvar."),
        "subf." => Some("subf."),
        _ => None,
    }
}

fn starts_with_author_token(tok: &str, next: Option<&str>) -> bool {
    tok.starts_with('(')
        || tok.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        || (matches!(
            tok.to_ascii_lowercase().as_str(),
            "van" | "von" | "de" | "da" | "del" | "du" | "ex" | "et" | "in"
        ) && next.is_some_and(|n| {
            n.starts_with('(') || n.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        }))
}

fn parse_infraspecific_tail<'a>(rest: &[&'a str]) -> Option<(&'static str, &'a str)> {
    let first = *rest.first()?;
    let (rank, infra, after) = if first.eq_ignore_ascii_case("f.")
        && rest.get(1).is_some_and(|s| s.eq_ignore_ascii_case("sp."))
    {
        ("f. sp.", *rest.get(2)?, rest.get(3..).unwrap_or_default())
    } else {
        (
            normalize_infraspecific_rank(first)?,
            *rest.get(1)?,
            rest.get(2..).unwrap_or_default(),
        )
    };
    if !is_valid_epithet(infra) {
        return None;
    }
    if after.is_empty() || starts_with_author_token(after.first()?, after.get(1).copied()) {
        Some((rank, infra))
    } else {
        None
    }
}

fn parse_after_author_citation(base: &str, rest: &[&str]) -> Option<String> {
    let first = *rest.first()?;
    if !starts_with_author_token(first, rest.get(1).copied()) {
        return None;
    }
    for idx in 1..rest.len() {
        let Some(slice) = rest.get(idx..) else {
            break;
        };
        if let Some((rank, infra)) = parse_infraspecific_tail(slice) {
            return Some(format!("{base} {rank} {infra}"));
        }
    }
    Some(base.to_string())
}
