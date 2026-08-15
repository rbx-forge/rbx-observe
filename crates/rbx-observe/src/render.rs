//! Human-output helpers. The `--json` path never comes through here.

use colored::Colorize;

/// `1540435` → `1 540 435`. A thin space would be prettier and breaks
/// alignment in half the terminals that matter, so the separator is a plain
/// space.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

/// Robux, or the reason there is no number. "off sale" and "free" are
/// different states and collapsing them loses the interesting half.
pub fn price(robux: Option<u64>, for_sale: bool) -> String {
    match (robux, for_sale) {
        (Some(0), true) | (None, true) => "free".to_string(),
        (Some(value), true) => format!("R$ {}", thousands(value)),
        (Some(value), false) => format!("R$ {} (off sale)", thousands(value)),
        (None, false) => "off sale".to_string(),
    }
}

/// Cuts long free text to `limit` characters on a character boundary — game
/// descriptions run to thousands of characters and are full of emoji, so
/// slicing bytes would panic.
pub fn truncate(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_string();
    }
    let kept: String = trimmed.chars().take(limit).collect();
    format!("{}…", kept.trim_end())
}

/// Free text as its own indented block: descriptions carry hard newlines that
/// would otherwise break the alignment of everything printed after them.
pub fn block(text: &str, indent: &str) -> String {
    text.lines()
        .map(|line| format!("{indent}{}", line.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Width of the name column in every listing, so storefront, badges and
/// charts line up as one tool. Long names push the grey id block right rather
/// than being cut: a truncated product name is worse than a ragged column.
pub const NAME_WIDTH: usize = 32;

pub fn heading(text: &str) -> String {
    text.bold().to_string()
}

pub fn dim(text: &str) -> String {
    text.dimmed().to_string()
}

/// One line, at the end of a section, saying how to turn the ids just printed
/// into images.
///
/// Printing a URL next to every id was the previous shape and it was the worst
/// of both: long, repetitive, and not even an image link — the CDN URL cannot
/// be built from an id (opaque hash, and the path segment varies by asset
/// type), so what got printed was the API call, not the picture. Ids here, one
/// command to resolve them.
pub fn asset_hint(ids: &[u64]) -> Option<String> {
    let mut unique: Vec<u64> = Vec::new();
    for id in ids {
        if !unique.contains(id) {
            unique.push(*id);
        }
    }
    if unique.is_empty() {
        return None;
    }

    // Long id lists wrap and stop being copy-pasteable; three is enough to
    // show the shape of the command.
    const SHOWN: usize = 3;
    let listed = unique
        .iter()
        .take(SHOWN)
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    let more = if unique.len() > SHOWN { " …" } else { "" };

    Some(format!("render icons  rbx-observe asset {listed}{more}"))
}

/// `2026-07-03T17:35:39.839Z` → `2026-07-03`. Nothing here needs the clock
/// time, and the full ISO string wrecks the column width.
pub fn date(timestamp: Option<&str>) -> String {
    timestamp
        .and_then(|value| value.split('T').next())
        .unwrap_or("-")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_groups_from_the_right() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1 000");
        assert_eq!(thousands(1_540_435), "1 540 435");
    }

    #[test]
    fn free_and_off_sale_stay_distinct() {
        assert_eq!(price(Some(49), true), "R$ 49");
        assert_eq!(price(Some(0), true), "free");
        assert_eq!(price(None, true), "free");
        assert_eq!(price(Some(49), false), "R$ 49 (off sale)");
        assert_eq!(price(None, false), "off sale");
    }

    #[test]
    fn the_asset_hint_dedupes_and_stops_at_three() {
        assert_eq!(asset_hint(&[]), None);
        // A storefront where every product shares one icon should not print
        // that id eleven times.
        assert_eq!(
            asset_hint(&[7, 7, 7]).unwrap(),
            "render icons  rbx-observe asset 7"
        );
        assert!(asset_hint(&[1, 2, 3, 4]).unwrap().ends_with("1 2 3 …"));
    }

    #[test]
    fn truncation_counts_characters_not_bytes() {
        // Descriptions open with emoji far more often than not; slicing bytes
        // here would panic mid-codepoint.
        assert_eq!(truncate("🌎 a long description", 8), "🌎 a long…");
        assert_eq!(truncate("  short  ", 50), "short");
    }

    #[test]
    fn blocks_are_indented_line_by_line() {
        assert_eq!(block("a\n  b", "  "), "  a\n  b");
    }

    #[test]
    fn dates_lose_the_clock_and_survive_being_absent() {
        assert_eq!(date(Some("2026-07-03T17:35:39.839Z")), "2026-07-03");
        assert_eq!(date(None), "-");
    }
}
