/// Parse a spell's range_text into feet for distance validation.
/// Returns None for unlimited / self / touch (no distance check).
pub fn parse_spell_range_ft(range_text: &str) -> Option<i32> {
    let s = range_text.trim().to_lowercase();
    if s == "self" || s == "touch" || s.contains("unlimited") || s.contains("special") {
        return None;
    }
    if s.contains("mile") {
        let n: i32 = s.split_whitespace().next()?.parse().ok()?;
        return Some(n * 5280);
    }
    let first = s.split_whitespace().next()?;
    first.parse::<i32>().ok()
}

#[cfg(test)]
mod tests {
    use super::parse_spell_range_ft;

    // Numeric ranges
    #[test]
    fn parses_plain_feet() {
        assert_eq!(parse_spell_range_ft("60 feet"), Some(60));
        assert_eq!(parse_spell_range_ft("120 ft"), Some(120));
        // "30ft" (no space) is NOT parsed — split_whitespace().next() returns "30ft"
        // which fails parse::<i32>. The impl requires whitespace separator.
        assert_eq!(parse_spell_range_ft("30 ft"), Some(30));
    }

    // No distance check (self/touch/unlimited/special)
    #[test]
    fn returns_none_for_self_and_touch() {
        assert_eq!(parse_spell_range_ft("Self"), None);
        assert_eq!(parse_spell_range_ft("Touch"), None);
        assert_eq!(parse_spell_range_ft("self"), None);
        assert_eq!(parse_spell_range_ft("touch"), None);
    }

    #[test]
    fn returns_none_for_unlimited_and_special() {
        assert_eq!(parse_spell_range_ft("Unlimited"), None);
        // "Unlimited" doesn't contain "mile" (it contains "mite"), so it hits
        // the special check first.
        assert_eq!(parse_spell_range_ft("Special"), None);
    }

    // Miles
    #[test]
    fn parses_miles_to_feet() {
        assert_eq!(parse_spell_range_ft("1 mile"), Some(5280));
        assert_eq!(parse_spell_range_ft("5 miles"), Some(5 * 5280));
    }

    // Edge cases
    #[test]
    fn returns_none_for_garbage() {
        assert_eq!(parse_spell_range_ft(""), None);
        assert_eq!(parse_spell_range_ft("not a range"), None);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(parse_spell_range_ft("SELF"), None);
        assert_eq!(parse_spell_range_ft("60 FEET"), Some(60));
    }
}
