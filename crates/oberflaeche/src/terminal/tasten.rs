//! Tasten → Bytes für den PTY, wie xterm sie schickt, plus die Editier-Tasten, die man aus dem eigenen
//! Terminal kennt (Shift+Enter neue Zeile, Strg+⌫ Wort löschen, ⌘← Zeilenanfang …).

/// Was an einer Taste hängt.
pub struct Taste<'a> {
    pub key: &'a str,
    pub strg: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

/// Ob der Browser auf einem Mac läuft (⌘ ist dort die Editier-Taste).
pub fn ist_mac() -> bool {
    web_sys::window()
        .and_then(|w| w.navigator().platform().ok())
        .is_some_and(|p| p.contains("Mac") || p.contains("iPhone") || p.contains("iPad"))
}

/// xterm-Modifikator-Parameter: 1 + Shift + 2·Alt + 4·Strg.
fn modifikator(t: &Taste) -> u8 {
    1 + t.shift as u8 + 2 * t.alt as u8 + 4 * t.strg as u8
}

/// Pfeile, Pos1 und Ende: `CSI x`, im Anwendungsmodus `SS3 x`, mit Modifikatoren `CSI 1;m x`.
fn cursor(t: &Taste, buchstabe: char, anwendung: bool) -> String {
    let m = modifikator(t);
    if m > 1 {
        format!("\x1b[1;{m}{buchstabe}")
    } else if anwendung {
        format!("\x1bO{buchstabe}")
    } else {
        format!("\x1b[{buchstabe}")
    }
}

/// `CSI n ~` (Bild auf, Entf, F5 …), mit Modifikatoren `CSI n;m ~`.
fn tilde(t: &Taste, n: u8) -> String {
    match modifikator(t) {
        1 => format!("\x1b[{n}~"),
        m => format!("\x1b[{n};{m}~"),
    }
}

/// Die Bytes zu einer Taste, oder `None`, wenn das Terminal sie nicht nimmt (der Browser behält sie).
pub fn bytes(t: &Taste, anwendung: bool, mac: bool) -> Option<String> {
    let cmd = mac && t.meta && !t.strg && !t.alt;
    // Die vertrauten Editier-Tasten zuerst.
    match t.key {
        "Enter" if t.shift && !t.strg && !t.alt => return Some("\n".into()),
        "Backspace" if t.strg && !mac => return Some("\x17".into()),
        "Backspace" if cmd => return Some("\x15".into()),
        "Delete" if cmd => return Some("\x0b".into()),
        "ArrowLeft" if cmd => return Some("\x01".into()),
        "ArrowRight" if cmd => return Some("\x05".into()),
        _ => {}
    }
    if t.meta {
        return None;
    }
    let alt = |s: &str| if t.alt { format!("\x1b{s}") } else { s.to_string() };
    let aus = match t.key {
        "Enter" => alt("\r"),
        "Backspace" => alt(if t.strg { "\x08" } else { "\x7f" }),
        "Tab" if t.shift => "\x1b[Z".into(),
        "Tab" => "\t".into(),
        "Escape" => "\x1b".into(),
        "ArrowUp" => cursor(t, 'A', anwendung),
        "ArrowDown" => cursor(t, 'B', anwendung),
        "ArrowRight" => cursor(t, 'C', anwendung),
        "ArrowLeft" => cursor(t, 'D', anwendung),
        "Home" => cursor(t, 'H', anwendung),
        "End" => cursor(t, 'F', anwendung),
        "Insert" => tilde(t, 2),
        "Delete" => tilde(t, 3),
        "PageUp" => tilde(t, 5),
        "PageDown" => tilde(t, 6),
        "F1" => "\x1bOP".into(),
        "F2" => "\x1bOQ".into(),
        "F3" => "\x1bOR".into(),
        "F4" => "\x1bOS".into(),
        "F5" => tilde(t, 15),
        "F6" => tilde(t, 17),
        "F7" => tilde(t, 18),
        "F8" => tilde(t, 19),
        "F9" => tilde(t, 20),
        "F10" => tilde(t, 21),
        "F11" => tilde(t, 23),
        "F12" => tilde(t, 24),
        k => {
            let mut zeichen = k.chars();
            let (Some(c), None) = (zeichen.next(), zeichen.next()) else { return None };
            if t.strg {
                steuerzeichen(c).map(|b| alt(&(b as char).to_string()))?
            } else {
                alt(k)
            }
        }
    };
    Some(aus)
}

/// Strg+Taste: Strg+A = 0x01 … Strg+Z = 0x1a, Strg+[ = Esc, Strg+Leertaste = NUL.
fn steuerzeichen(c: char) -> Option<u8> {
    Some(match c.to_ascii_lowercase() {
        c @ 'a'..='z' => c as u8 - b'a' + 1,
        ' ' | '@' | '2' => 0,
        '[' | '3' => 0x1b,
        '\\' | '4' => 0x1c,
        ']' | '5' => 0x1d,
        '^' | '6' => 0x1e,
        '_' | '-' | '7' => 0x1f,
        '8' | '?' => 0x7f,
        _ => return None,
    })
}

/// Eingefügter Text, in Klammern, wenn das Programm darum gebeten hat.
pub fn einfuegen(text: &str, klammern: bool) -> String {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if klammern { format!("\x1b[200~{text}\x1b[201~") } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(key: &str) -> Taste<'_> {
        Taste { key, strg: false, alt: false, shift: false, meta: false }
    }

    #[test]
    fn grundtasten() {
        assert_eq!(bytes(&t("a"), false, false).as_deref(), Some("a"));
        assert_eq!(bytes(&t("Enter"), false, false).as_deref(), Some("\r"));
        assert_eq!(bytes(&t("ArrowUp"), false, false).as_deref(), Some("\x1b[A"));
        assert_eq!(bytes(&t("ArrowUp"), true, false).as_deref(), Some("\x1bOA"));
        assert_eq!(bytes(&t("F5"), false, false).as_deref(), Some("\x1b[15~"));
        assert_eq!(bytes(&t("Shift"), false, false), None);
    }

    #[test]
    fn modifikatoren() {
        let strg_c = Taste { strg: true, ..t("c") };
        assert_eq!(bytes(&strg_c, false, false).as_deref(), Some("\x03"));
        let strg_klammer = Taste { strg: true, ..t("[") };
        assert_eq!(bytes(&strg_klammer, false, false).as_deref(), Some("\x1b"));
        let shift_enter = Taste { shift: true, ..t("Enter") };
        assert_eq!(bytes(&shift_enter, false, false).as_deref(), Some("\n"));
        let strg_rechts = Taste { strg: true, ..t("ArrowRight") };
        assert_eq!(bytes(&strg_rechts, false, false).as_deref(), Some("\x1b[1;5C"));
        let cmd_links = Taste { meta: true, ..t("ArrowLeft") };
        assert_eq!(bytes(&cmd_links, false, true).as_deref(), Some("\x01"));
        let alt_b = Taste { alt: true, ..t("b") };
        assert_eq!(bytes(&alt_b, false, false).as_deref(), Some("\x1bb"));
    }

    #[test]
    fn einfuegen_mit_klammern() {
        assert_eq!(einfuegen("a\nb", true), "\x1b[200~a\rb\x1b[201~");
    }
}
