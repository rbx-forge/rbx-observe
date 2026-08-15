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

pub fn heading(text: &str) -> String {
    text.bold().to_string()
}

pub fn dim(text: &str) -> String {
    text.dimmed().to_string()
}

/// An asset id is only useful if you can look at it, so it is always printed
/// with the URL that renders it.
pub fn asset_url(asset_id: u64) -> String {
    format!("https://thumbnails.roblox.com/v1/assets?assetIds={asset_id}&size=420x420&format=Png")
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
    fn dates_lose_the_clock_and_survive_being_absent() {
        assert_eq!(date(Some("2026-07-03T17:35:39.839Z")), "2026-07-03");
        assert_eq!(date(None), "-");
    }
}
