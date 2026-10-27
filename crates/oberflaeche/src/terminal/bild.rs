//! Zeichnet einen vt100-Bildschirm als HTML: eine `<pre>`-Zeile je Terminalzeile, gleiche Attribute
//! zu einem `<span>` zusammengefasst. Als Text im DOM bleibt alles markier- und kopierbar.

use std::fmt::Write;

use vt100::{Color, Screen};

/// Hintergrund und Schrift (wie die Terminals im alten Office).
pub const HINTERGRUND: &str = "#1e1f2e";
pub const SCHRIFT: &str = "#e6e6f0";
const CURSOR: &str = "#ffd166";

/// Die 16 Grundfarben.
const PALETTE: [&str; 16] = [
    "#282a36", "#ff5c7a", "#7cf29a", "#ffd166", "#6cb6ff", "#d69cff", "#72ddf7", "#e6e6f0", "#6c7086", "#ff8fa3", "#a6f4b8",
    "#ffe29a", "#9ccfff", "#e5c1ff", "#a5ecfb", "#ffffff",
];

/// Eine der 256 Farben als CSS.
fn idx(i: u8) -> String {
    match i {
        0..16 => PALETTE[i as usize].to_string(),
        16..232 => {
            let n = i - 16;
            let stufe = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            format!("#{:02x}{:02x}{:02x}", stufe(n / 36), stufe((n / 6) % 6), stufe(n % 6))
        }
        _ => {
            let g = 8 + (i - 232) * 10;
            format!("#{g:02x}{g:02x}{g:02x}")
        }
    }
}

fn farbe(c: Color, fett: bool) -> Option<String> {
    match c {
        Color::Default => None,
        // Fett hellt die Grundfarben auf, wie in xterm.
        Color::Idx(i) if fett && i < 8 => Some(idx(i + 8)),
        Color::Idx(i) => Some(idx(i)),
        Color::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
    }
}

/// Wie eine Zelle aussieht; gleiche Nachbarn kommen in einen Span.
#[derive(PartialEq, Default)]
struct Stil {
    vorne: Option<String>,
    hinten: Option<String>,
    fett: bool,
    blass: bool,
    kursiv: bool,
    unterstrichen: bool,
    cursor: bool,
}

impl Stil {
    fn css(&self) -> String {
        let mut s = String::new();
        if let Some(v) = &self.vorne {
            let _ = write!(s, "color:{v};");
        }
        if let Some(h) = &self.hinten {
            let _ = write!(s, "background:{h};");
        }
        if self.fett {
            s.push_str("font-weight:700;");
        }
        if self.blass {
            s.push_str("opacity:.6;");
        }
        if self.kursiv {
            s.push_str("font-style:italic;");
        }
        if self.unterstrichen {
            s.push_str("text-decoration:underline;");
        }
        s
    }
}

fn escape(text: &str, aus: &mut String) {
    for ch in text.chars() {
        match ch {
            '<' => aus.push_str("&lt;"),
            '>' => aus.push_str("&gt;"),
            '&' => aus.push_str("&amp;"),
            _ => aus.push(ch),
        }
    }
}

fn span(stil: &Stil, text: &str, aus: &mut String) {
    if text.is_empty() {
        return;
    }
    let css = stil.css();
    if css.is_empty() && !stil.cursor {
        escape(text, aus);
        return;
    }
    aus.push_str("<span");
    if stil.cursor {
        aus.push_str(" class=\"cursor\"");
    }
    if !css.is_empty() {
        let _ = write!(aus, " style=\"{css}\"");
    }
    aus.push('>');
    escape(text, aus);
    aus.push_str("</span>");
}

/// Der ganze Bildschirm als HTML. `fokus`: der Cursor blinkt nur im Terminal, das man gerade tippt.
pub fn html(screen: &Screen, fokus: bool) -> String {
    let (zeilen, spalten) = screen.size();
    let (zeilen, spalten) = (zeilen.get(), spalten.get());
    let (cz, cs) = screen.cursor_position();
    let cursor_zeigen = !screen.hide_cursor() && screen.scrollback() == 0;
    let mut aus = String::with_capacity(zeilen as usize * (spalten as usize + 32));
    for z in 0..zeilen {
        aus.push_str("<pre>");
        let mut stil = Stil::default();
        let mut text = String::new();
        for s in 0..spalten {
            let Some(zelle) = screen.cell(z, s) else { continue };
            if zelle.is_wide_continuation() {
                continue;
            }
            let fett = zelle.bold();
            let (mut v, mut h) = (farbe(zelle.fgcolor(), fett), farbe(zelle.bgcolor(), false));
            if zelle.inverse() {
                (v, h) = (Some(h.unwrap_or_else(|| HINTERGRUND.into())), Some(v.unwrap_or_else(|| SCHRIFT.into())));
            }
            let ist_cursor = cursor_zeigen && z == cz && s == cs;
            if ist_cursor {
                h = Some(if fokus { CURSOR.into() } else { "#6c7086".into() });
                v = Some(HINTERGRUND.into());
            }
            let neu = Stil {
                vorne: v,
                hinten: h,
                fett,
                blass: zelle.dim(),
                kursiv: zelle.italic(),
                unterstrichen: zelle.underline(),
                cursor: ist_cursor,
            };
            if neu != stil {
                span(&stil, &text, &mut aus);
                text.clear();
                stil = neu;
            }
            if zelle.has_contents() {
                text.push_str(zelle.contents());
            } else {
                text.push(' ');
            }
        }
        // Leerzeichen am Zeilenende ohne Farbe weglassen: kürzeres HTML, sauberes Kopieren.
        if stil == Stil::default() {
            let ende = text.trim_end_matches(' ').len();
            text.truncate(ende);
        }
        span(&stil, &text, &mut aus);
        aus.push_str("</pre>");
    }
    aus
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;

    #[test]
    fn farben_und_escape() {
        let mut p = vt100::Parser::new(NonZeroU16::new(2).unwrap(), NonZeroU16::new(10).unwrap(), 0);
        p.process(b"\x1b[31mrot\x1b[0m <a>\x1b[?25l");
        let h = super::html(p.screen(), false);
        assert!(h.starts_with("<pre><span style=\"color:#ff5c7a;\">rot</span> &lt;a&gt;</pre>"), "{h}");
        assert_eq!(h.matches("<pre>").count(), 2);
    }
}
