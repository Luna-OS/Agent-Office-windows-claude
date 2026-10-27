//! Der Terminal-Puffer eines Arbeiters auf dem Server: alles, was sein PTY ausgibt, läuft durch
//! einen vt100-Parser. Daraus entsteht der Snapshot für einen Browser, der das Terminal neu öffnet,
//! der Fenstertitel und der Fortschritt (OSC 9;4), an dem man sieht, ob ein Agent gerade arbeitet.

/// Was der Parser nebenbei mitbekommt.
#[derive(Default)]
struct Merker {
    titel: Option<String>,
    /// Letzter OSC 9;4-Fortschritt: `Some(true)` arbeitet, `Some(false)` ruht.
    fortschritt: Option<bool>,
    /// OSC 9;4;4 (pausiert): wartet auf einen Menschen – bis zum Abholen.
    wartet: bool,
    /// OSC 7700;sitzung;<id> von ACP-Agenten – bis zum Abholen.
    sitzung: Option<String>,
}

impl vt100::Callbacks for Merker {
    fn set_window_title(&mut self, _: &mut vt100::Screen, titel: &[u8]) {
        self.titel = Some(String::from_utf8_lossy(titel).trim().to_string());
    }

    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        // OSC 9;4;<zustand>[;<prozent>]: 0 heißt fertig/ruhend, alles andere beschäftigt.
        if params.len() >= 3 && params[0] == b"9" && params[1] == b"4" {
            self.fortschritt = Some(params[2] != b"0");
            self.wartet |= params[2] == b"4";
        }
        if params.len() >= 3 && params[0] == b"7700" && params[1] == b"sitzung" {
            self.sitzung = Some(String::from_utf8_lossy(params[2]).into_owned());
        }
    }
}

/// Der Bildschirm eines Arbeiters.
pub struct Terminal {
    parser: vt100::Parser<Merker>,
    /// Bytes eines noch unvollständigen UTF-8-Zeichens vom Ende des letzten Stücks.
    rest: Vec<u8>,
}

/// Was ein Stück Ausgabe verändert hat.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Aenderung {
    /// Die Ausgabe als Text, für die Zuschauer.
    pub text: String,
    pub neuer_titel: Option<String>,
    pub neuer_fortschritt: Option<bool>,
    /// Der Agent wartet jetzt auf einen Menschen (OSC 9;4;4).
    pub wartet: bool,
    /// Eine neue Sitzungs-ID (OSC 7700;sitzung;<id>).
    pub sitzung: Option<String>,
}

impl Terminal {
    pub fn neu(spalten: u16, zeilen: u16) -> Self {
        Self {
            parser: vt100::Parser::new_with_callbacks(zeilen, spalten, 2000, Merker::default()),
            rest: Vec::new(),
        }
    }

    /// Nimmt ein Stück PTY-Ausgabe auf. Ein am Ende abgeschnittenes UTF-8-Zeichen wartet auf das nächste Stück.
    pub fn einspeisen(&mut self, bytes: &[u8]) -> Aenderung {
        let mut daten = std::mem::take(&mut self.rest);
        daten.extend_from_slice(bytes);
        let gueltig = match std::str::from_utf8(&daten) {
            Ok(_) => daten.len(),
            Err(e) if e.error_len().is_none() => e.valid_up_to(),
            Err(_) => daten.len(),
        };
        self.rest = daten.split_off(gueltig);
        let titel_vorher = self.parser.callbacks().titel.clone();
        let fortschritt_vorher = self.parser.callbacks().fortschritt;
        self.parser.process(&daten);
        let merker = self.parser.callbacks_mut();
        let wartet = std::mem::take(&mut merker.wartet);
        let sitzung = merker.sitzung.take();
        let merker = self.parser.callbacks();
        Aenderung {
            wartet,
            sitzung,
            text: String::from_utf8_lossy(&daten).into_owned(),
            neuer_titel: (merker.titel != titel_vorher)
                .then(|| merker.titel.clone())
                .flatten(),
            neuer_fortschritt: (merker.fortschritt != fortschritt_vorher)
                .then_some(merker.fortschritt)
                .flatten(),
        }
    }

    /// Der ganze sichtbare Bildschirm samt Farben und Cursor, als Escape-Sequenzen.
    pub fn snapshot(&self) -> String {
        let schirm = self.parser.screen();
        let mut aus = Vec::new();
        if schirm.alternate_screen() {
            aus.extend_from_slice(b"\x1b[?1049h");
        }
        aus.extend_from_slice(b"\x1b[H\x1b[2J");
        aus.extend_from_slice(&schirm.state_formatted());
        String::from_utf8_lossy(&aus).into_owned()
    }

    /// Der Bildschirm als reiner Text (zum Erkennen von Login- oder Vertrauensabfragen).
    pub fn text(&self) -> String {
        self.parser.screen().contents()
    }

    pub fn titel(&self) -> Option<&str> {
        self.parser.callbacks().titel.as_deref()
    }

    pub fn groesse_aendern(&mut self, spalten: u16, zeilen: u16) {
        self.parser.screen_mut().set_size(zeilen, spalten);
    }

    /// Alle Zeilen samt Verlauf (älteste zuerst), als reiner Text – für die Suche.
    pub fn alle_zeilen(&mut self) -> Vec<String> {
        let schirm = self.parser.screen_mut();
        let (zeilen, spalten) = schirm.size();
        let zeilen = zeilen as usize;
        schirm.set_scrollback(usize::MAX);
        let verlauf = schirm.scrollback();
        let mut aus = Vec::new();
        let mut versatz = verlauf;
        loop {
            schirm.set_scrollback(versatz);
            let nimm = if versatz == 0 {
                zeilen
            } else {
                zeilen.min(versatz)
            };
            aus.extend(
                schirm
                    .rows(0, spalten)
                    .take(nimm)
                    .map(|z| z.trim_end().to_string()),
            );
            if versatz == 0 {
                break;
            }
            versatz = versatz.saturating_sub(zeilen);
        }
        schirm.set_scrollback(0);
        aus
    }

    /// Die Antwort auf eine Cursor-Abfrage (DSR 6): `ESC [ zeile ; spalte R`, ab 1 gezählt.
    pub fn cursor_antwort(&self) -> String {
        let (zeile, spalte) = self.parser.screen().cursor_position();
        format!("\x1b[{};{}R", zeile + 1, spalte + 1)
    }

    pub fn groesse(&self) -> (u16, u16) {
        let (zeilen, spalten) = self.parser.screen().size();
        (spalten, zeilen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_ueber_stueckgrenzen() {
        let mut t = Terminal::neu(80, 24);
        let ae = "ä".as_bytes();
        let a = t.einspeisen(&[b'x', ae[0]]);
        assert_eq!(a.text, "x");
        let b = t.einspeisen(&[ae[1], b'y']);
        assert_eq!(b.text, "äy");
        assert!(t.text().starts_with("xäy"));
    }

    #[test]
    fn titel_und_fortschritt() {
        let mut t = Terminal::neu(80, 24);
        let a = t.einspeisen(b"\x1b]0;Mein Titel\x07");
        assert_eq!(a.neuer_titel.as_deref(), Some("Mein Titel"));
        let b = t.einspeisen(b"\x1b]9;4;3\x07");
        assert_eq!(b.neuer_fortschritt, Some(true));
        let c = t.einspeisen(b"\x1b]9;4;0\x07");
        assert_eq!(c.neuer_fortschritt, Some(false));
        let d = t.einspeisen(b"\x1b]9;4;4\x07\x1b]7700;sitzung;abc\x07");
        assert!(d.wartet);
        assert_eq!(d.sitzung.as_deref(), Some("abc"));
        assert!(!t.einspeisen(b"x").wartet);
    }

    #[test]
    fn zeilen_mit_verlauf() {
        let mut t = Terminal::neu(20, 3);
        for i in 0..10 {
            t.einspeisen(format!("zeile {i}\r\n").as_bytes());
        }
        let z: Vec<String> = t
            .alle_zeilen()
            .into_iter()
            .filter(|z| !z.is_empty())
            .collect();
        assert_eq!(z.first().map(String::as_str), Some("zeile 0"));
        assert_eq!(z.last().map(String::as_str), Some("zeile 9"));
        assert_eq!(z.len(), 10);
    }

    #[test]
    fn snapshot_enthaelt_text() {
        let mut t = Terminal::neu(40, 10);
        t.einspeisen(b"hallo \x1b[31mrot\x1b[0m");
        let s = t.snapshot();
        assert!(s.contains("hallo"));
        assert!(s.contains("rot"));
    }
}
