//! String helpers.

/// Keep the last `max` characters of `s`, prefixed with "..." when it was cut.
pub fn elide_start(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(3);
    let tail: String = s.chars().skip(n - keep).collect();
    format!("...{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elides_from_the_start() {
        assert_eq!(elide_start("abc", 5), "abc");
        assert_eq!(elide_start("abcdefgh", 6), "...fgh");
    }
}
