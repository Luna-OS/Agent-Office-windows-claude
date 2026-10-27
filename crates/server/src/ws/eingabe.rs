//! Eingaben aus dem Netz säubern: Texte kürzen, Zahlen begrenzen, Farben prüfen.

/// Kürzt auf `max` Zeichen und entfernt Steuerzeichen.
pub fn text(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control() || *c == '\n').take(max).collect()
}

/// Eine Zahl, endlich und in [-max, max]; sonst 0.
pub fn zahl(v: f32, max: f32) -> f32 {
    if v.is_finite() { v.clamp(-max, max) } else { 0.0 }
}

/// "#rrggbb"?
pub fn ist_farbe(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// Eine http(s)-URL von vernünftiger Länge?
pub fn ist_url(s: &str) -> bool {
    (s.starts_with("https://") || s.starts_with("http://")) && s.len() <= 2000 && !s.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saeubern() {
        assert_eq!(text("ab\u{0}c", 2), "ab");
        assert_eq!(zahl(f32::NAN, 5.0), 0.0);
        assert_eq!(zahl(9.0, 5.0), 5.0);
        assert!(ist_farbe("#a0B1c2"));
        assert!(!ist_farbe("#a0B1c"));
        assert!(ist_url("https://x.de/a.png"));
        assert!(!ist_url("javascript:alert(1)"));
    }
}
