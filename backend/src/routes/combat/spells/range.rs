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
